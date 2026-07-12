// 新 vault sync plan DTO 与决策模型及其唯一实现入口。
// 前端 schema 与 Tauri boundary 复用本模块的 GitHub Vault 模型。
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{AppConfig, LimitsConfig, RemoteConfig};
use crate::detect::{ScanCollision, ScanRootStatus};
use crate::errors::{AppError, Result};
use crate::local_apply::{
    clean_dir_contents, clear_journal, commit_staged, move_to_trash, namespace_root, rollback_dir,
    save_journal, stage_dir, trash_dir, ApplyJournal,
};
use crate::pack::{unpack_skill, PackOptions, PackOutcome, SkillPackInput, SkillPacker};
use crate::portable_path::{collision_key, validate_component};
use crate::remote_store::{blob_path_for_hash, BlobWrite, RemoteChanges, RemoteStore};
use crate::skill::{namespace_value, parse_skill_md, skill_id, SkillNamespace};
use crate::sync_state::{RemoteIdentity, SkillSyncState, SyncState};
use crate::vault_manifest::{VaultManifest, VaultSkill};

/// 单个 skill 在三方比较后的同步状态（13 行真值表 + blocked/unknown）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SyncStatus {
    Synced,
    LocalUpdate,
    RemoteUpdate,
    LocalDeleted,
    RemoteDeleted,
    BothDeleted,
    Conflict,
    Blocked,
    Unknown,
}

/// 冲突原因，对应 decision 白名单的四种情形。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConflictReason {
    SameNameFirstSeen,
    BothChanged,
    LocalDeletedRemoteChanged,
    RemoteDeletedLocalChanged,
}

/// 删除方向：本地 -> trash，或云端 manifest 条目。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteDirection {
    DeleteLocal,
    DeleteRemote,
}

/// 用户对冲突的决策；必须落在对应 ConflictReason 的白名单内。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SyncDecision {
    KeepLocal,
    UseRemote,
    Skip,
    DeleteRemote,
    RestoreRemote,
    AcceptDelete,
}

/// 计划中单个 skill 的条目；同时承载可执行动作、冲突、blocked、unknown 信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncSkillEntry {
    pub action_id: String,
    pub skill_id: String,
    pub name: String,
    pub namespace: SkillNamespace,
    pub folder_name: String,
    pub relative_dir: Option<String>,
    pub status: SyncStatus,
    pub local_hash: Option<String>,
    pub remote_hash: Option<String>,
    pub base_hash: Option<String>,
    pub local_path: Option<String>,
    pub remote_blob: Option<String>,
    pub conflict_reason: Option<ConflictReason>,
    pub delete_direction: Option<DeleteDirection>,
    pub blocked_reason: Option<String>,
    pub warnings: Vec<String>,
}

/// base adoption：base 为空且 local==remote 时生成的本机状态协调（0 commit）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BaseAdoption {
    pub skill_id: String,
    pub hash: String,
}

/// 冲突条目；decisions 按 skill_id 索引，apply 时校验 decision 白名单。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Conflict {
    pub skill_id: String,
    pub name: String,
    pub namespace: SkillNamespace,
    pub folder_name: String,
    pub relative_dir: Option<String>,
    pub conflict_reason: ConflictReason,
    pub local_hash: Option<String>,
    pub remote_hash: Option<String>,
    pub base_hash: Option<String>,
    pub local_path: Option<String>,
    pub remote_blob: Option<String>,
    pub warnings: Vec<String>,
}

/// blocked skill；扫描碰撞、远端超限、identity 非法等不参与任何动作。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BlockedSkill {
    pub skill_id: String,
    pub name: String,
    pub namespace: SkillNamespace,
    pub folder_name: String,
    pub reason: String,
}

/// 提交摘要；`local_state_updates` 为去重后的 adoption/removal skill 数量。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CommitSummary {
    pub uploads: usize,
    pub downloads: usize,
    pub delete_remote: usize,
    pub delete_local: usize,
    pub local_state_updates: usize,
}

/// 完整同步计划。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncPlan {
    pub entries: Vec<SyncSkillEntry>,
    pub uploads: Vec<String>,
    pub downloads: Vec<String>,
    pub delete_remote: Vec<String>,
    pub delete_local: Vec<String>,
    pub conflicts: Vec<Conflict>,
    pub blocked: Vec<BlockedSkill>,
    pub warnings: Vec<String>,
    pub delete_guard_tripped: bool,
    pub expected_remote_commit: String,
    pub plan_fingerprint: String,
    pub base_adoptions: Vec<BaseAdoption>,
    pub base_removals: Vec<String>,
    pub will_create_commit: bool,
    pub commit_summary: CommitSummary,
}

/// apply 请求：携带预览时的 expected_remote_commit + plan_fingerprint + 用户选择。
/// selected_action_ids 只控制普通可执行项；冲突由 decisions 控制。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ApplySyncRequest {
    pub expected_remote_commit: String,
    pub plan_fingerprint: String,
    pub selected_action_ids: Vec<String>,
    pub decisions: HashMap<String, SyncDecision>,
    pub delete_guard_ack: bool,
}

/// PlanChanged 的原因：远端 commit 变化，或 commit 不变但计划内容变化。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanChangeReason {
    RemoteChanged,
    PlanChanged,
}

/// 恢复阶段；RecoveryRequired 只用于已产生持久化副作用或远端结果无法证明的事务。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecoveryPhase {
    RemoteOutcomeUnknown,
    LocalReplaceFailed,
    TrashMoveFailed,
    StateSaveFailed,
}

/// 恢复信息：task/phase、远端 commit、已完成/待完成 action、消息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RecoveryInfo {
    pub task_id: String,
    pub phase: RecoveryPhase,
    pub remote_commit: Option<String>,
    pub completed_action_ids: Vec<String>,
    pub pending_action_ids: Vec<String>,
    pub message: String,
}

/// apply 成功结果；包含本机状态更新和清理告警。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ApplyResult {
    pub applied: Vec<String>,
    pub state_updated: Vec<String>,
    pub warnings: Vec<String>,
    pub remote_commit: Option<String>,
}

/// apply 响应：成功、计划变更（需重新确认）、或需恢复。
/// `latest_plan` 装箱以避免 enum 变体大小悬殊（serde 透明，JSON 不变）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ApplySyncResponse {
    Applied {
        result: ApplyResult,
    },
    PlanChanged {
        reason: PlanChangeReason,
        latest_plan: Box<SyncPlan>,
    },
    RecoveryRequired {
        recovery: RecoveryInfo,
    },
}

// ---- Task 8: build_plan / merge_plan ----

/// 已打包的本地 skill 信息（scan + pack 后合并），作为 merge_plan 的 local 输入。
#[derive(Debug, Clone)]
pub(crate) struct LocalSkillInfo {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub folder_name: String,
    pub namespace: SkillNamespace,
    pub relative_dir: String,
    pub source_path: String,
    pub hash: String,
    pub zip_size: u64,
    pub warnings: Vec<String>,
    pub blocked_reason: Option<String>,
}

/// 三方比较的中间结果，包含 SyncSkillEntry 全部字段 + adoption/removal 内部标记。
struct RawEntry {
    skill_id: String,
    name: String,
    namespace: SkillNamespace,
    folder_name: String,
    relative_dir: Option<String>,
    status: SyncStatus,
    local_hash: Option<String>,
    remote_hash: Option<String>,
    base_hash: Option<String>,
    local_path: Option<String>,
    remote_blob: Option<String>,
    conflict_reason: Option<ConflictReason>,
    delete_direction: Option<DeleteDirection>,
    blocked_reason: Option<String>,
    warnings: Vec<String>,
    adoption: Option<String>,
    removal: bool,
}

/// 按 13 行真值表推导单个 skill 的状态与字段。
#[allow(clippy::too_many_arguments)]
fn derive_entry(
    id: &str,
    b: Option<&SkillSyncState>,
    l: Option<&LocalSkillInfo>,
    r: Option<&VaultSkill>,
) -> RawEntry {
    let local_hash = l.and_then(|x| x.blocked_reason.is_none().then(|| x.hash.clone()));
    let remote_hash = r.map(|x| x.hash.clone());
    let base_hash = b.map(|x| x.base_hash.clone());
    let namespace = l
        .map(|x| x.namespace)
        .or(r.map(|x| x.namespace))
        .or(b.map(|x| x.namespace))
        .unwrap_or(SkillNamespace::Agents);
    let folder_name = l
        .map(|x| x.folder_name.clone())
        .or(r.map(|x| x.folder_name.clone()))
        .or(b.map(|x| x.relative_dir.clone()))
        .unwrap_or_default();
    let relative_dir = l
        .map(|l| l.relative_dir.clone())
        .or_else(|| b.map(|b| b.relative_dir.clone()))
        .or_else(|| r.map(|r| r.folder_name.clone()));
    let name = l
        .map(|x| x.name.clone())
        .or(r.map(|x| x.name.clone()))
        .unwrap_or_default();
    let local_path = l.map(|x| x.source_path.clone());
    let remote_blob = r.map(|x| x.blob.clone());

    let (status, conflict_reason, delete_direction, adoption, removal) = match (b, l, r) {
        (None, Some(_), None) => (SyncStatus::LocalUpdate, None, None, None, false),
        (None, None, Some(_)) => (SyncStatus::RemoteUpdate, None, None, None, false),
        (None, Some(l), Some(r)) => {
            if l.hash == r.hash {
                (SyncStatus::Synced, None, None, Some(l.hash.clone()), false)
            } else {
                (
                    SyncStatus::Conflict,
                    Some(ConflictReason::SameNameFirstSeen),
                    None,
                    None,
                    false,
                )
            }
        }
        (Some(b), Some(l), Some(r)) => {
            if l.hash == r.hash {
                let adoption = if b.base_hash != l.hash {
                    Some(l.hash.clone())
                } else {
                    None
                };
                (SyncStatus::Synced, None, None, adoption, false)
            } else if r.hash == b.base_hash {
                (SyncStatus::LocalUpdate, None, None, None, false)
            } else if l.hash == b.base_hash {
                (SyncStatus::RemoteUpdate, None, None, None, false)
            } else {
                (
                    SyncStatus::Conflict,
                    Some(ConflictReason::BothChanged),
                    None,
                    None,
                    false,
                )
            }
        }
        (Some(b), None, Some(r)) => {
            if r.hash == b.base_hash {
                (
                    SyncStatus::LocalDeleted,
                    None,
                    Some(DeleteDirection::DeleteRemote),
                    None,
                    false,
                )
            } else {
                (
                    SyncStatus::Conflict,
                    Some(ConflictReason::LocalDeletedRemoteChanged),
                    Some(DeleteDirection::DeleteRemote),
                    None,
                    false,
                )
            }
        }
        (Some(b), Some(l), None) => {
            if l.hash == b.base_hash {
                (
                    SyncStatus::RemoteDeleted,
                    None,
                    Some(DeleteDirection::DeleteLocal),
                    None,
                    false,
                )
            } else {
                (
                    SyncStatus::Conflict,
                    Some(ConflictReason::RemoteDeletedLocalChanged),
                    Some(DeleteDirection::DeleteLocal),
                    None,
                    false,
                )
            }
        }
        (Some(_), None, None) => (SyncStatus::BothDeleted, None, None, None, true),
        (None, None, None) => unreachable!("skill_id must come from base/local/remote"),
    };

    RawEntry {
        skill_id: id.to_string(),
        name,
        namespace,
        folder_name,
        relative_dir,
        status,
        local_hash,
        remote_hash,
        base_hash,
        local_path,
        remote_blob,
        conflict_reason,
        delete_direction,
        blocked_reason: None,
        warnings: l.map(|x| x.warnings.clone()).unwrap_or_default(),
        adoption,
        removal,
    }
}

/// 由状态推导 action_kind（action_id 前缀）。
fn action_kind_for(status: SyncStatus) -> &'static str {
    match status {
        SyncStatus::LocalUpdate => "upload",
        SyncStatus::RemoteUpdate => "download",
        SyncStatus::LocalDeleted => "delete_remote",
        SyncStatus::RemoteDeleted => "delete_local",
        SyncStatus::Conflict => "conflict",
        SyncStatus::Synced => "synced",
        SyncStatus::BothDeleted => "both_deleted",
        SyncStatus::Blocked => "blocked",
        SyncStatus::Unknown => "unknown",
    }
}

/// 校验 action_id 全计划唯一，重复返回 `Blocked`。
fn ensure_unique_action_ids(entries: &[SyncSkillEntry]) -> Result<()> {
    let mut seen: HashSet<&str> = HashSet::new();
    for e in entries {
        if !seen.insert(e.action_id.as_str()) {
            return Err(AppError::Blocked(format!(
                "duplicate action_id: {}",
                e.action_id
            )));
        }
    }
    Ok(())
}

fn collision_entry(id: &str, collision: &ScanCollision) -> RawEntry {
    let index = collision
        .skill_ids
        .iter()
        .position(|skill_id| skill_id == id);
    let folder_name = index
        .and_then(|i| collision.paths.get(i))
        .and_then(|path| path.rsplit(['/', '\\']).next())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| collision.collision_key.clone());
    let name = id
        .split_once(':')
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| id.to_string());

    RawEntry {
        skill_id: id.to_string(),
        name,
        namespace: collision.namespace,
        folder_name: folder_name.clone(),
        relative_dir: Some(folder_name),
        status: SyncStatus::Blocked,
        local_hash: None,
        remote_hash: None,
        base_hash: None,
        local_path: index.and_then(|i| collision.paths.get(i).cloned()),
        remote_blob: None,
        conflict_reason: None,
        delete_direction: None,
        blocked_reason: Some("scan collision".into()),
        warnings: Vec::new(),
        adoption: None,
        removal: false,
    }
}

fn namespace_matches_id(id: &str, namespace: SkillNamespace) -> bool {
    id.split_once(':')
        .is_some_and(|(prefix, name)| !name.is_empty() && prefix == namespace_value(namespace))
}

/// 把 base/local/remote 三方按 13 行真值表合并成 SyncPlan。纯函数，不含 IO，
/// 供 build_plan 与测试直接调用。不写本地、不上传、不下载、不删除。
#[allow(clippy::too_many_arguments)]
pub(crate) fn merge_plan(
    base: &BTreeMap<String, SkillSyncState>,
    local: &[LocalSkillInfo],
    remote: &VaultManifest,
    scan_roots: &[ScanRootStatus],
    scan_collisions: &[ScanCollision],
    limits: &LimitsConfig,
    expected_remote_commit: &str,
) -> Result<SyncPlan> {
    let mut local_ids = HashSet::new();
    for skill in local {
        if !local_ids.insert(skill.skill_id.as_str()) {
            return Err(AppError::Blocked(format!(
                "duplicate local skill_id: {}",
                skill.skill_id
            )));
        }
    }

    let local_map: HashMap<&str, &LocalSkillInfo> =
        local.iter().map(|l| (l.skill_id.as_str(), l)).collect();

    let mut identity_blocked_ids = HashSet::new();
    for (id, state) in base {
        if !namespace_matches_id(id, state.namespace) {
            identity_blocked_ids.insert(id.clone());
        }
    }
    for skill in local {
        if !namespace_matches_id(&skill.skill_id, skill.namespace) {
            identity_blocked_ids.insert(skill.skill_id.clone());
        }
    }
    for (id, skill) in &remote.skills {
        if id != &skill.id || !namespace_matches_id(id, skill.namespace) {
            identity_blocked_ids.insert(id.clone());
        }
    }

    // 全集 skill_id（base ∪ local ∪ remote ∪ scan_collision）
    let mut all_ids: BTreeSet<String> = base.keys().cloned().collect();
    for l in local {
        all_ids.insert(l.skill_id.clone());
    }
    for k in remote.skills.keys() {
        all_ids.insert(k.clone());
    }
    for c in scan_collisions {
        for id in &c.skill_ids {
            all_ids.insert(id.clone());
        }
    }

    let scan_collision_ids: HashSet<String> = scan_collisions
        .iter()
        .flat_map(|c| c.skill_ids.iter().cloned())
        .collect();

    // 推导原始 entry
    let mut raw: Vec<RawEntry> = Vec::new();
    for id in &all_ids {
        let b = base.get(id);
        let l = local_map.get(id.as_str()).copied();
        let r = remote.skills.get(id);
        if b.is_none() && l.is_none() && r.is_none() {
            if let Some(collision) = scan_collisions
                .iter()
                .find(|collision| collision.skill_ids.iter().any(|skill_id| skill_id == id))
            {
                raw.push(collision_entry(id, collision));
            }
        } else {
            raw.push(derive_entry(id, b, l, r));
        }
    }

    // pack、identity 与 scan collision 都不能进入三方动作。
    for e in raw.iter_mut() {
        if let Some(local) = local_map.get(e.skill_id.as_str()) {
            if let Some(reason) = &local.blocked_reason {
                block_entry(e, reason);
            }
        }
        if identity_blocked_ids.contains(&e.skill_id) {
            block_entry(e, "skill identity namespace mismatch");
        }
        if scan_collision_ids.contains(&e.skill_id) {
            block_entry(e, "scan collision");
        }
    }

    // 远端 entry manifest size 超限 -> Blocked（不 fetch blob）
    for e in raw.iter_mut() {
        if let Some(r) = remote.skills.get(&e.skill_id) {
            if r.size > limits.max_skill_zip_bytes {
                block_entry(e, "remote skill exceeds max_skill_zip_bytes");
            }
        }
    }

    // 合并目标 key (namespace, folded folder) collision -> Blocked
    let mut target_groups: HashMap<(SkillNamespace, String), Vec<String>> = HashMap::new();
    for e in &raw {
        if e.folder_name.is_empty() || (e.local_path.is_none() && e.remote_blob.is_none()) {
            continue;
        }
        let key = (e.namespace, collision_key(&e.folder_name));
        target_groups
            .entry(key)
            .or_default()
            .push(e.skill_id.clone());
    }
    let merge_collision_ids: HashSet<String> = target_groups
        .values()
        .filter_map(|ids| {
            let distinct: HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
            (distinct.len() > 1).then(|| ids.clone())
        })
        .flatten()
        .collect();
    for e in raw.iter_mut() {
        if merge_collision_ids.contains(&e.skill_id) {
            block_entry(e, "target collision");
        }
    }

    // base 删除推断行（9/10/13）在 namespace root 不健康时标 Unknown，不推断删除
    for e in raw.iter_mut() {
        let is_delete_inference =
            matches!(e.status, SyncStatus::LocalDeleted | SyncStatus::BothDeleted)
                || (e.status == SyncStatus::Conflict
                    && matches!(
                        e.conflict_reason,
                        Some(ConflictReason::LocalDeletedRemoteChanged)
                    ));
        if is_delete_inference {
            let root = scan_roots.iter().find(|r| r.namespace == e.namespace);
            let root_healthy =
                root.is_some_and(|root| root.exists && root.readable && root.scan_complete);
            if !root_healthy {
                e.status = SyncStatus::Unknown;
                e.conflict_reason = None;
                e.delete_direction = None;
                e.adoption = None;
                e.removal = false;
                let root_path = root
                    .map(|root| root.root_path.as_str())
                    .unwrap_or("missing scan root status");
                e.blocked_reason = Some(format!("namespace root unhealthy: {root_path}"));
            }
        }
    }

    // 构建 entries + 列表
    let mut entries: Vec<SyncSkillEntry> = Vec::new();
    let mut uploads: Vec<String> = Vec::new();
    let mut downloads: Vec<String> = Vec::new();
    let mut delete_remote: Vec<String> = Vec::new();
    let mut delete_local: Vec<String> = Vec::new();
    let mut conflicts: Vec<Conflict> = Vec::new();
    let mut blocked: Vec<BlockedSkill> = Vec::new();
    let mut base_adoptions: Vec<BaseAdoption> = Vec::new();
    let mut base_removals: Vec<String> = Vec::new();

    for e in &raw {
        let action_kind = action_kind_for(e.status);
        let action_id = format!("{action_kind}:{}", e.skill_id);
        entries.push(SyncSkillEntry {
            action_id: action_id.clone(),
            skill_id: e.skill_id.clone(),
            name: e.name.clone(),
            namespace: e.namespace,
            folder_name: e.folder_name.clone(),
            relative_dir: e.relative_dir.clone(),
            status: e.status,
            local_hash: e.local_hash.clone(),
            remote_hash: e.remote_hash.clone(),
            base_hash: e.base_hash.clone(),
            local_path: e.local_path.clone(),
            remote_blob: e.remote_blob.clone(),
            conflict_reason: e.conflict_reason,
            delete_direction: e.delete_direction,
            blocked_reason: e.blocked_reason.clone(),
            warnings: e.warnings.clone(),
        });
        match e.status {
            SyncStatus::LocalUpdate => uploads.push(action_id),
            SyncStatus::RemoteUpdate => downloads.push(action_id),
            SyncStatus::LocalDeleted => delete_remote.push(action_id),
            SyncStatus::RemoteDeleted => delete_local.push(action_id),
            SyncStatus::Conflict => {
                let conflict_reason = e.conflict_reason.ok_or_else(|| {
                    AppError::Blocked(format!("conflict entry missing reason: {}", e.skill_id))
                })?;
                conflicts.push(Conflict {
                    skill_id: e.skill_id.clone(),
                    name: e.name.clone(),
                    namespace: e.namespace,
                    folder_name: e.folder_name.clone(),
                    relative_dir: e.relative_dir.clone(),
                    conflict_reason,
                    local_hash: e.local_hash.clone(),
                    remote_hash: e.remote_hash.clone(),
                    base_hash: e.base_hash.clone(),
                    local_path: e.local_path.clone(),
                    remote_blob: e.remote_blob.clone(),
                    warnings: e.warnings.clone(),
                });
            }
            SyncStatus::Blocked => blocked.push(BlockedSkill {
                skill_id: e.skill_id.clone(),
                name: e.name.clone(),
                namespace: e.namespace,
                folder_name: e.folder_name.clone(),
                reason: e.blocked_reason.clone().unwrap_or_default(),
            }),
            _ => {}
        }
        if let Some(h) = &e.adoption {
            base_adoptions.push(BaseAdoption {
                skill_id: e.skill_id.clone(),
                hash: h.clone(),
            });
        }
        if e.removal {
            base_removals.push(e.skill_id.clone());
        }
    }

    ensure_unique_action_ids(&entries)?;

    // 删除护栏：definite 删除 + 删除侧 conflict
    let delete_count = raw
        .iter()
        .filter(|e| {
            matches!(
                e.status,
                SyncStatus::LocalDeleted | SyncStatus::RemoteDeleted
            ) || (e.status == SyncStatus::Conflict
                && matches!(
                    e.conflict_reason,
                    Some(ConflictReason::LocalDeletedRemoteChanged)
                        | Some(ConflictReason::RemoteDeletedLocalChanged)
                ))
        })
        .count();
    let tracked = base.len();
    let delete_guard_tripped =
        delete_count > limits.max_auto_delete || (tracked > 0 && delete_count * 2 > tracked);

    let mut state_ids: HashSet<&str> = HashSet::new();
    for a in &base_adoptions {
        state_ids.insert(a.skill_id.as_str());
    }
    for r in &base_removals {
        state_ids.insert(r.as_str());
    }
    let local_state_updates = state_ids.len();

    let will_create_commit = !uploads.is_empty() || !delete_remote.is_empty();

    let commit_summary = CommitSummary {
        uploads: uploads.len(),
        downloads: downloads.len(),
        delete_remote: delete_remote.len(),
        delete_local: delete_local.len(),
        local_state_updates,
    };

    let mut plan = SyncPlan {
        entries,
        uploads,
        downloads,
        delete_remote,
        delete_local,
        conflicts,
        blocked,
        warnings: Vec::new(),
        delete_guard_tripped,
        expected_remote_commit: expected_remote_commit.to_string(),
        plan_fingerprint: String::new(),
        base_adoptions,
        base_removals,
        will_create_commit,
        commit_summary,
    };
    plan.plan_fingerprint = compute_fingerprint(&plan)?;
    Ok(plan)
}

fn block_entry(e: &mut RawEntry, reason: &str) {
    e.status = SyncStatus::Blocked;
    e.conflict_reason = None;
    e.delete_direction = None;
    e.adoption = None;
    e.removal = false;
    e.blocked_reason = Some(reason.to_string());
}

// ---- plan fingerprint ----

#[derive(Serialize)]
struct FpEntry<'a> {
    action_id: &'a str,
    namespace: &'a str,
    folder_name: &'a str,
    relative_dir: Option<&'a str>,
    status: SyncStatus,
    local_hash: Option<&'a str>,
    remote_hash: Option<&'a str>,
    base_hash: Option<&'a str>,
    local_path: Option<&'a str>,
    remote_blob: Option<&'a str>,
    delete_direction: Option<DeleteDirection>,
    conflict_reason: Option<ConflictReason>,
    blocked_reason: Option<&'a str>,
}

#[derive(Serialize)]
struct FpAdoption<'a> {
    skill_id: &'a str,
    hash: &'a str,
}

#[derive(Serialize)]
struct FpInput<'a> {
    expected_remote_commit: &'a str,
    delete_guard_tripped: bool,
    base_adoptions: Vec<FpAdoption<'a>>,
    base_removals: Vec<&'a str>,
    entries: Vec<FpEntry<'a>>,
}

fn fingerprint_input(plan: &SyncPlan) -> FpInput<'_> {
    let mut entries: Vec<FpEntry> = plan
        .entries
        .iter()
        .map(|e| FpEntry {
            action_id: &e.action_id,
            namespace: namespace_value(e.namespace),
            folder_name: &e.folder_name,
            relative_dir: e.relative_dir.as_deref(),
            status: e.status,
            local_hash: e.local_hash.as_deref(),
            remote_hash: e.remote_hash.as_deref(),
            base_hash: e.base_hash.as_deref(),
            local_path: e.local_path.as_deref(),
            remote_blob: e.remote_blob.as_deref(),
            delete_direction: e.delete_direction,
            conflict_reason: e.conflict_reason,
            blocked_reason: e.blocked_reason.as_deref(),
        })
        .collect();
    entries.sort_by(|a, b| a.action_id.cmp(b.action_id));

    let mut adoptions: Vec<FpAdoption> = plan
        .base_adoptions
        .iter()
        .map(|a| FpAdoption {
            skill_id: &a.skill_id,
            hash: &a.hash,
        })
        .collect();
    adoptions.sort_by(|a, b| a.skill_id.cmp(b.skill_id));

    let mut removals: Vec<&str> = plan.base_removals.iter().map(|s| s.as_str()).collect();
    removals.sort();

    FpInput {
        expected_remote_commit: &plan.expected_remote_commit,
        delete_guard_tripped: plan.delete_guard_tripped,
        base_adoptions: adoptions,
        base_removals: removals,
        entries,
    }
}

fn canonical_fingerprint_bytes(plan: &SyncPlan) -> Result<Vec<u8>> {
    let input = fingerprint_input(plan);
    let bytes = serde_json::to_vec(&input)
        .map_err(|e| AppError::Vault(format!("fingerprint serialize failed: {e}")))?;
    Ok(bytes)
}

/// 计算 plan fingerprint：固定字段顺序，optional 缺失为显式 null，entries/adoptions/
/// removals 按 id 字节序排序；`serde_json::to_vec` 后 sha256。不含展示 warnings/顺序/时间戳。
fn compute_fingerprint(plan: &SyncPlan) -> Result<String> {
    let bytes = canonical_fingerprint_bytes(plan)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(&bytes))))
}

/// 验证 config.remote 存在且与 state.remote identity（installation/repository/branch）一致。
/// 缺失返回 `NotConfigured`，不一致返回 `Blocked`，均发生在任何 RemoteStore 调用前。
fn validate_remote(cfg: &RemoteConfig, state: &RemoteIdentity) -> Result<()> {
    if cfg.installation_id != state.installation_id
        || cfg.repository_id != state.repository_id
        || cfg.branch != state.branch
    {
        return Err(AppError::Blocked(
            "sync state remote identity mismatch".into(),
        ));
    }
    Ok(())
}

/// 构建同步计划：校验远端配置与 state identity -> fetch manifest -> spawn_blocking
/// 扫描固定 root 与打包本地 skill -> merge_plan。预览结束清理临时 zip 目录。
/// 不写本地、不上传、不下载、不删除。
pub(crate) async fn build_plan<S: RemoteStore>(
    config: &AppConfig,
    state: &SyncState,
    store: &S,
) -> Result<SyncPlan> {
    let remote_cfg = config.remote.as_ref().ok_or_else(|| {
        AppError::NotConfigured("remote not configured; onboarding required".into())
    })?;
    validate_remote(remote_cfg, &state.remote)?;
    config.limits.validate()?;

    let snapshot = store.fetch_manifest().await?;

    let home =
        dirs::home_dir().ok_or_else(|| AppError::Config("cannot determine home dir".into()))?;
    let scan = {
        let home = home.clone();
        tauri::async_runtime::spawn_blocking(move || crate::detect::scan_fixed_roots(&home))
            .await
            .map_err(|e| AppError::Vault(format!("scan task failed: {e}")))?
    }?;

    let pack_inputs: Vec<SkillPackInput> = scan
        .skills
        .iter()
        .map(|s| SkillPackInput {
            skill_id: s.id.clone(),
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

    let local_infos: Vec<LocalSkillInfo> = scan
        .skills
        .iter()
        .zip(batch.outcomes.iter())
        .map(|(s, o)| match o {
            PackOutcome::Packed(p) => LocalSkillInfo {
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
                    .map(|warning| format!("{:?}: {}", warning.kind, warning.relative_path))
                    .collect(),
                blocked_reason: None,
            },
            PackOutcome::Blocked(blocked) => LocalSkillInfo {
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
                blocked_reason: Some(blocked.reason.clone()),
            },
        })
        .collect();

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
    drop(batch);
    Ok(plan)
}

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
    pub batch: crate::pack::PackBatch,
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
            skill_id: s.id.clone(),
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
        batch,
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
    entry_folder: String,
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
        let conflict = plan
            .conflicts
            .iter()
            .find(|c| &c.skill_id == skill_id)
            .ok_or_else(|| {
                AppError::Blocked(format!("decision for non-conflict skill: {skill_id}"))
            })?;
        if !allowed_decisions(conflict.conflict_reason).contains(decision) {
            return Err(AppError::Blocked(format!(
                "invalid decision for {skill_id}"
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
                    entry_folder: entry.folder_name.clone(),
                });
            }
            _ => {}
        }
    }

    for (skill_id, decision) in &request.decisions {
        let conflict = plan
            .conflicts
            .iter()
            .find(|c| &c.skill_id == skill_id)
            .ok_or_else(|| {
                AppError::Blocked(format!("decision for non-conflict skill: {skill_id}"))
            })?;
        match decision {
            SyncDecision::KeepLocal => {
                let pl = prepared.packed.get(skill_id).ok_or_else(|| {
                    AppError::Blocked(format!("conflict keep_local not packed: {skill_id}"))
                })?;
                uploads.push(UploadItem {
                    skill_id: skill_id.clone(),
                    name: pl.info.name.clone(),
                    description: pl.info.description.clone(),
                    namespace: conflict.namespace,
                    folder_name: conflict.folder_name.clone(),
                    relative_dir: conflict.relative_dir.clone(),
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
                    entry_ns: conflict.namespace,
                    entry_folder: conflict.folder_name.clone(),
                    vskill,
                });
            }
            SyncDecision::DeleteRemote => {
                delete_remote_ids.push(skill_id.clone());
            }
            SyncDecision::AcceptDelete => {
                delete_local_items.push(DeleteLocalItem {
                    skill_id: skill_id.clone(),
                    entry_ns: conflict.namespace,
                    entry_folder: conflict.folder_name.clone(),
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
    for up in &uploads {
        let bytes = fs::read(&up.zip_path)?;
        let blob_path = blob_path_for_hash(&up.hash)?;
        blobs.push(BlobWrite {
            path: blob_path.clone(),
            bytes,
            expected_hash: up.hash.clone(),
        });
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
    use crate::remote_store::{RemoteChanges, RemoteCommit, RemoteSnapshot};

    const HASH_L: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const HASH_R: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    const HASH_B: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
    const HASH_LR: &str = "sha256:4444444444444444444444444444444444444444444444444444444444444444";

    use SkillNamespace::*;

    fn lid(id: &str, hash: &str, ns: SkillNamespace, folder: &str) -> LocalSkillInfo {
        LocalSkillInfo {
            skill_id: id.into(),
            name: id.into(),
            description: "d".into(),
            folder_name: folder.into(),
            namespace: ns,
            relative_dir: folder.into(),
            source_path: format!("/home/x/{folder}"),
            hash: hash.into(),
            zip_size: 100,
            warnings: Vec::new(),
            blocked_reason: None,
        }
    }

    fn vskill(id: &str, hash: &str, ns: SkillNamespace, folder: &str) -> VaultSkill {
        let hex = &hash["sha256:".len()..];
        VaultSkill {
            id: id.into(),
            name: id.into(),
            description: "d".into(),
            namespace: ns,
            folder_name: folder.into(),
            hash: hash.into(),
            blob: format!("blobs/sha256/{hex}.skill.zip"),
            size: 100,
            updated_at: String::new(),
            updated_by: "d".into(),
        }
    }

    fn manifest_of(skills: &[VaultSkill]) -> VaultManifest {
        let mut m = VaultManifest::empty("d");
        for s in skills {
            m.skills.insert(s.id.clone(), s.clone());
        }
        m
    }

    fn base_map(
        entries: &[(&str, &str, SkillNamespace, &str)],
    ) -> BTreeMap<String, SkillSyncState> {
        entries
            .iter()
            .map(|(id, h, ns, rel)| {
                (
                    id.to_string(),
                    SkillSyncState {
                        base_hash: h.to_string(),
                        last_remote_hash: h.to_string(),
                        last_synced_at: String::new(),
                        namespace: *ns,
                        relative_dir: rel.to_string(),
                    },
                )
            })
            .collect()
    }

    fn healthy_roots() -> Vec<ScanRootStatus> {
        [Agents, Codex, ClaudeCode]
            .iter()
            .map(|ns| ScanRootStatus {
                namespace: *ns,
                root_path: format!("/home/x/{}/skills", namespace_value(*ns)),
                exists: true,
                readable: true,
                scan_complete: true,
                error: None,
            })
            .collect()
    }

    fn root(ns: SkillNamespace, exists: bool, readable: bool, complete: bool) -> ScanRootStatus {
        ScanRootStatus {
            namespace: ns,
            root_path: format!("/home/x/{}/skills", namespace_value(ns)),
            exists,
            readable,
            scan_complete: complete,
            error: None,
        }
    }

    fn merge(
        base: &[(&str, &str, SkillNamespace, &str)],
        local: &[LocalSkillInfo],
        remote: &[VaultSkill],
    ) -> SyncPlan {
        let bm = base_map(base);
        let m = manifest_of(remote);
        merge_plan(
            &bm,
            local,
            &m,
            &healthy_roots(),
            &[],
            &LimitsConfig::default(),
            "commit-sha",
        )
        .unwrap()
    }

    fn entry_of<'a>(plan: &'a SyncPlan, skill_id: &str) -> &'a SyncSkillEntry {
        plan.entries
            .iter()
            .find(|e| e.skill_id == skill_id)
            .unwrap()
    }

    // ---- Task 7 serialization tests ----

    #[test]
    fn status_reason_decision_serialize_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&SyncStatus::LocalUpdate).unwrap(),
            "\"local_update\""
        );
        assert_eq!(
            serde_json::to_string(&SyncStatus::BothDeleted).unwrap(),
            "\"both_deleted\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictReason::SameNameFirstSeen).unwrap(),
            "\"same_name_first_seen\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictReason::LocalDeletedRemoteChanged).unwrap(),
            "\"local_deleted_remote_changed\""
        );
        assert_eq!(
            serde_json::to_string(&SyncDecision::RestoreRemote).unwrap(),
            "\"restore_remote\""
        );
        assert_eq!(
            serde_json::to_string(&DeleteDirection::DeleteLocal).unwrap(),
            "\"delete_local\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryPhase::StateSaveFailed).unwrap(),
            "\"state_save_failed\""
        );
    }

    fn minimal_plan() -> SyncPlan {
        SyncPlan {
            entries: vec![],
            uploads: vec![],
            downloads: vec![],
            delete_remote: vec![],
            delete_local: vec![],
            conflicts: vec![],
            blocked: vec![],
            warnings: vec![],
            delete_guard_tripped: false,
            expected_remote_commit: "sha".into(),
            plan_fingerprint: "fp".into(),
            base_adoptions: vec![],
            base_removals: vec![],
            will_create_commit: false,
            commit_summary: CommitSummary::default(),
        }
    }

    #[test]
    fn apply_response_uses_status_tag() {
        let applied = ApplySyncResponse::Applied {
            result: ApplyResult {
                applied: vec!["a".into()],
                state_updated: vec![],
                warnings: vec![],
                remote_commit: None,
            },
        };
        let json = serde_json::to_value(&applied).unwrap();
        assert_eq!(json["status"], "applied");
        assert_eq!(json["result"]["applied"][0], "a");

        let plan_changed = ApplySyncResponse::PlanChanged {
            reason: PlanChangeReason::RemoteChanged,
            latest_plan: Box::new(minimal_plan()),
        };
        let json = serde_json::to_value(&plan_changed).unwrap();
        assert_eq!(json["status"], "plan_changed");
        assert_eq!(json["reason"], "remote_changed");
    }

    #[test]
    fn sync_plan_roundtrips() {
        let plan = minimal_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let back: SyncPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }

    // ---- Task 8: 三方比较 13 行真值表 ----

    #[test]
    fn base_empty_local_only_is_local_update() {
        let plan = merge(&[], &[lid("codex:demo", HASH_L, Codex, "demo")], &[]);
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::LocalUpdate);
        assert_eq!(e.local_hash.as_deref(), Some(HASH_L));
        assert!(plan.uploads.iter().any(|a| a == "upload:codex:demo"));
        assert!(plan.will_create_commit);
        assert!(plan.base_adoptions.is_empty());
    }

    #[test]
    fn base_empty_remote_only_is_remote_update() {
        let plan = merge(&[], &[], &[vskill("codex:demo", HASH_R, Codex, "demo")]);
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::RemoteUpdate);
        assert_eq!(e.remote_hash.as_deref(), Some(HASH_R));
        assert_eq!(e.relative_dir.as_deref(), Some("demo"));
        assert!(!plan.will_create_commit);
    }

    #[test]
    fn base_empty_both_same_emits_base_adoption_without_writing_state() {
        let plan = merge(
            &[],
            &[lid("codex:demo", HASH_LR, Codex, "demo")],
            &[vskill("codex:demo", HASH_LR, Codex, "demo")],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Synced);
        assert_eq!(plan.base_adoptions.len(), 1);
        assert_eq!(plan.base_adoptions[0].skill_id, "codex:demo");
        assert_eq!(plan.base_adoptions[0].hash, HASH_LR);
        assert!(!plan.will_create_commit);
    }

    #[test]
    fn both_same_new_hash_advances_existing_base() {
        let plan = merge(
            &[("codex:demo", HASH_B, Codex, "demo")],
            &[lid("codex:demo", HASH_LR, Codex, "demo")],
            &[vskill("codex:demo", HASH_LR, Codex, "demo")],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Synced);
        assert_eq!(plan.base_adoptions.len(), 1);
        assert_eq!(plan.base_adoptions[0].hash, HASH_LR);
    }

    #[test]
    fn both_same_current_base_emits_no_adoption() {
        let plan = merge(
            &[("codex:demo", HASH_LR, Codex, "demo")],
            &[lid("codex:demo", HASH_LR, Codex, "demo")],
            &[vskill("codex:demo", HASH_LR, Codex, "demo")],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Synced);
        assert!(plan.base_adoptions.is_empty());
    }

    #[test]
    fn base_empty_both_different_is_same_name_conflict() {
        let plan = merge(
            &[],
            &[lid("codex:demo", HASH_L, Codex, "demo")],
            &[vskill("codex:demo", HASH_R, Codex, "demo")],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Conflict);
        assert_eq!(e.conflict_reason, Some(ConflictReason::SameNameFirstSeen));
        assert_eq!(plan.conflicts.len(), 1);
    }

    #[test]
    fn local_deleted_remote_base_is_local_deleted() {
        let plan = merge(
            &[("codex:demo", HASH_B, Codex, "demo")],
            &[],
            &[vskill("codex:demo", HASH_B, Codex, "demo")],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::LocalDeleted);
        assert_eq!(e.delete_direction, Some(DeleteDirection::DeleteRemote));
        assert!(plan.will_create_commit);
    }

    #[test]
    fn local_deleted_remote_changed_is_conflict() {
        let plan = merge(
            &[("codex:demo", HASH_B, Codex, "demo")],
            &[],
            &[vskill("codex:demo", HASH_R, Codex, "demo")],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Conflict);
        assert_eq!(
            e.conflict_reason,
            Some(ConflictReason::LocalDeletedRemoteChanged)
        );
    }

    #[test]
    fn remote_deleted_local_base_is_remote_deleted() {
        let plan = merge(
            &[("codex:demo", HASH_B, Codex, "demo")],
            &[lid("codex:demo", HASH_B, Codex, "demo")],
            &[],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::RemoteDeleted);
        assert_eq!(e.delete_direction, Some(DeleteDirection::DeleteLocal));
        assert!(!plan.will_create_commit);
    }

    #[test]
    fn remote_deleted_local_changed_is_conflict() {
        let plan = merge(
            &[("codex:demo", HASH_B, Codex, "demo")],
            &[lid("codex:demo", HASH_L, Codex, "demo")],
            &[],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Conflict);
        assert_eq!(
            e.conflict_reason,
            Some(ConflictReason::RemoteDeletedLocalChanged)
        );
    }

    #[test]
    fn both_deleted_emits_base_removal_without_writing_state() {
        let plan = merge(&[("codex:demo", HASH_B, Codex, "demo")], &[], &[]);
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::BothDeleted);
        assert_eq!(plan.base_removals, vec!["codex:demo".to_string()]);
        assert!(!plan.will_create_commit);
    }

    #[test]
    fn both_deleted_base_does_not_collide_with_remote_new_target() {
        let plan = merge(
            &[("codex:old", HASH_B, Codex, "demo")],
            &[],
            &[vskill("codex:new", HASH_R, Codex, "demo")],
        );
        assert_eq!(entry_of(&plan, "codex:old").status, SyncStatus::BothDeleted);
        assert_eq!(
            entry_of(&plan, "codex:new").status,
            SyncStatus::RemoteUpdate
        );
        assert!(plan.blocked.is_empty());
    }

    // ---- plan fingerprint ----

    /// 一个含 adoption + removal + conflict 的丰富计划，用于 fingerprint 测试。
    fn rich_plan() -> SyncPlan {
        let base = base_map(&[
            ("codex:adv", HASH_B, Codex, "adv"),
            ("codex:del", HASH_B, Codex, "del"),
            ("codex:cnf", HASH_B, Codex, "cnf"),
        ]);
        let local = vec![lid("codex:adv", HASH_LR, Codex, "adv")];
        let remote = vec![
            vskill("codex:adv", HASH_LR, Codex, "adv"),
            vskill("codex:cnf", HASH_R, Codex, "cnf"),
        ];
        merge_plan(
            &base,
            &local,
            &manifest_of(&remote),
            &healthy_roots(),
            &[],
            &LimitsConfig::default(),
            "commit-sha",
        )
        .unwrap()
    }

    #[test]
    fn plan_fingerprint_is_stable_when_entry_order_changes() {
        let plan = rich_plan();
        let fp1 = plan.plan_fingerprint.clone();
        let mut shuffled = plan.clone();
        shuffled.entries.reverse();
        shuffled.base_adoptions.reverse();
        shuffled.base_removals.reverse();
        let fp2 = compute_fingerprint(&shuffled).unwrap();
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn plan_fingerprint_changes_when_action_input_changes() {
        let base_plan = rich_plan();
        let base_fp = base_plan.plan_fingerprint.clone();
        let check = |mutated: SyncPlan, label: &str| {
            let fp = compute_fingerprint(&mutated).unwrap();
            assert_ne!(fp, base_fp, "fingerprint did not change for {label}");
        };
        let mut p = base_plan.clone();
        p.expected_remote_commit = "other".into();
        check(p, "expected_remote_commit");
        let mut p = base_plan.clone();
        p.delete_guard_tripped = !p.delete_guard_tripped;
        check(p, "delete_guard_tripped");
        let mut p = base_plan.clone();
        p.entries[0].action_id = "x:codex:adv".into();
        check(p, "action_id");
        let mut p = base_plan.clone();
        p.entries[0].namespace = Agents;
        check(p, "namespace");
        let mut p = base_plan.clone();
        p.entries[0].folder_name = "other".into();
        check(p, "folder_name");
        let mut p = base_plan.clone();
        p.entries[0].relative_dir = Some("other".into());
        check(p, "relative_dir");
        let mut p = base_plan.clone();
        p.entries[0].status = SyncStatus::Blocked;
        check(p, "status");
        let mut p = base_plan.clone();
        p.entries[0].local_hash = Some("sha256:9999".into());
        check(p, "local_hash");
        let mut p = base_plan.clone();
        p.entries[0].remote_hash = Some("sha256:8888".into());
        check(p, "remote_hash");
        let mut p = base_plan.clone();
        p.entries[0].base_hash = Some("sha256:7777".into());
        check(p, "base_hash");
        let mut p = base_plan.clone();
        p.entries[0].local_path = Some("/other".into());
        check(p, "local_path");
        let mut p = base_plan.clone();
        p.entries[0].remote_blob = Some("other".into());
        check(p, "remote_blob");
        let mut p = base_plan.clone();
        p.entries[0].delete_direction = Some(DeleteDirection::DeleteLocal);
        check(p, "delete_direction");
        let mut p = base_plan.clone();
        p.entries[0].conflict_reason = Some(ConflictReason::BothChanged);
        check(p, "conflict_reason");
        let mut p = base_plan.clone();
        p.entries[0].blocked_reason = Some("r".into());
        check(p, "blocked_reason");
        let mut p = base_plan.clone();
        p.base_adoptions[0].skill_id = "codex:other".into();
        check(p, "adoption skill_id");
        let mut p = base_plan.clone();
        p.base_adoptions[0].hash = "sha256:aaaa".into();
        check(p, "adoption hash");
        let mut p = base_plan.clone();
        p.base_removals.push("codex:extra".into());
        check(p, "removal membership");
    }

    #[test]
    fn plan_fingerprint_uses_explicit_nulls_and_excludes_display_warnings() {
        let mut plan = rich_plan();
        let fp1 = plan.plan_fingerprint.clone();
        // 纯展示 warnings 不影响 fingerprint
        plan.entries[0].warnings.push("display warning".into());
        plan.warnings.push("plan warning".into());
        let fp2 = compute_fingerprint(&plan).unwrap();
        assert_eq!(fp1, fp2);
        // optional 缺失序列化为显式 null
        let e = FpEntry {
            action_id: "x",
            namespace: "agents",
            folder_name: "f",
            relative_dir: None,
            status: SyncStatus::Synced,
            local_hash: None,
            remote_hash: None,
            base_hash: None,
            local_path: None,
            remote_blob: None,
            delete_direction: None,
            conflict_reason: None,
            blocked_reason: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"relative_dir\":null"));
        assert!(json.contains("\"local_hash\":null"));
        assert!(json.contains("\"conflict_reason\":null"));
    }

    #[test]
    fn plan_fingerprint_matches_golden_json_and_digest() {
        let plan = rich_plan();
        let json = String::from_utf8(canonical_fingerprint_bytes(&plan).unwrap()).unwrap();
        assert_eq!(json, GOLDEN_JSON);
        assert_eq!(plan.plan_fingerprint, GOLDEN_FP);
    }

    // golden vector 锁定 canonical serialization；rich_plan 含非空 adoption+removal。
    const GOLDEN_JSON: &str = "{\"expected_remote_commit\":\"commit-sha\",\"delete_guard_tripped\":false,\"base_adoptions\":[{\"skill_id\":\"codex:adv\",\"hash\":\"sha256:4444444444444444444444444444444444444444444444444444444444444444\"}],\"base_removals\":[\"codex:del\"],\"entries\":[{\"action_id\":\"both_deleted:codex:del\",\"namespace\":\"codex\",\"folder_name\":\"del\",\"relative_dir\":\"del\",\"status\":\"both_deleted\",\"local_hash\":null,\"remote_hash\":null,\"base_hash\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",\"local_path\":null,\"remote_blob\":null,\"delete_direction\":null,\"conflict_reason\":null,\"blocked_reason\":null},{\"action_id\":\"conflict:codex:cnf\",\"namespace\":\"codex\",\"folder_name\":\"cnf\",\"relative_dir\":\"cnf\",\"status\":\"conflict\",\"local_hash\":null,\"remote_hash\":\"sha256:2222222222222222222222222222222222222222222222222222222222222222\",\"base_hash\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",\"local_path\":null,\"remote_blob\":\"blobs/sha256/2222222222222222222222222222222222222222222222222222222222222222.skill.zip\",\"delete_direction\":\"delete_remote\",\"conflict_reason\":\"local_deleted_remote_changed\",\"blocked_reason\":null},{\"action_id\":\"synced:codex:adv\",\"namespace\":\"codex\",\"folder_name\":\"adv\",\"relative_dir\":\"adv\",\"status\":\"synced\",\"local_hash\":\"sha256:4444444444444444444444444444444444444444444444444444444444444444\",\"remote_hash\":\"sha256:4444444444444444444444444444444444444444444444444444444444444444\",\"base_hash\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",\"local_path\":\"/home/x/adv\",\"remote_blob\":\"blobs/sha256/4444444444444444444444444444444444444444444444444444444444444444.skill.zip\",\"delete_direction\":null,\"conflict_reason\":null,\"blocked_reason\":null}]}";
    const GOLDEN_FP: &str =
        "sha256:0668805e79203b0bbf4958e1ef2fe71a635deb9bd0586386325a27bb67be0eb9";

    #[test]
    fn duplicate_action_id_is_blocked() {
        fn dummy(action_id: &str) -> SyncSkillEntry {
            SyncSkillEntry {
                action_id: action_id.into(),
                skill_id: "codex:x".into(),
                name: String::new(),
                namespace: Codex,
                folder_name: String::new(),
                relative_dir: None,
                status: SyncStatus::Synced,
                local_hash: None,
                remote_hash: None,
                base_hash: None,
                local_path: None,
                remote_blob: None,
                conflict_reason: None,
                delete_direction: None,
                blocked_reason: None,
                warnings: Vec::new(),
            }
        }
        assert!(ensure_unique_action_ids(&[dummy("a:x"), dummy("a:x")]).is_err());
        assert!(ensure_unique_action_ids(&[dummy("a:x"), dummy("b:y")]).is_ok());
    }

    #[test]
    fn adoptions_and_removals_are_unique_disjoint_and_deduped() {
        let plan = rich_plan();
        let adopt_ids: HashSet<&str> = plan
            .base_adoptions
            .iter()
            .map(|a| a.skill_id.as_str())
            .collect();
        let rem_ids: HashSet<&str> = plan.base_removals.iter().map(|s| s.as_str()).collect();
        assert_eq!(adopt_ids.len(), plan.base_adoptions.len());
        assert_eq!(rem_ids.len(), plan.base_removals.len());
        assert!(adopt_ids.intersection(&rem_ids).next().is_none());
        assert_eq!(
            plan.commit_summary.local_state_updates,
            adopt_ids.len() + rem_ids.len()
        );
    }

    // ---- 删除护栏 ----

    #[test]
    fn unreadable_root_does_not_infer_local_deletes() {
        let roots = vec![
            root(Codex, true, false, false),
            root(Agents, true, true, true),
            root(ClaudeCode, true, true, true),
        ];
        let plan = merge_plan(
            &base_map(&[("codex:demo", HASH_B, Codex, "demo")]),
            &[],
            &manifest_of(&[vskill("codex:demo", HASH_B, Codex, "demo")]),
            &roots,
            &[],
            &LimitsConfig::default(),
            "c",
        )
        .unwrap();
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Unknown);
    }

    #[test]
    fn empty_scan_does_not_delete_tracked_skills() {
        // scan_complete=false 表示扫描未完成（空扫描），不得推断删除
        let roots = vec![
            root(Codex, true, true, false),
            root(Agents, true, true, true),
            root(ClaudeCode, true, true, true),
        ];
        let plan = merge_plan(
            &base_map(&[("codex:demo", HASH_B, Codex, "demo")]),
            &[],
            &manifest_of(&[]),
            &roots,
            &[],
            &LimitsConfig::default(),
            "c",
        )
        .unwrap();
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Unknown);
    }

    #[test]
    fn missing_namespace_status_does_not_infer_local_deletes() {
        let plan = merge_plan(
            &base_map(&[("codex:demo", HASH_B, Codex, "demo")]),
            &[],
            &manifest_of(&[vskill("codex:demo", HASH_B, Codex, "demo")]),
            &[],
            &[],
            &LimitsConfig::default(),
            "c",
        )
        .unwrap();
        assert_eq!(entry_of(&plan, "codex:demo").status, SyncStatus::Unknown);
    }

    #[test]
    fn delete_count_over_threshold_trips_guard() {
        // 11 个 local_deleted（row 9），max_auto_delete=10 -> 超过阈值
        let mut base_entries: Vec<(&str, &str, SkillNamespace, &str)> = Vec::new();
        let mut remote_skills: Vec<VaultSkill> = Vec::new();
        for i in 0..11 {
            let id = Box::leak(format!("codex:s{i}").into_boxed_str());
            let folder = Box::leak(format!("s{i}").into_boxed_str());
            base_entries.push((id, HASH_B, Codex, folder));
            remote_skills.push(vskill(id, HASH_B, Codex, folder));
        }
        let limits = LimitsConfig {
            max_auto_delete: 10,
            ..LimitsConfig::default()
        };
        let plan = merge_plan(
            &base_map(&base_entries),
            &[],
            &manifest_of(&remote_skills),
            &healthy_roots(),
            &[],
            &limits,
            "c",
        )
        .unwrap();
        assert!(plan.delete_guard_tripped);
    }

    #[test]
    fn delete_guard_counts_delete_side_conflicts() {
        // row 10 (local_deleted_remote_changed) 与 row 12 (remote_deleted_local_changed) 都算删除侧
        let base = base_map(&[
            ("codex:r10", HASH_B, Codex, "r10"),
            ("codex:r12", HASH_B, Codex, "r12"),
        ]);
        let local = vec![lid("codex:r12", HASH_L, Codex, "r12")]; // l!=b -> row 12
        let remote = vec![vskill("codex:r10", HASH_R, Codex, "r10")]; // r!=b -> row 10
        let limits = LimitsConfig {
            max_auto_delete: 1,
            ..LimitsConfig::default()
        };
        let plan = merge_plan(
            &base,
            &local,
            &manifest_of(&remote),
            &healthy_roots(),
            &[],
            &limits,
            "c",
        )
        .unwrap();
        assert!(plan.delete_guard_tripped);
    }

    #[test]
    fn unhealthy_namespace_only_blocks_deletes_for_that_namespace() {
        // Codex root 不健康 -> Codex 删除推断变 Unknown；Agents root 健康 -> Agents 删除正常
        let roots = vec![
            root(Agents, true, true, true),
            root(Codex, true, false, false),
            root(ClaudeCode, true, true, true),
        ];
        let base = base_map(&[
            ("agents:a", HASH_B, Agents, "a"),
            ("codex:c", HASH_B, Codex, "c"),
        ]);
        let remote = vec![
            vskill("agents:a", HASH_B, Agents, "a"),
            vskill("codex:c", HASH_B, Codex, "c"),
        ];
        let plan = merge_plan(
            &base,
            &[],
            &manifest_of(&remote),
            &roots,
            &[],
            &LimitsConfig::default(),
            "c",
        )
        .unwrap();
        assert_eq!(entry_of(&plan, "agents:a").status, SyncStatus::LocalDeleted);
        assert_eq!(entry_of(&plan, "codex:c").status, SyncStatus::Unknown);
    }

    #[test]
    fn scan_collision_blocks_all_involved_paths() {
        let collision = ScanCollision {
            namespace: Codex,
            collision_key: "demo".into(),
            kind: crate::detect::ScanCollisionKind::NormalizedId,
            skill_ids: vec!["codex:demo".into(), "codex:demo2".into()],
            paths: vec!["/x/demo".into(), "/x/demo2".into()],
        };
        let plan = merge_plan(
            &base_map(&[("codex:demo", HASH_B, Codex, "demo")]),
            &[],
            &manifest_of(&[vskill("codex:demo", HASH_B, Codex, "demo")]),
            &healthy_roots(),
            &[collision],
            &LimitsConfig::default(),
            "c",
        )
        .unwrap();
        assert_eq!(entry_of(&plan, "codex:demo").status, SyncStatus::Blocked);
        assert_eq!(entry_of(&plan, "codex:demo2").status, SyncStatus::Blocked);
        assert!(plan.blocked.iter().any(|b| b.skill_id == "codex:demo"));
        assert!(plan.blocked.iter().any(|b| b.skill_id == "codex:demo2"));
    }

    #[test]
    fn remote_or_merged_target_collision_blocks_all_involved_entries() {
        // local skill codex:foo (folder demo) 与 remote skill codex:bar (folder demo) 同 namespace 同 folded folder
        let plan = merge(
            &[],
            &[lid("codex:foo", HASH_L, Codex, "demo")],
            &[vskill("codex:bar", HASH_R, Codex, "demo")],
        );
        assert_eq!(entry_of(&plan, "codex:foo").status, SyncStatus::Blocked);
        assert_eq!(entry_of(&plan, "codex:bar").status, SyncStatus::Blocked);
        assert_eq!(plan.blocked.len(), 2);
    }

    #[test]
    fn blocked_local_pack_is_not_inferred_as_local_delete() {
        let mut local = lid("codex:demo", HASH_B, Codex, "demo");
        local.blocked_reason = Some("skill zip exceeds limit".into());
        let plan = merge(
            &[("codex:demo", HASH_B, Codex, "demo")],
            &[local],
            &[vskill("codex:demo", HASH_B, Codex, "demo")],
        );
        let e = entry_of(&plan, "codex:demo");
        assert_eq!(e.status, SyncStatus::Blocked);
        assert!(plan.delete_remote.is_empty());
        assert!(plan.base_adoptions.is_empty());
    }

    #[test]
    fn remote_entry_over_local_compressed_limit_is_blocked_before_blob_fetch() {
        let limits = LimitsConfig {
            max_skill_zip_bytes: 10,
            ..LimitsConfig::default()
        };
        let big = VaultSkill {
            size: 100,
            ..vskill("codex:demo", HASH_R, Codex, "demo")
        };
        let plan = merge_plan(
            &base_map(&[]),
            &[],
            &manifest_of(&[big]),
            &healthy_roots(),
            &[],
            &limits,
            "c",
        )
        .unwrap();
        assert_eq!(entry_of(&plan, "codex:demo").status, SyncStatus::Blocked);
    }

    // ---- build_plan 预检（async）----

    struct NoFetchStore;
    #[async_trait::async_trait]
    impl RemoteStore for NoFetchStore {
        async fn fetch_manifest(&self) -> Result<RemoteSnapshot> {
            Err(AppError::Vault("fetch_manifest must not be called".into()))
        }
        async fn fetch_blob(&self, _: &str, _: &str) -> Result<Vec<u8>> {
            Ok(vec![])
        }
        async fn commit_changes(&self, _: RemoteChanges) -> Result<RemoteCommit> {
            Ok(RemoteCommit {
                commit_sha: "c".into(),
            })
        }
    }

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

    #[tokio::test]
    async fn missing_remote_config_requires_onboarding() {
        let config = AppConfig::default_config(); // remote = None
        let state = SyncState::empty(remote_identity(1, 10, "main"));
        let err = build_plan(&config, &state, &NoFetchStore)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConfigured(_)));
    }

    #[tokio::test]
    async fn state_remote_identity_mismatch_is_blocked_before_fetch() {
        let mut config = AppConfig::default_config();
        config.remote = Some(remote_cfg(1, 10, "main"));
        let state = SyncState::empty(remote_identity(2, 10, "main")); // installation_id 不同
        let err = build_plan(&config, &state, &NoFetchStore)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Blocked(_)));
    }

    #[tokio::test]
    async fn invalid_limits_are_rejected_before_manifest_fetch() {
        let mut config = AppConfig::default_config();
        config.remote = Some(remote_cfg(1, 10, "main"));
        config.limits.max_skill_zip_bytes = 0;
        let state = SyncState::empty(remote_identity(1, 10, "main"));
        let err = build_plan(&config, &state, &NoFetchStore)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
    }

    // ---- Task 9: apply_plan 测试 ----

    use std::sync::{Arc, Mutex};

    enum CommitMode {
        Ok,
        RemoteChanged,
        OutcomeUnknown,
        DefiniteError,
    }

    impl Default for CommitMode {
        fn default() -> Self {
            CommitMode::Ok
        }
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
                branch: "local".into(),
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
                        let _ = std::fs::remove_file(&path);
                        let _ = std::fs::create_dir(&path);
                        return Err(AppError::Vault("definite commit error".into()));
                    }
                    JournalMutation::BreakSaveOnUnknown => {
                        let _ = std::fs::remove_dir_all(dir);
                        let _ = std::fs::File::create(dir);
                        return Err(AppError::RemoteOutcomeUnknown {
                            base_commit_sha: self.commit.clone(),
                            candidate_commit_sha: "candidate".into(),
                        });
                    }
                    JournalMutation::BreakStateSavingOnSuccess => {
                        let path = crate::local_apply::journal_path(dir);
                        let _ = std::fs::remove_file(&path);
                        let _ = std::fs::create_dir(&path);
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
                CommitMode::RemoteChanged => Err(AppError::RemoteChanged("remote changed".into())),
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
            skill_id: skill_id(ns, name),
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
            hex::encode(Sha256::digest(&captured.validated_bytes().unwrap()))
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
