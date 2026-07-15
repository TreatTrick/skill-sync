// apply_plan 及其批量入口（upload_skills / download_skills）：在持久化副作用前只改 working
// state，远端结果不明或本地提交失败时返回 RecoveryRequired。包含 download/upload/delete 的执行。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::{AppConfig, LimitsConfig};
use crate::errors::{AppError, Result};
use crate::local_apply::{
    clean_dir_contents, clear_journal, commit_staged, move_to_trash, namespace_root, rollback_dir,
    save_journal, stage_dir, trash_dir, ApplyJournal,
};
use crate::pack::{unpack_skill, PackOptions, PackOutcome, SkillPackInput, SkillPacker};
use crate::portable_path::validate_component;
use crate::remote_store::{blob_path_for_hash, BlobWrite, RemoteChanges, RemoteStore};
use crate::skill::{parse_skill_md, skill_id, SkillNamespace};
use crate::sync_state::{SkillSyncState, SyncState};
use crate::vault_manifest::{VaultManifest, VaultSkill};

use super::model::{
    ApplyResult, ApplySyncRequest, ApplySyncResponse, ConflictReason, PlanChangeReason,
    RecoveryInfo, RecoveryPhase, SyncDecision, SyncPlan, SyncStatus,
};
use super::plan::{action_kind_for, merge_plan, validate_remote, LocalSkillInfo};

// ---- Task 9: apply_plan ----

/// 打包后的本地 skill（LocalSkillInfo + zip 路径，zip 存活于 PreparedSyncPlan.batch）。
#[derive(Debug, Clone)]
pub(crate) struct PackedLocal {
    pub info: LocalSkillInfo,
    pub zip_path: PathBuf,
}

/// apply 前重新生成的计划：持有 SyncPlan、本次打包结果（upload bytes）、远端 manifest、
/// 当前 commit，以及 RAII PackBatch（task dir）。
pub(crate) struct PreparedSyncPlan {
    pub plan: SyncPlan,
    pub packed: BTreeMap<String, PackedLocal>,
    pub manifest: VaultManifest,
    pub expected_commit: String,
    pub _batch: crate::pack::PackBatch,
}

/// 重新 fetch + scan + pack + merge，返回 PreparedSyncPlan。`home` 注入用于测试。
async fn prepare_plan<S: RemoteStore>(
    config: &AppConfig,
    state: &SyncState,
    store: &S,
    home: &Path,
) -> Result<PreparedSyncPlan> {
    let remote_cfg = config.remote.as_ref().ok_or_else(|| {
        AppError::NotConfigured("remote not configured; onboarding required".into())
    })?;
    validate_remote(remote_cfg, &state.remote)?;

    let snapshot = store.fetch_manifest().await?;
    let scan = {
        let home = home.to_path_buf();
        tauri::async_runtime::spawn_blocking(move || crate::detect::scan_fixed_roots(&home))
            .await
            .map_err(|e| AppError::Vault(format!("scan task failed: {e}")))?
    }?;

    let pack_inputs: Vec<SkillPackInput> = scan
        .skills
        .iter()
        .map(|s| SkillPackInput {
            source_path: PathBuf::from(&s.source_path),
        })
        .collect();
    let batch = {
        let limits = config.limits.clone();
        let user_ignore = config.ignore.clone();
        tauri::async_runtime::spawn_blocking(move || {
            SkillPacker::pack_batch(
                &pack_inputs,
                &PackOptions {
                    limits,
                    user_ignore,
                },
            )
        })
        .await
        .map_err(|e| AppError::Vault(format!("pack task failed: {e}")))?
    }?;

    let mut local_infos: Vec<LocalSkillInfo> = Vec::new();
    let mut packed: BTreeMap<String, PackedLocal> = BTreeMap::new();
    for (s, o) in scan.skills.iter().zip(batch.outcomes.iter()) {
        match o {
            PackOutcome::Packed(p) => {
                let info = LocalSkillInfo {
                    skill_id: s.id.clone(),
                    name: s.name.clone(),
                    description: s.description.clone(),
                    folder_name: s.folder_name.clone(),
                    namespace: s.namespace,
                    relative_dir: s.relative_dir.clone(),
                    source_path: s.source_path.clone(),
                    hash: p.hash.clone(),
                    zip_size: p.zip_size,
                    warnings: p
                        .warnings
                        .iter()
                        .map(|w| format!("{:?}: {}", w.kind, w.relative_path))
                        .collect(),
                    blocked_reason: None,
                };
                packed.insert(
                    s.id.clone(),
                    PackedLocal {
                        info: info.clone(),
                        zip_path: p.zip_path.clone(),
                    },
                );
                local_infos.push(info);
            }
            PackOutcome::Blocked(b) => {
                local_infos.push(LocalSkillInfo {
                    skill_id: s.id.clone(),
                    name: s.name.clone(),
                    description: s.description.clone(),
                    folder_name: s.folder_name.clone(),
                    namespace: s.namespace,
                    relative_dir: s.relative_dir.clone(),
                    source_path: s.source_path.clone(),
                    hash: String::new(),
                    zip_size: 0,
                    warnings: Vec::new(),
                    blocked_reason: Some(b.reason.clone()),
                });
            }
        }
    }

    let mut plan = merge_plan(
        &state.skills,
        &local_infos,
        &snapshot.manifest,
        &scan.roots,
        &scan.collisions,
        &config.limits,
        &snapshot.commit_sha,
    )?;
    plan.warnings.extend(scan.warnings);
    Ok(PreparedSyncPlan {
        plan,
        packed,
        manifest: snapshot.manifest,
        expected_commit: snapshot.commit_sha,
        _batch: batch,
    })
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn is_executable(status: SyncStatus) -> bool {
    matches!(
        status,
        SyncStatus::LocalUpdate
            | SyncStatus::RemoteUpdate
            | SyncStatus::LocalDeleted
            | SyncStatus::RemoteDeleted
    )
}

/// conflict decision 白名单。
fn allowed_decisions(reason: ConflictReason) -> &'static [SyncDecision] {
    match reason {
        ConflictReason::SameNameFirstSeen | ConflictReason::BothChanged => &[
            SyncDecision::KeepLocal,
            SyncDecision::UseRemote,
            SyncDecision::Skip,
        ],
        ConflictReason::LocalDeletedRemoteChanged => &[
            SyncDecision::DeleteRemote,
            SyncDecision::RestoreRemote,
            SyncDecision::Skip,
        ],
        ConflictReason::RemoteDeletedLocalChanged => &[
            SyncDecision::KeepLocal,
            SyncDecision::AcceptDelete,
            SyncDecision::Skip,
        ],
    }
}

/// 纯删除条目的 decision 白名单。
fn allowed_decisions_for_status(status: SyncStatus) -> &'static [SyncDecision] {
    match status {
        SyncStatus::LocalDeleted => &[
            SyncDecision::RestoreRemote,
            SyncDecision::DeleteRemote,
            SyncDecision::Skip,
        ],
        SyncStatus::RemoteDeleted => &[
            SyncDecision::KeepLocal,
            SyncDecision::AcceptDelete,
            SyncDecision::Skip,
        ],
        _ => &[],
    }
}

struct UploadItem {
    skill_id: String,
    name: String,
    description: String,
    namespace: SkillNamespace,
    folder_name: String,
    relative_dir: Option<String>,
    hash: String,
    zip_size: u64,
    zip_path: PathBuf,
}

struct DownloadItem {
    skill_id: String,
    entry_ns: SkillNamespace,
    entry_folder: String,
    vskill: VaultSkill,
}

struct DeleteLocalItem {
    skill_id: String,
    entry_ns: SkillNamespace,
}

/// 下载目标：base 存在则用 state.namespace/relative_dir（update），否则用 entry.namespace/folder_name（remote-new）。
fn download_target(dl: &DownloadItem, working: &SyncState) -> Result<(SkillNamespace, String)> {
    if let Some(b) = working.skills.get(&dl.skill_id) {
        if dl.entry_ns != b.namespace {
            return Err(AppError::Blocked(format!(
                "download namespace mismatch: {}",
                dl.skill_id
            )));
        }
        validate_component(&b.relative_dir)?;
        Ok((b.namespace, b.relative_dir.clone()))
    } else {
        validate_component(&dl.entry_folder)?;
        Ok((dl.entry_ns, dl.entry_folder.clone()))
    }
}

/// 删本地目标：必须用 base 的 namespace/relative_dir。
fn delete_local_target(
    dl: &DeleteLocalItem,
    working: &SyncState,
) -> Result<(SkillNamespace, String)> {
    let b = working.skills.get(&dl.skill_id).ok_or_else(|| {
        AppError::Blocked(format!("delete_local skill not in base: {}", dl.skill_id))
    })?;
    if dl.entry_ns != b.namespace {
        return Err(AppError::Blocked(format!(
            "delete_local namespace mismatch: {}",
            dl.skill_id
        )));
    }
    validate_component(&b.relative_dir)?;
    Ok((b.namespace, b.relative_dir.clone()))
}

/// 执行单个下载：fetch blob -> 校验 size/hash -> 解包到 staging -> 校验 SKILL.md/identity
/// -> stage -> target（带 rollback）。任一 stage/identity 失败清理 stage 返回 Blocked；
/// replace 失败返回 RecoveryPending。
async fn execute_download<S: RemoteStore>(
    store: &S,
    dl: &DownloadItem,
    working: &SyncState,
    home: &Path,
    task_id: &str,
    limits: &LimitsConfig,
) -> Result<()> {
    let blob = store.fetch_blob(&dl.vskill.blob, &dl.vskill.hash).await?;
    if blob.len() as u64 != dl.vskill.size {
        return Err(AppError::Blocked(format!(
            "blob size mismatch for {}",
            dl.skill_id
        )));
    }
    let computed = format!("sha256:{}", hex::encode(Sha256::digest(&blob)));
    if computed != dl.vskill.hash {
        return Err(AppError::Blocked(format!(
            "blob hash mismatch for {}",
            dl.skill_id
        )));
    }
    let (ns, folder) = download_target(dl, working)?;
    let root = namespace_root(home, ns)?;
    let stage_root = root.join(".skill-sync-staging").join(task_id);
    fs::create_dir_all(&stage_root)?;
    let zip_path = stage_root.join(format!("{folder}.zip"));
    fs::write(&zip_path, &blob)?;
    let stage = stage_dir(&root, task_id, &folder);
    fs::create_dir_all(&stage)?;

    let zip_clone = zip_path.clone();
    let stage_clone = stage.clone();
    let limits_clone = limits.clone();
    tauri::async_runtime::spawn_blocking(move || {
        unpack_skill(&zip_clone, &stage_clone, &limits_clone)
    })
    .await
    .map_err(|e| AppError::Vault(format!("unpack task failed: {e}")))??;

    // 校验根级 SKILL.md 与 identity
    let skill_md = stage.join("SKILL.md");
    if !skill_md.is_file() {
        clean_dir_contents(&stage);
        return Err(AppError::Blocked(format!(
            "download missing SKILL.md: {}",
            dl.skill_id
        )));
    }
    let content = fs::read_to_string(&skill_md)?;
    let meta = parse_skill_md(&content)?;
    let id = skill_id(ns, &meta.name);
    if id != dl.skill_id {
        clean_dir_contents(&stage);
        return Err(AppError::Blocked(format!(
            "download identity mismatch: {id} != {}",
            dl.skill_id
        )));
    }

    let target = root.join(&folder);
    let rollback = rollback_dir(&root, task_id, &folder);
    if let Err(e) = commit_staged(&stage, &target, &rollback) {
        return Err(AppError::RecoveryPending(format!(
            "local replace failed: {e}"
        )));
    }
    drop(fs::remove_file(&zip_path));
    Ok(())
}

fn save_recovery_journal(
    config_dir: &Path,
    task_id: &str,
    phase: &str,
    working: &SyncState,
    completed_action_ids: Vec<String>,
    pending_action_ids: Vec<String>,
) -> Result<()> {
    let bytes = serde_json::to_vec(working)
        .map_err(|e| AppError::Vault(format!("state serialize failed: {e}")))?;
    let journal = ApplyJournal {
        schema: 1,
        task_id: task_id.into(),
        phase: phase.into(),
        remote_candidate: None,
        next_state_hash: format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
        next_state_bytes: bytes,
        remote_base: None,
        next_manifest_hash: None,
        completed_action_ids,
        pending_action_ids,
    };
    save_journal(config_dir, &journal)
}

fn cleanup_task_artifacts(home: &Path, task_id: &str) -> bool {
    let mut ok = true;
    for ns in [
        SkillNamespace::Agents,
        SkillNamespace::Codex,
        SkillNamespace::ClaudeCode,
    ] {
        let root = match namespace_root(home, ns) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for sub in [".skill-sync-rollback", ".skill-sync-staging"] {
            let p = root.join(sub).join(task_id);
            if p.exists() && fs::remove_dir_all(&p).is_err() {
                ok = false;
            }
        }
    }
    ok
}

/// 执行同步计划。`home` 注入固定 root（测试用）。command 先 load/validate state，再 clone
/// 成 working state；engine 在任何持久化副作用前只修改 working。PlanChanged/Blocked/预提交
/// 错误丢弃 clone，原 state 不变；一旦进入远端结果不明或本地提交阶段，失败返回 RecoveryRequired。
pub(crate) async fn apply_plan<S: RemoteStore>(
    config: &AppConfig,
    state: &mut SyncState,
    request: &ApplySyncRequest,
    store: &S,
    home: &Path,
    config_dir: &Path,
) -> Result<ApplySyncResponse> {
    let remote_cfg = match config.remote.as_ref() {
        Some(r) => r,
        None => {
            return Err(AppError::NotConfigured(
                "remote not configured; onboarding required".into(),
            ));
        }
    };
    validate_remote(remote_cfg, &state.remote)?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let mut working = state.clone();

    let prepared = prepare_plan(config, &working, store, home).await?;
    let plan = &prepared.plan;

    // 1-3. 校验 expected_remote_commit + fingerprint
    if request.expected_remote_commit != plan.expected_remote_commit {
        return Ok(ApplySyncResponse::PlanChanged {
            reason: PlanChangeReason::RemoteChanged,
            latest_plan: Box::new(prepared.plan),
        });
    }
    if request.plan_fingerprint != plan.plan_fingerprint {
        return Ok(ApplySyncResponse::PlanChanged {
            reason: PlanChangeReason::PlanChanged,
            latest_plan: Box::new(prepared.plan),
        });
    }

    // 4. 校验 selected_action_ids
    let mut selected_set: HashSet<&str> = HashSet::new();
    for action_id in &request.selected_action_ids {
        if !selected_set.insert(action_id.as_str()) {
            return Err(AppError::Blocked(format!(
                "duplicate selected action_id: {action_id}"
            )));
        }
        let entry = plan
            .entries
            .iter()
            .find(|e| &e.action_id == action_id)
            .ok_or_else(|| AppError::Blocked(format!("unknown action_id: {action_id}")))?;
        if !is_executable(entry.status) {
            return Err(AppError::Blocked(format!(
                "non-executable action_id: {action_id}"
            )));
        }
    }

    // 5. 校验 decisions 白名单
    for (skill_id, decision) in &request.decisions {
        let allowed = if let Some(conflict) = plan
            .conflicts
            .iter()
            .find(|conflict| &conflict.skill_id == skill_id)
        {
            allowed_decisions(conflict.conflict_reason)
        } else {
            let entry = plan
                .entries
                .iter()
                .find(|entry| &entry.skill_id == skill_id)
                .ok_or_else(|| {
                    AppError::Blocked(format!(
                        "decision for unknown/non-deletable skill: {skill_id}"
                    ))
                })?;
            let allowed = allowed_decisions_for_status(entry.status);
            if allowed.is_empty() {
                return Err(AppError::Blocked(format!(
                    "decision for unknown/non-deletable skill: {skill_id}"
                )));
            }
            allowed
        };
        if !allowed.contains(decision) {
            return Err(AppError::Blocked(format!(
                "invalid decision for {skill_id}"
            )));
        }
    }

    for (skill_id, decision) in &request.decisions {
        if !matches!(
            decision,
            SyncDecision::RestoreRemote | SyncDecision::KeepLocal
        ) {
            continue;
        }
        let entry = plan
            .entries
            .iter()
            .find(|entry| &entry.skill_id == skill_id)
            .ok_or_else(|| {
                AppError::Blocked(format!(
                    "decision for unknown/non-deletable skill: {skill_id}"
                ))
            })?;
        if matches!(
            entry.status,
            SyncStatus::LocalDeleted | SyncStatus::RemoteDeleted
        ) && selected_set.contains(entry.action_id.as_str())
        {
            return Err(AppError::Blocked(format!(
                "conflicting delete + recovery decision: {skill_id}"
            )));
        }
    }

    // 6. delete guard
    let has_delete = request.selected_action_ids.iter().any(|a| {
        plan.entries
            .iter()
            .find(|e| &e.action_id == a)
            .map(|e| {
                matches!(
                    e.status,
                    SyncStatus::LocalDeleted | SyncStatus::RemoteDeleted
                )
            })
            .unwrap_or(false)
    }) || request
        .decisions
        .values()
        .any(|d| matches!(d, SyncDecision::DeleteRemote | SyncDecision::AcceptDelete));
    if plan.delete_guard_tripped && has_delete && !request.delete_guard_ack {
        return Err(AppError::Blocked("delete guard ack required".into()));
    }

    let mut applied: Vec<String> = Vec::new();
    let mut state_updated: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut next_manifest = prepared.manifest.clone();
    next_manifest.updated_at = now_iso();
    next_manifest.updated_by = "device".into();

    let mut uploads: Vec<UploadItem> = Vec::new();
    let mut delete_remote_ids: Vec<String> = Vec::new();
    let mut download_items: Vec<DownloadItem> = Vec::new();
    let mut delete_local_items: Vec<DeleteLocalItem> = Vec::new();

    for action_id in &request.selected_action_ids {
        let entry = plan
            .entries
            .iter()
            .find(|e| &e.action_id == action_id)
            .ok_or_else(|| AppError::Blocked(format!("unknown action_id: {action_id}")))?;
        match entry.status {
            SyncStatus::LocalUpdate => {
                let pl = prepared.packed.get(&entry.skill_id).ok_or_else(|| {
                    AppError::Blocked(format!("upload skill not packed: {}", entry.skill_id))
                })?;
                uploads.push(UploadItem {
                    skill_id: entry.skill_id.clone(),
                    name: pl.info.name.clone(),
                    description: pl.info.description.clone(),
                    namespace: entry.namespace,
                    folder_name: entry.folder_name.clone(),
                    relative_dir: entry.relative_dir.clone(),
                    hash: pl.info.hash.clone(),
                    zip_size: pl.info.zip_size,
                    zip_path: pl.zip_path.clone(),
                });
            }
            SyncStatus::RemoteUpdate => {
                let vskill = next_manifest
                    .skills
                    .get(&entry.skill_id)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::Blocked(format!("remote skill missing: {}", entry.skill_id))
                    })?;
                download_items.push(DownloadItem {
                    skill_id: entry.skill_id.clone(),
                    entry_ns: entry.namespace,
                    entry_folder: entry.folder_name.clone(),
                    vskill,
                });
            }
            SyncStatus::LocalDeleted => {
                delete_remote_ids.push(entry.skill_id.clone());
            }
            SyncStatus::RemoteDeleted => {
                delete_local_items.push(DeleteLocalItem {
                    skill_id: entry.skill_id.clone(),
                    entry_ns: entry.namespace,
                });
            }
            _ => {}
        }
    }

    for (skill_id, decision) in &request.decisions {
        let entry = plan
            .entries
            .iter()
            .find(|entry| &entry.skill_id == skill_id)
            .ok_or_else(|| AppError::Blocked(format!("decision entry missing: {skill_id}")))?;
        match decision {
            SyncDecision::KeepLocal => {
                let pl = prepared.packed.get(skill_id).ok_or_else(|| {
                    AppError::Blocked(format!("conflict keep_local not packed: {skill_id}"))
                })?;
                uploads.push(UploadItem {
                    skill_id: skill_id.clone(),
                    name: pl.info.name.clone(),
                    description: pl.info.description.clone(),
                    namespace: entry.namespace,
                    folder_name: entry.folder_name.clone(),
                    relative_dir: entry.relative_dir.clone(),
                    hash: pl.info.hash.clone(),
                    zip_size: pl.info.zip_size,
                    zip_path: pl.zip_path.clone(),
                });
            }
            SyncDecision::UseRemote | SyncDecision::RestoreRemote => {
                let vskill = next_manifest.skills.get(skill_id).cloned().ok_or_else(|| {
                    AppError::Blocked(format!("conflict remote missing: {skill_id}"))
                })?;
                download_items.push(DownloadItem {
                    skill_id: skill_id.clone(),
                    entry_ns: entry.namespace,
                    entry_folder: entry.folder_name.clone(),
                    vskill,
                });
            }
            SyncDecision::DeleteRemote => {
                delete_remote_ids.push(skill_id.clone());
            }
            SyncDecision::AcceptDelete => {
                delete_local_items.push(DeleteLocalItem {
                    skill_id: skill_id.clone(),
                    entry_ns: entry.namespace,
                });
            }
            SyncDecision::Skip => {}
        }
    }

    for conflict in &plan.conflicts {
        if !request.decisions.contains_key(&conflict.skill_id) {
            warnings.push(format!(
                "{}: conflict skipped (no decision)",
                conflict.skill_id
            ));
        }
    }

    // 已完成本地副作用（download / local-delete）与待远端发布（upload / delete-remote）的 action ID。
    let mut completed_action_ids: Vec<String> = Vec::new();
    let mut pending_action_ids: Vec<String> = Vec::new();

    // 下载（stage/verify/commit）
    for dl in &download_items {
        execute_download(store, dl, &working, home, &task_id, &config.limits).await?;
        let (ns, folder) = download_target(dl, &working)?;
        working.skills.insert(
            dl.skill_id.clone(),
            SkillSyncState {
                base_hash: dl.vskill.hash.clone(),
                last_remote_hash: dl.vskill.hash.clone(),
                last_synced_at: now_iso(),
                namespace: ns,
                relative_dir: folder,
            },
        );
        applied.push(dl.skill_id.clone());
        completed_action_ids.push(dl.skill_id.clone());
    }

    // 删本地（trash）
    for dl in &delete_local_items {
        let (ns, folder) = delete_local_target(dl, &working)?;
        let root = namespace_root(home, ns)?;
        let target = root.join(&folder);
        let trash = trash_dir(&root, &task_id, &folder);
        if target.exists() {
            if let Err(e) = move_to_trash(&target, &trash) {
                save_recovery_journal(
                    config_dir,
                    &task_id,
                    "trash_move_failed",
                    &working,
                    applied.clone(),
                    vec![dl.skill_id.clone()],
                )?;
                return Ok(ApplySyncResponse::RecoveryRequired {
                    recovery: RecoveryInfo {
                        task_id: task_id.clone(),
                        phase: RecoveryPhase::TrashMoveFailed,
                        remote_commit: None,
                        completed_action_ids: applied.clone(),
                        pending_action_ids: vec![dl.skill_id.clone()],
                        message: format!("trash move failed: {e}"),
                    },
                });
            }
        }
        applied.push(dl.skill_id.clone());
        completed_action_ids.push(dl.skill_id.clone());
        working.skills.remove(&dl.skill_id);
    }

    // 上传 + 删云端 -> next_manifest
    let mut blobs: Vec<BlobWrite> = Vec::new();
    // 内容寻址去重：相同 hash 推导出相同 blob path，多个 skill 在 manifest 中共享同一 blob；
    // git tree 每个 path 唯一，故相同内容只写入一次，避免触发远端重复路径断言。
    let mut seen_blob_paths: HashSet<String> = HashSet::new();
    for up in &uploads {
        let blob_path = blob_path_for_hash(&up.hash)?;
        if seen_blob_paths.insert(blob_path.clone()) {
            let bytes = fs::read(&up.zip_path)?;
            blobs.push(BlobWrite {
                path: blob_path.clone(),
                bytes,
                expected_hash: up.hash.clone(),
            });
        }
        next_manifest.skills.insert(
            up.skill_id.clone(),
            VaultSkill {
                id: up.skill_id.clone(),
                name: up.name.clone(),
                description: up.description.clone(),
                namespace: up.namespace,
                folder_name: up.folder_name.clone(),
                hash: up.hash.clone(),
                blob: blob_path,
                size: up.zip_size,
                updated_at: now_iso(),
                updated_by: "device".into(),
            },
        );
        applied.push(up.skill_id.clone());
        pending_action_ids.push(up.skill_id.clone());
        working.skills.insert(
            up.skill_id.clone(),
            SkillSyncState {
                base_hash: up.hash.clone(),
                last_remote_hash: up.hash.clone(),
                last_synced_at: now_iso(),
                namespace: up.namespace,
                relative_dir: up
                    .relative_dir
                    .clone()
                    .unwrap_or_else(|| up.folder_name.clone()),
            },
        );
    }
    for id in &delete_remote_ids {
        next_manifest.skills.remove(id);
        applied.push(id.clone());
        pending_action_ids.push(id.clone());
        working.skills.remove(id);
    }

    // adoptions / removals（纯状态转移，移到远端提交之前，确保预期状态完整）
    for adoption in &plan.base_adoptions {
        let entry = plan
            .entries
            .iter()
            .find(|e| e.skill_id == adoption.skill_id);
        let (ns, rel) = entry
            .map(|e| {
                (
                    e.namespace,
                    e.relative_dir
                        .clone()
                        .unwrap_or_else(|| e.folder_name.clone()),
                )
            })
            .unwrap_or((SkillNamespace::Agents, adoption.skill_id.clone()));
        working.skills.insert(
            adoption.skill_id.clone(),
            SkillSyncState {
                base_hash: adoption.hash.clone(),
                last_remote_hash: adoption.hash.clone(),
                last_synced_at: now_iso(),
                namespace: ns,
                relative_dir: rel,
            },
        );
        state_updated.push(adoption.skill_id.clone());
    }
    for id in &plan.base_removals {
        working.skills.remove(id);
        state_updated.push(id.clone());
    }

    // 远端提交（uploads + delete_remote 合并一次）。先持久化完整预期状态再发起提交；
    // working.remote.commit_sha 暂留 base，直到拿到 candidate。
    let remote_commit: Option<String> = if !uploads.is_empty() || !delete_remote_ids.is_empty() {
        let base_commit = prepared.expected_commit.clone();
        // 预期状态的 remote.commit_sha 暂留 base，直到拿到 candidate。
        working.remote.commit_sha = base_commit.clone();
        let intended_manifest_hash = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(&next_manifest.validated_bytes()?))
        );
        let intended_state_bytes = serde_json::to_vec(&working)
            .map_err(|e| AppError::Vault(format!("state serialize failed: {e}")))?;
        let intended_state_hash = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(&intended_state_bytes))
        );
        let journal = ApplyJournal {
            schema: 1,
            task_id: task_id.clone(),
            phase: "remote_committing".into(),
            remote_candidate: None,
            remote_base: Some(base_commit.clone()),
            next_manifest_hash: Some(intended_manifest_hash.clone()),
            completed_action_ids: completed_action_ids.clone(),
            pending_action_ids: pending_action_ids.clone(),
            next_state_hash: intended_state_hash.clone(),
            next_state_bytes: intended_state_bytes.clone(),
        };
        save_journal(config_dir, &journal)?;

        let changes = RemoteChanges {
            base_commit_sha: base_commit.clone(),
            blobs,
            next_manifest: next_manifest.clone(),
            commit_message: "skill-sync: apply".into(),
        };
        match store.commit_changes(changes).await {
            Ok(c) => Some(c.commit_sha),
            Err(AppError::RemoteChanged(_)) => {
                clear_journal(config_dir)?;
                return Ok(ApplySyncResponse::PlanChanged {
                    reason: PlanChangeReason::RemoteChanged,
                    latest_plan: Box::new(prepared.plan),
                });
            }
            Err(AppError::RemoteOutcomeUnknown {
                candidate_commit_sha,
                ..
            }) => {
                let journal = ApplyJournal {
                    schema: 1,
                    task_id: task_id.clone(),
                    phase: "remote_outcome_unknown".into(),
                    remote_candidate: Some(candidate_commit_sha.clone()),
                    remote_base: Some(base_commit.clone()),
                    next_manifest_hash: Some(intended_manifest_hash.clone()),
                    completed_action_ids: completed_action_ids.clone(),
                    pending_action_ids: pending_action_ids.clone(),
                    next_state_hash: intended_state_hash.clone(),
                    next_state_bytes: intended_state_bytes.clone(),
                };
                save_journal(config_dir, &journal)?;
                return Ok(ApplySyncResponse::RecoveryRequired {
                    recovery: RecoveryInfo {
                        task_id: task_id.clone(),
                        phase: RecoveryPhase::RemoteOutcomeUnknown,
                        remote_commit: Some(candidate_commit_sha),
                        completed_action_ids: completed_action_ids.clone(),
                        pending_action_ids: pending_action_ids.clone(),
                        message: format!("remote outcome unknown (base={base_commit})"),
                    },
                });
            }
            Err(e) => {
                clear_journal(config_dir)?;
                return Err(e);
            }
        }
    } else {
        None
    };

    working.remote.commit_sha = remote_commit
        .clone()
        .unwrap_or_else(|| prepared.expected_commit.clone());

    // 保存 sync_state（durable）；先写 state_saving journal 以便 StateSaveFailed 恢复
    let next_state_bytes = serde_json::to_vec(&working)
        .map_err(|e| AppError::Vault(format!("state serialize failed: {e}")))?;
    let journal = ApplyJournal {
        schema: 1,
        task_id: task_id.clone(),
        phase: "state_saving".into(),
        remote_candidate: remote_commit.clone(),
        next_state_hash: format!("sha256:{}", hex::encode(Sha256::digest(&next_state_bytes))),
        next_state_bytes,
        remote_base: None,
        next_manifest_hash: None,
        completed_action_ids: Vec::new(),
        pending_action_ids: Vec::new(),
    };
    save_journal(config_dir, &journal)?;
    let state_to_save = working.clone();
    let state_dir = config_dir.to_path_buf();
    let save_result =
        tauri::async_runtime::spawn_blocking(move || state_to_save.save_to(&state_dir))
            .await
            .map_err(|e| AppError::Vault(format!("state save task failed: {e}")))?;
    if let Err(e) = save_result {
        return Ok(ApplySyncResponse::RecoveryRequired {
            recovery: RecoveryInfo {
                task_id,
                phase: RecoveryPhase::StateSaveFailed,
                remote_commit: remote_commit.clone(),
                completed_action_ids: applied.clone(),
                pending_action_ids: Vec::new(),
                message: format!("state save failed: {e}"),
            },
        });
    }
    clear_journal(config_dir)?;

    if !cleanup_task_artifacts(home, &task_id) {
        warnings.push("cleanup_pending".into());
    }

    *state = working;
    Ok(ApplySyncResponse::Applied {
        result: ApplyResult {
            applied,
            state_updated,
            warnings,
            remote_commit,
        },
    })
}

/// 批量上传：重新生成计划，选择匹配 skill_id 的 upload 动作后 apply。
pub(crate) async fn upload_skills<S: RemoteStore>(
    skill_ids: &[String],
    config: &AppConfig,
    state: &mut SyncState,
    store: &S,
    home: &Path,
    config_dir: &Path,
) -> Result<ApplySyncResponse> {
    batch_apply(
        skill_ids,
        config,
        state,
        store,
        home,
        config_dir,
        SyncStatus::LocalUpdate,
    )
    .await
}

/// 批量下载：重新生成计划，选择匹配 skill_id 的 download 动作后 apply。
pub(crate) async fn download_skills<S: RemoteStore>(
    skill_ids: &[String],
    config: &AppConfig,
    state: &mut SyncState,
    store: &S,
    home: &Path,
    config_dir: &Path,
) -> Result<ApplySyncResponse> {
    batch_apply(
        skill_ids,
        config,
        state,
        store,
        home,
        config_dir,
        SyncStatus::RemoteUpdate,
    )
    .await
}

async fn batch_apply<S: RemoteStore>(
    skill_ids: &[String],
    config: &AppConfig,
    state: &mut SyncState,
    store: &S,
    home: &Path,
    config_dir: &Path,
    want: SyncStatus,
) -> Result<ApplySyncResponse> {
    let prepared = prepare_plan(config, state, store, home).await?;
    let plan = &prepared.plan;
    let want_kind = action_kind_for(want);
    let mut selected: Vec<String> = Vec::new();
    for id in skill_ids {
        if plan
            .entries
            .iter()
            .any(|e| &e.skill_id == id && e.status == want)
        {
            selected.push(format!("{want_kind}:{id}"));
        }
    }
    let request = ApplySyncRequest {
        expected_remote_commit: plan.expected_remote_commit.clone(),
        plan_fingerprint: plan.plan_fingerprint.clone(),
        selected_action_ids: selected,
        decisions: HashMap::new(),
        delete_guard_ack: false,
    };
    drop(prepared);
    apply_plan(config, state, &request, store, home, config_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RemoteConfig;
    use crate::remote_store::{RemoteChanges, RemoteCommit, RemoteSnapshot};
    use crate::sync_state::RemoteIdentity;
    use SkillNamespace::*;

    const HASH_B: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";

    fn remote_cfg(install: u64, repo: u64, branch: &str) -> RemoteConfig {
        RemoteConfig {
            installation_id: install,
            repository_id: repo,
            owner: "o".into(),
            repo: "r".into(),
            branch: branch.into(),
        }
    }

    fn remote_identity(install: u64, repo: u64, branch: &str) -> RemoteIdentity {
        RemoteIdentity {
            provider: "github".into(),
            installation_id: install,
            repository_id: repo,
            owner: "o".into(),
            repo: "r".into(),
            branch: branch.into(),
            commit_sha: "c".into(),
        }
    }
    // ---- Task 9: apply_plan 测试 ----

    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    enum CommitMode {
        #[default]
        Ok,
        OutcomeUnknown,
        DefiniteError,
    }

    /// commit_changes 期间的文件系统突变，用于验证 journal 持久化失败传播。
    #[derive(Default)]
    enum JournalMutation {
        #[default]
        None,
        /// 把 apply-transaction.json 替换为目录后返回 definite error，使 clear_journal 失败。
        BreakClearOnDefiniteError,
        /// 把 config dir 替换为文件后返回 OutcomeUnknown，使 unknown-phase save_journal 失败。
        BreakSaveOnUnknown,
        /// 把 apply-transaction.json 替换为目录后返回 success，使 state_saving save_journal 失败。
        BreakStateSavingOnSuccess,
    }

    struct ApplyMockStore {
        manifest: VaultManifest,
        commit: String,
        blobs: HashMap<String, Vec<u8>>,
        commit_count: Arc<Mutex<usize>>,
        commit_mode: CommitMode,
        captured_manifest: Arc<Mutex<Option<VaultManifest>>>,
        config_dir: Option<PathBuf>,
        mutation: JournalMutation,
    }

    impl Default for ApplyMockStore {
        fn default() -> Self {
            Self {
                manifest: VaultManifest::empty("d"),
                commit: "commit-1".into(),
                blobs: HashMap::new(),
                commit_count: Arc::new(Mutex::new(0)),
                commit_mode: CommitMode::Ok,
                captured_manifest: Arc::new(Mutex::new(None)),
                config_dir: None,
                mutation: JournalMutation::None,
            }
        }
    }

    #[async_trait::async_trait]
    impl RemoteStore for ApplyMockStore {
        async fn fetch_manifest(&self) -> Result<RemoteSnapshot> {
            Ok(RemoteSnapshot {
                manifest: self.manifest.clone(),
                commit_sha: self.commit.clone(),
            })
        }
        async fn fetch_blob(&self, path: &str, _expected: &str) -> Result<Vec<u8>> {
            self.blobs
                .get(path)
                .cloned()
                .ok_or_else(|| AppError::Vault(format!("blob not found: {path}")))
        }
        async fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit> {
            let count = {
                let mut c = self.commit_count.lock().unwrap();
                *c += 1;
                *c
            };
            *self.captured_manifest.lock().unwrap() = Some(changes.next_manifest.clone());

            if let Some(dir) = &self.config_dir {
                match self.mutation {
                    JournalMutation::BreakClearOnDefiniteError => {
                        let path = crate::local_apply::journal_path(dir);
                        drop(std::fs::remove_file(&path));
                        drop(std::fs::create_dir(&path));
                        return Err(AppError::Vault("definite commit error".into()));
                    }
                    JournalMutation::BreakSaveOnUnknown => {
                        drop(std::fs::remove_dir_all(dir));
                        drop(std::fs::File::create(dir));
                        return Err(AppError::RemoteOutcomeUnknown {
                            base_commit_sha: self.commit.clone(),
                            candidate_commit_sha: "candidate".into(),
                        });
                    }
                    JournalMutation::BreakStateSavingOnSuccess => {
                        let path = crate::local_apply::journal_path(dir);
                        drop(std::fs::remove_file(&path));
                        drop(std::fs::create_dir(&path));
                        return Ok(RemoteCommit {
                            commit_sha: "candidate".into(),
                        });
                    }
                    JournalMutation::None => {}
                }
            }

            match self.commit_mode {
                CommitMode::Ok => Ok(RemoteCommit {
                    commit_sha: format!("commit-{count}"),
                }),
                CommitMode::OutcomeUnknown => Err(AppError::RemoteOutcomeUnknown {
                    base_commit_sha: self.commit.clone(),
                    candidate_commit_sha: "candidate".into(),
                }),
                CommitMode::DefiniteError => Err(AppError::Vault("definite commit error".into())),
            }
        }
    }

    fn apply_config() -> AppConfig {
        let mut c = AppConfig::default_config();
        c.remote = Some(remote_cfg(1, 10, "main"));
        c.limits = LimitsConfig::default();
        c
    }

    fn apply_state() -> SyncState {
        SyncState::empty(remote_identity(1, 10, "main"))
    }

    fn temp_home() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        for ns in [Agents, Codex, ClaudeCode] {
            let root = namespace_root(home.path(), ns).unwrap();
            std::fs::create_dir_all(&root).unwrap();
        }
        home
    }

    fn make_skill(home: &Path, ns: SkillNamespace, folder: &str, name: &str) {
        let root = namespace_root(home, ns).unwrap();
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: d\n---\nbody"),
        )
        .unwrap();
    }

    /// 在独立 temp home 中打包一个 skill，返回 (VaultSkill, blob_path, zip_bytes)。
    fn make_blob(ns: SkillNamespace, folder: &str, name: &str) -> (VaultSkill, String, Vec<u8>) {
        let pack_home = tempfile::tempdir().unwrap();
        for n in [Agents, Codex, ClaudeCode] {
            std::fs::create_dir_all(namespace_root(pack_home.path(), n).unwrap()).unwrap();
        }
        make_skill(pack_home.path(), ns, folder, name);
        let source = namespace_root(pack_home.path(), ns).unwrap().join(folder);
        let inputs = vec![SkillPackInput {
            source_path: source,
        }];
        let batch = SkillPacker::pack_batch(
            &inputs,
            &PackOptions {
                limits: LimitsConfig::default(),
                user_ignore: vec![],
            },
        )
        .unwrap();
        let (hash, bytes) = match &batch.outcomes[0] {
            PackOutcome::Packed(p) => (p.hash.clone(), std::fs::read(&p.zip_path).unwrap()),
            _ => panic!("pack failed"),
        };
        let hex = &hash["sha256:".len()..];
        let blob_path = format!("blobs/sha256/{hex}.skill.zip");
        let vskill = VaultSkill {
            id: skill_id(ns, name),
            name: name.into(),
            description: "d".into(),
            namespace: ns,
            folder_name: folder.into(),
            hash: hash.clone(),
            blob: blob_path.clone(),
            size: bytes.len() as u64,
            updated_at: String::new(),
            updated_by: "d".into(),
        };
        (vskill, blob_path, bytes)
    }

    fn action_id_of(plan: &SyncPlan, skill_id: &str) -> String {
        plan.entries
            .iter()
            .find(|e| e.skill_id == skill_id)
            .unwrap()
            .action_id
            .clone()
    }

    fn apply_request(
        plan: &SyncPlan,
        selected: Vec<String>,
        decisions: HashMap<String, SyncDecision>,
        ack: bool,
    ) -> ApplySyncRequest {
        ApplySyncRequest {
            expected_remote_commit: plan.expected_remote_commit.clone(),
            plan_fingerprint: plan.plan_fingerprint.clone(),
            selected_action_ids: selected,
            decisions,
            delete_guard_ack: ack,
        }
    }

    fn mock_store(manifest: VaultManifest) -> ApplyMockStore {
        ApplyMockStore {
            manifest,
            commit: "commit-1".into(),
            blobs: HashMap::new(),
            commit_count: Arc::new(Mutex::new(0)),
            commit_mode: CommitMode::Ok,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn applying_local_updates_writes_one_remote_commit() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let config = apply_config();
        let mut state = apply_state();
        let store = mock_store(VaultManifest::empty("d"));
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            false,
        );
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.remote_commit.is_some());
                assert_eq!(*store.commit_count.lock().unwrap(), 1);
                assert!(state.skills.contains_key("codex:demo"));
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn uploading_identical_skills_across_namespaces_dedupes_single_blob() {
        let home = temp_home();
        // 两个不同 namespace 下内容完全相同的 skill：skill_id 不同，但打包 hash 相同。
        make_skill(home.path(), Codex, "demo", "demo");
        make_skill(home.path(), Agents, "demo", "demo");
        let config = apply_config();
        let mut state = apply_state();
        let store = mock_store(VaultManifest::empty("d"));
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(
            &plan,
            vec![
                action_id_of(&plan, "codex:demo"),
                action_id_of(&plan, "agents:demo"),
            ],
            HashMap::new(),
            false,
        );
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                // 去重后仅一次远端提交：一个 blob + manifest 含两个 skill。
                assert_eq!(*store.commit_count.lock().unwrap(), 1);
                assert!(result.remote_commit.is_some());
                assert!(state.skills.contains_key("codex:demo"));
                assert!(state.skills.contains_key("agents:demo"));
                // manifest 中两个 skill 共享同一 blob path。
                let captured = store.captured_manifest.lock().unwrap().clone().unwrap();
                let codex = captured.skills.get("codex:demo").unwrap();
                let agents = captured.skills.get("agents:demo").unwrap();
                assert_eq!(codex.hash, agents.hash);
                assert_eq!(codex.blob, agents.blob);
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn download_only_creates_zero_remote_commits() {
        let home = temp_home();
        let (vskill, blob_path, bytes) = make_blob(Codex, "demo", "demo");
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill);
        let mut blobs = HashMap::new();
        blobs.insert(blob_path, bytes);
        let store = ApplyMockStore {
            manifest,
            commit: "commit-1".into(),
            blobs,
            commit_count: Arc::new(Mutex::new(0)),
            commit_mode: CommitMode::Ok,
            ..Default::default()
        };
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            false,
        );
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.remote_commit.is_none());
                assert_eq!(*store.commit_count.lock().unwrap(), 0);
                assert!(state.skills.contains_key("codex:demo"));
                let target = namespace_root(home.path(), Codex).unwrap().join("demo");
                assert!(target.join("SKILL.md").exists());
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn local_deleted_removes_manifest_entry_in_one_commit() {
        let home = temp_home();
        let (vskill, _, _) = make_blob(Codex, "demo", "demo");
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill.clone());
        let store = mock_store(manifest);
        let config = apply_config();
        let mut state = apply_state();
        state.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: vskill.hash.clone(),
                last_remote_hash: vskill.hash.clone(),
                last_synced_at: String::new(),
                namespace: Codex,
                relative_dir: "demo".into(),
            },
        );
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            true,
        );
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.remote_commit.is_some());
                assert_eq!(*store.commit_count.lock().unwrap(), 1);
                assert!(!state.skills.contains_key("codex:demo"));
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn remote_deleted_moves_local_skill_to_trash_with_zero_commit() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let (vskill, _, _) = make_blob(Codex, "demo", "demo");
        let store = mock_store(VaultManifest::empty("d"));
        let config = apply_config();
        let mut state = apply_state();
        state.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: vskill.hash.clone(),
                last_remote_hash: vskill.hash.clone(),
                last_synced_at: String::new(),
                namespace: Codex,
                relative_dir: "demo".into(),
            },
        );
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            true,
        );
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.remote_commit.is_none());
                assert_eq!(*store.commit_count.lock().unwrap(), 0);
                assert!(!state.skills.contains_key("codex:demo"));
                // target 移入 trash（trash 目录存在内容）
                let root = namespace_root(home.path(), Codex).unwrap();
                assert!(!root.join("demo").exists());
                assert!(root
                    .join(".skill-sync-trash")
                    .read_dir()
                    .unwrap()
                    .next()
                    .is_some());
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn local_deleted_with_restore_remote_downloads_and_aligns_state() {
        let home = temp_home();
        let (vskill, blob_path, bytes) = make_blob(Codex, "demo", "demo");
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill.clone());
        let mut blobs = HashMap::new();
        blobs.insert(blob_path, bytes);
        let store = ApplyMockStore {
            manifest,
            blobs,
            ..Default::default()
        };
        let config = apply_config();
        let mut state = apply_state();
        state.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: vskill.hash.clone(),
                last_remote_hash: vskill.hash.clone(),
                last_synced_at: String::new(),
                namespace: Codex,
                relative_dir: "demo".into(),
            },
        );
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let mut decisions = HashMap::new();
        decisions.insert("codex:demo".into(), SyncDecision::RestoreRemote);
        let req = apply_request(&plan, vec![], decisions, false);
        let cfgdir = tempfile::tempdir().unwrap();

        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();

        match resp {
            ApplySyncResponse::Applied { result } => {
                assert_eq!(result.applied, vec!["codex:demo"]);
                assert!(result.remote_commit.is_none());
                assert_eq!(*store.commit_count.lock().unwrap(), 0);
                assert_eq!(
                    state.skills.get("codex:demo").unwrap().base_hash,
                    vskill.hash
                );
                let target = namespace_root(home.path(), Codex).unwrap().join("demo");
                assert!(target.join("SKILL.md").exists());
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn remote_deleted_with_keep_local_uploads_and_aligns_state() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let (vskill, _, _) = make_blob(Codex, "demo", "demo");
        let store = mock_store(VaultManifest::empty("d"));
        let config = apply_config();
        let mut state = apply_state();
        state.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: vskill.hash.clone(),
                last_remote_hash: vskill.hash.clone(),
                last_synced_at: String::new(),
                namespace: Codex,
                relative_dir: "demo".into(),
            },
        );
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        let local_hash = plan
            .entries
            .iter()
            .find(|entry| entry.skill_id == "codex:demo")
            .and_then(|entry| entry.local_hash.clone())
            .unwrap();
        drop(prepared);
        let mut decisions = HashMap::new();
        decisions.insert("codex:demo".into(), SyncDecision::KeepLocal);
        let req = apply_request(&plan, vec![], decisions, false);
        let cfgdir = tempfile::tempdir().unwrap();

        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();

        match resp {
            ApplySyncResponse::Applied { result } => {
                assert_eq!(result.applied, vec!["codex:demo"]);
                assert!(result.remote_commit.is_some());
                assert_eq!(*store.commit_count.lock().unwrap(), 1);
                assert_eq!(
                    state.skills.get("codex:demo").unwrap().base_hash,
                    local_hash
                );
                let captured = store.captured_manifest.lock().unwrap().clone().unwrap();
                assert_eq!(captured.skills.get("codex:demo").unwrap().hash, local_hash);
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn local_deleted_with_delete_remote_via_decision_equivalent_to_selected() {
        let home = temp_home();
        let (vskill, _, _) = make_blob(Codex, "demo", "demo");
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill.clone());
        let store = mock_store(manifest);
        let config = apply_config();
        let mut state = apply_state();
        state.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: vskill.hash.clone(),
                last_remote_hash: vskill.hash,
                last_synced_at: String::new(),
                namespace: Codex,
                relative_dir: "demo".into(),
            },
        );
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let mut decisions = HashMap::new();
        decisions.insert("codex:demo".into(), SyncDecision::DeleteRemote);
        let req = apply_request(&plan, vec![], decisions, true);
        let cfgdir = tempfile::tempdir().unwrap();

        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();

        match resp {
            ApplySyncResponse::Applied { result } => {
                assert_eq!(result.applied, vec!["codex:demo"]);
                assert!(result.remote_commit.is_some());
                assert_eq!(*store.commit_count.lock().unwrap(), 1);
                assert!(!state.skills.contains_key("codex:demo"));
                let captured = store.captured_manifest.lock().unwrap().clone().unwrap();
                assert!(!captured.skills.contains_key("codex:demo"));
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn recovery_decision_conflicting_with_selected_delete_blocks() {
        let home = temp_home();
        let (vskill, _, _) = make_blob(Codex, "demo", "demo");
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill.clone());
        let store = mock_store(manifest);
        let config = apply_config();
        let mut state = apply_state();
        state.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: vskill.hash.clone(),
                last_remote_hash: vskill.hash,
                last_synced_at: String::new(),
                namespace: Codex,
                relative_dir: "demo".into(),
            },
        );
        let original_state = state.clone();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let mut decisions = HashMap::new();
        decisions.insert("codex:demo".into(), SyncDecision::RestoreRemote);
        let req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            decisions,
            true,
        );
        let cfgdir = tempfile::tempdir().unwrap();

        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();

        assert!(
            matches!(err, AppError::Blocked(message) if message == "conflicting delete + recovery decision: codex:demo")
        );
        assert_eq!(*store.commit_count.lock().unwrap(), 0);
        assert_eq!(state, original_state);
        assert!(!namespace_root(home.path(), Codex)
            .unwrap()
            .join("demo")
            .exists());
    }

    #[tokio::test]
    async fn both_deleted_removes_sync_state_base() {
        let home = temp_home();
        let store = mock_store(VaultManifest::empty("d"));
        let config = apply_config();
        let mut state = apply_state();
        state.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: HASH_B.into(),
                last_remote_hash: HASH_B.into(),
                last_synced_at: String::new(),
                namespace: Codex,
                relative_dir: "demo".into(),
            },
        );
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(&plan, vec![], HashMap::new(), false);
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.applied.is_empty());
                assert_eq!(result.state_updated, vec!["codex:demo".to_string()]);
                assert!(result.remote_commit.is_none());
                assert_eq!(*store.commit_count.lock().unwrap(), 0);
                assert!(!state.skills.contains_key("codex:demo"));
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn adoption_only_apply_writes_base_with_zero_remote_commits() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let (vskill, blob_path, bytes) = make_blob(Codex, "demo", "demo");
        // local hash == remote hash（同一 skill 打包两次，bytes 相同 -> hash 相同）
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill);
        let mut blobs = HashMap::new();
        blobs.insert(blob_path, bytes);
        let store = ApplyMockStore {
            manifest,
            commit: "commit-1".into(),
            blobs,
            commit_count: Arc::new(Mutex::new(0)),
            commit_mode: CommitMode::Ok,
            ..Default::default()
        };
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        // adoption: base 空，local==remote -> synced + adoption。selected 为空。
        let req = apply_request(&plan, vec![], HashMap::new(), false);
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.applied.is_empty());
                assert_eq!(result.state_updated, vec!["codex:demo".to_string()]);
                assert!(result.remote_commit.is_none());
                assert_eq!(*store.commit_count.lock().unwrap(), 0);
                assert!(state.skills.contains_key("codex:demo"));
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn remote_change_after_preview_returns_latest_plan_without_side_effect() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let store = mock_store(VaultManifest::empty("d"));
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        // 用错误的 expected_remote_commit 触发 RemoteChanged
        let mut req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            false,
        );
        req.expected_remote_commit = "stale-commit".into();
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::PlanChanged { reason, .. } => {
                assert_eq!(reason, PlanChangeReason::RemoteChanged);
                assert_eq!(*store.commit_count.lock().unwrap(), 0);
                assert!(state.skills.is_empty()); // state 不变
            }
            _ => panic!("expected PlanChanged"),
        }
    }

    #[tokio::test]
    async fn stale_plan_returns_latest_plan_before_persistent_side_effect() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let store = mock_store(VaultManifest::empty("d"));
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let mut req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            false,
        );
        req.plan_fingerprint = "sha256:stale".into();
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::PlanChanged { reason, .. } => {
                assert_eq!(reason, PlanChangeReason::PlanChanged);
                assert_eq!(*store.commit_count.lock().unwrap(), 0);
                assert!(state.skills.is_empty());
            }
            _ => panic!("expected PlanChanged"),
        }
    }

    #[tokio::test]
    async fn unselected_upload_action_is_not_applied() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let store = mock_store(VaultManifest::empty("d"));
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        // 不选择 upload -> 0 commit，state 不变
        let req = apply_request(&plan, vec![], HashMap::new(), false);
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.applied.is_empty());
                assert_eq!(*store.commit_count.lock().unwrap(), 0);
                assert!(state.skills.is_empty());
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn duplicate_selected_action_id_blocks_whole_apply() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let store = mock_store(VaultManifest::empty("d"));
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let id = action_id_of(&plan, "codex:demo");
        let req = apply_request(&plan, vec![id.clone(), id], HashMap::new(), false);
        let cfgdir = tempfile::tempdir().unwrap();
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Blocked(_)));
        assert_eq!(*store.commit_count.lock().unwrap(), 0);
        assert!(state.skills.is_empty());
    }

    #[tokio::test]
    async fn unknown_action_id_blocks_whole_apply() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let store = mock_store(VaultManifest::empty("d"));
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(
            &plan,
            vec!["upload:codex:nonexistent".into()],
            HashMap::new(),
            false,
        );
        let cfgdir = tempfile::tempdir().unwrap();
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Blocked(_)));
    }

    #[tokio::test]
    async fn extra_conflict_decision_blocks_whole_apply() {
        let home = temp_home();
        // local+remote 不同 -> same_name conflict (base 空)
        make_skill(home.path(), Codex, "demo", "demo");
        let (mut vskill, _, _) = make_blob(Codex, "demo", "demo");
        vskill.hash = "sha256:99999999".into(); // 不同 hash -> conflict
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill);
        let store = mock_store(manifest);
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        // 为不存在的 skill 提供决策
        let mut decisions = HashMap::new();
        decisions.insert("codex:other".into(), SyncDecision::KeepLocal);
        let req = apply_request(&plan, vec![], decisions, false);
        let cfgdir = tempfile::tempdir().unwrap();
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Blocked(_)));
    }

    #[tokio::test]
    async fn missing_conflict_decision_skips_with_warning() {
        let home = temp_home();
        make_skill(home.path(), Codex, "demo", "demo");
        let (mut vskill, _, _) = make_blob(Codex, "demo", "demo");
        vskill.hash = "sha256:99999999".into();
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill);
        let store = mock_store(manifest);
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        // 不提供 decision -> 跳过 + warning，不 block
        let req = apply_request(&plan, vec![], HashMap::new(), false);
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.applied.is_empty());
                assert!(result
                    .warnings
                    .iter()
                    .any(|w| w.contains("conflict skipped")));
            }
            _ => panic!("expected Applied"),
        }
    }

    #[tokio::test]
    async fn download_rejects_hash_mismatch_leaves_target_unchanged() {
        let home = temp_home();
        let (vskill, blob_path, _) = make_blob(Codex, "demo", "demo");
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill.clone());
        let mut blobs = HashMap::new();
        blobs.insert(blob_path, b"corrupt-bytes".to_vec()); // hash 不匹配
        let store = ApplyMockStore {
            manifest,
            commit: "commit-1".into(),
            blobs,
            commit_count: Arc::new(Mutex::new(0)),
            commit_mode: CommitMode::Ok,
            ..Default::default()
        };
        let config = apply_config();
        let mut state = apply_state();
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            false,
        );
        let cfgdir = tempfile::tempdir().unwrap();
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Blocked(_)));
        let target = namespace_root(home.path(), Codex).unwrap().join("demo");
        assert!(!target.exists()); // target 未写入
        assert!(state.skills.is_empty());
    }

    #[tokio::test]
    async fn missing_remote_config_requires_onboarding_apply() {
        let home = temp_home();
        let config = AppConfig::default_config(); // remote None
        let mut state = apply_state();
        let store = mock_store(VaultManifest::empty("d"));
        let req = ApplySyncRequest {
            expected_remote_commit: "c".into(),
            plan_fingerprint: "fp".into(),
            selected_action_ids: vec![],
            decisions: HashMap::new(),
            delete_guard_ack: false,
        };
        let cfgdir = tempfile::tempdir().unwrap();
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::NotConfigured(_)));
    }

    #[tokio::test]
    async fn state_remote_identity_mismatch_blocks_apply() {
        let home = temp_home();
        let mut config = apply_config();
        config.remote = Some(remote_cfg(2, 10, "main")); // installation_id 不同
        let mut state = apply_state(); // identity(1,10,main)
        let store = mock_store(VaultManifest::empty("d"));
        let req = ApplySyncRequest {
            expected_remote_commit: "c".into(),
            plan_fingerprint: "fp".into(),
            selected_action_ids: vec![],
            decisions: HashMap::new(),
            delete_guard_ack: false,
        };
        let cfgdir = tempfile::tempdir().unwrap();
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Blocked(_)));
    }

    /// 构造单 upload apply 场景：本地有 codex:demo，远端空 manifest。
    async fn single_upload_request(
        home: &Path,
        store: &ApplyMockStore,
    ) -> (AppConfig, SyncState, ApplySyncRequest) {
        make_skill(home, Codex, "demo", "demo");
        let config = apply_config();
        let state = apply_state();
        let prepared = prepare_plan(&config, &state, store, home).await.unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        let req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            false,
        );
        (config, state, req)
    }

    #[tokio::test]
    async fn definite_commit_error_clears_journal_and_preserves_error() {
        let home = temp_home();
        let store = ApplyMockStore {
            manifest: VaultManifest::empty("d"),
            commit_mode: CommitMode::DefiniteError,
            ..Default::default()
        };
        let (config, mut state, req) = single_upload_request(home.path(), &store).await;
        let cfgdir = tempfile::tempdir().unwrap();
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Vault(msg) if msg.contains("definite commit error")));
        assert_eq!(*store.commit_count.lock().unwrap(), 1);
        assert!(crate::local_apply::load_pending(cfgdir.path()).is_none());
    }

    #[tokio::test]
    async fn unknown_outcome_persists_complete_recovery_evidence() {
        let home = temp_home();
        let store = ApplyMockStore {
            manifest: VaultManifest::empty("d"),
            commit_mode: CommitMode::OutcomeUnknown,
            ..Default::default()
        };
        let (config, mut state, req) = single_upload_request(home.path(), &store).await;
        let cfgdir = tempfile::tempdir().unwrap();
        let resp = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap();
        let recovery = match resp {
            ApplySyncResponse::RecoveryRequired { recovery } => recovery,
            _ => panic!("expected RecoveryRequired"),
        };
        assert_eq!(recovery.phase, RecoveryPhase::RemoteOutcomeUnknown);
        assert_eq!(recovery.remote_commit.as_deref(), Some("candidate"));
        assert_eq!(recovery.completed_action_ids, Vec::<String>::new());
        assert_eq!(recovery.pending_action_ids, vec!["codex:demo".to_string()]);

        let journal = crate::local_apply::load_pending(cfgdir.path()).expect("journal retained");
        assert_eq!(journal.phase, "remote_outcome_unknown");
        assert_eq!(journal.remote_candidate.as_deref(), Some("candidate"));
        assert_eq!(journal.remote_base.as_deref(), Some("commit-1"));
        assert_eq!(journal.completed_action_ids, Vec::<String>::new());
        assert_eq!(journal.pending_action_ids, vec!["codex:demo".to_string()]);
        let captured = store.captured_manifest.lock().unwrap().clone().unwrap();
        let expected_hash = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(captured.validated_bytes().unwrap()))
        );
        assert_eq!(
            journal.next_manifest_hash.as_deref(),
            Some(expected_hash.as_str())
        );
        let intended: SyncState = serde_json::from_slice(&journal.next_state_bytes).unwrap();
        assert!(intended.skills.contains_key("codex:demo"));
        assert_eq!(intended.remote.commit_sha, "commit-1");
    }

    #[tokio::test]
    async fn initial_journal_save_failure_skips_remote_commit() {
        let home = temp_home();
        let store = ApplyMockStore {
            manifest: VaultManifest::empty("d"),
            commit_mode: CommitMode::DefiniteError,
            ..Default::default()
        };
        let (config, mut state, req) = single_upload_request(home.path(), &store).await;
        let cfgdir = tempfile::tempdir().unwrap();
        // config_dir 指向一个文件 -> save_journal(create_dir_all) 失败
        let bad_config_dir = cfgdir.path().join("config-file");
        std::fs::write(&bad_config_dir, b"x").unwrap();
        let _ = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            &bad_config_dir,
        )
        .await
        .unwrap_err();
        assert_eq!(*store.commit_count.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn clear_failure_on_definite_error_propagates_and_retains_journal_path() {
        let home = temp_home();
        let cfgdir = tempfile::tempdir().unwrap();
        let store = ApplyMockStore {
            manifest: VaultManifest::empty("d"),
            commit_mode: CommitMode::DefiniteError,
            config_dir: Some(cfgdir.path().to_path_buf()),
            mutation: JournalMutation::BreakClearOnDefiniteError,
            ..Default::default()
        };
        let (config, mut state, req) = single_upload_request(home.path(), &store).await;
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        // clear_journal 失败 -> 返回持久化错误（Io），而非原始 definite error
        assert!(matches!(err, AppError::Io(_)));
        // journal 路径仍存在（现在是目录）
        let journal_path = crate::local_apply::journal_path(cfgdir.path());
        assert!(journal_path.exists() && journal_path.is_dir());
    }

    #[tokio::test]
    async fn unknown_save_failure_wins_over_recovery_response() {
        let home = temp_home();
        let cfgdir = tempfile::tempdir().unwrap();
        let store = ApplyMockStore {
            manifest: VaultManifest::empty("d"),
            commit_mode: CommitMode::OutcomeUnknown,
            config_dir: Some(cfgdir.path().to_path_buf()),
            mutation: JournalMutation::BreakSaveOnUnknown,
            ..Default::default()
        };
        let (config, mut state, req) = single_upload_request(home.path(), &store).await;
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        // unknown-phase save_journal 失败 -> 持久化错误胜出，而非 RecoveryRequired
        assert!(matches!(err, AppError::Io(_)));
        // config dir 现在是文件 -> sync_state 未保存
        assert!(cfgdir.path().is_file());
        assert!(!cfgdir.path().join("sync_state.json").exists());
    }

    #[tokio::test]
    async fn state_saving_failure_after_success_retains_base_state_and_evidence() {
        let home = temp_home();
        let cfgdir = tempfile::tempdir().unwrap();
        let store = ApplyMockStore {
            manifest: VaultManifest::empty("d"),
            commit_mode: CommitMode::Ok,
            config_dir: Some(cfgdir.path().to_path_buf()),
            mutation: JournalMutation::BreakStateSavingOnSuccess,
            ..Default::default()
        };
        let (config, mut state, req) = single_upload_request(home.path(), &store).await;
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        // state_saving save_journal 失败 -> 持久化错误
        assert!(matches!(err, AppError::Io(_)));
        // 远端提交发生了一次
        assert_eq!(*store.commit_count.lock().unwrap(), 1);
        // 本机 state 仍在 base（未更新为 candidate）
        assert_eq!(state.remote.commit_sha, "c");
        // sync_state 未落盘
        assert!(!cfgdir.path().join("sync_state.json").exists());
        // recovery evidence 未被静默清除（journal 路径仍是目录）
        let journal_path = crate::local_apply::journal_path(cfgdir.path());
        assert!(journal_path.exists() && journal_path.is_dir());
    }

    #[tokio::test]
    async fn delete_guard_requires_ack_for_selected_delete() {
        let home = temp_home();
        let (vskill, _, _) = make_blob(Codex, "demo", "demo");
        let mut manifest = VaultManifest::empty("d");
        manifest.skills.insert(vskill.id.clone(), vskill.clone());
        let store = mock_store(manifest);
        let mut config = apply_config();
        config.limits.max_auto_delete = 0; // 任何删除触发护栏
        let mut state = apply_state();
        state.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: vskill.hash.clone(),
                last_remote_hash: vskill.hash.clone(),
                last_synced_at: String::new(),
                namespace: Codex,
                relative_dir: "demo".into(),
            },
        );
        let prepared = prepare_plan(&config, &state, &store, home.path())
            .await
            .unwrap();
        let plan = prepared.plan.clone();
        drop(prepared);
        // 选择 delete 但不 ack -> Blocked
        let req = apply_request(
            &plan,
            vec![action_id_of(&plan, "codex:demo")],
            HashMap::new(),
            false,
        );
        let cfgdir = tempfile::tempdir().unwrap();
        let err = apply_plan(
            &config,
            &mut state,
            &req,
            &store,
            home.path(),
            cfgdir.path(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Blocked(_)));
        assert_eq!(*store.commit_count.lock().unwrap(), 0);
    }
}
