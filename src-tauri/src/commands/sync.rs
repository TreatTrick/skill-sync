// 核心同步命令（get_app_state / save_config / scan_skills / get_sync_plan /
// apply_sync_plan / establish_baseline / resume_sync_recovery）与远端恢复裁决逻辑。

use super::*;
use crate::remote_store::{RemoteSnapshot, RemoteStore};
use crate::sync_engine::apply::apply_plan;
use crate::sync_engine::model::{
    ApplyResult, ApplySyncRequest, ApplySyncResponse, BaselineResult, PlanChangeReason, SyncPlan,
};
use crate::sync_engine::plan::build_plan;

#[tauri::command]
pub(crate) async fn get_app_state(state: State<'_, AppRuntime>) -> Result<AppState> {
    log_command_result("get_app_state", get_app_state_impl(&state).await)
}

pub(crate) async fn get_app_state_impl(runtime: &AppRuntime) -> Result<AppState> {
    let _gate = runtime.gate.inner.write().await;
    let config = load_config().await?;
    let config_dir = AppConfig::config_dir()
        .ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
    let pending = run_blocking({
        let config_dir = config_dir.clone();
        move || recover_pending(&config_dir)
    })
    .await?;
    let credential = runtime
        .credentials
        .valid_credential(&runtime.app_config.client_id)
        .await;
    let (github_authorized, github_user, credential_status) = match credential {
        Ok(credential) => (true, Some(credential.github_login), CredentialStatus::Valid),
        Err(AppError::ReauthorizationRequired(_)) => {
            (false, None, CredentialStatus::ReauthorizationRequired)
        }
        Err(_) => (false, None, CredentialStatus::Disconnected),
    };
    let (installation_id, repository_id, remote_owner, remote_repo, remote_branch) = config
        .remote
        .as_ref()
        .map(|remote| {
            (
                Some(remote.installation_id),
                Some(remote.repository_id),
                Some(remote.owner.clone()),
                Some(remote.repo.clone()),
                Some(remote.branch.clone()),
            )
        })
        .unwrap_or_default();
    let remote_commit = run_blocking(move || {
        Ok(SyncState::load_from(&config_dir)
            .ok()
            .map(|state| state.remote.commit_sha))
    })
    .await?;
    let device_name = config.device_id.clone();
    Ok(AppState {
        configured: config.is_configured(),
        config,
        github_authorized,
        github_user,
        github_app_slug: runtime
            .app_configured
            .then(|| runtime.app_config.slug.clone()),
        credential_status,
        installation_id,
        repository_id,
        remote_owner,
        remote_repo,
        remote_branch,
        vault_status: None,
        device_name,
        remote_commit,
        pending_recovery: pending.map(journal_to_recovery),
    })
}

#[tauri::command]
pub(crate) async fn save_config(state: State<'_, AppRuntime>, config: AppConfig) -> Result<()> {
    log_command_result("save_config", save_config_impl(&state, config).await)
}

pub(crate) async fn save_config_impl(runtime: &AppRuntime, config: AppConfig) -> Result<()> {
    let _gate = runtime.gate.inner.write().await;
    ensure_no_pending_recovery().await?;
    let current = load_config().await?;
    current.validate_save_candidate(&config)?;
    run_blocking(move || config.save()).await?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn scan_skills(state: State<'_, AppRuntime>) -> Result<ScanResult> {
    log_command_result("scan_skills", scan_skills_impl(&state).await)
}

pub(crate) async fn scan_skills_impl(runtime: &AppRuntime) -> Result<ScanResult> {
    let _gate = runtime.gate.inner.read().await;
    ensure_preview_has_no_pending_recovery().await?;
    let _config = load_config().await?;
    let home = dirs::home_dir().ok_or_else(|| AppError::Config("home not found".into()))?;
    run_blocking(move || crate::detect::scan_fixed_roots(&home)).await
}

#[tauri::command]
pub(crate) async fn get_sync_plan(state: State<'_, AppRuntime>) -> Result<SyncPlan> {
    log_command_result("get_sync_plan", get_sync_plan_impl(&state).await)
}

pub(crate) async fn get_sync_plan_impl(runtime: &AppRuntime) -> Result<SyncPlan> {
    let _gate = runtime.gate.inner.read().await;
    ensure_preview_has_no_pending_recovery().await?;
    let config = load_config().await?;
    let (store, state) = load_store_and_state(runtime, &config).await?;
    build_plan(&config, &state, &store).await
}

#[tauri::command]
pub(crate) async fn apply_sync_plan(
    state: State<'_, AppRuntime>,
    request: ApplySyncRequest,
) -> Result<ApplySyncResponse> {
    log_command_result(
        "apply_sync_plan",
        apply_sync_plan_impl(&state, request).await,
    )
}

pub(crate) async fn apply_sync_plan_impl(
    runtime: &AppRuntime,
    request: ApplySyncRequest,
) -> Result<ApplySyncResponse> {
    let _gate = runtime.gate.inner.write().await;
    let config_dir = config_dir()?;
    ensure_no_pending_recovery_at(&config_dir).await?;
    let config = load_config().await?;
    let (store, mut sync_state) = load_store_and_state(runtime, &config).await?;
    let home = dirs::home_dir().ok_or_else(|| AppError::Config("home not found".into()))?;
    apply_plan(
        &config,
        &mut sync_state,
        &request,
        &store,
        &home,
        &config_dir,
    )
    .await
}

#[tauri::command]
pub(crate) async fn establish_baseline(state: State<'_, AppRuntime>) -> Result<BaselineResult> {
    log_command_result("establish_baseline", establish_baseline_impl(&state).await)
}

pub(crate) async fn establish_baseline_impl(runtime: &AppRuntime) -> Result<BaselineResult> {
    let _gate = runtime.gate.inner.write().await;
    let config_dir = config_dir()?;
    ensure_no_pending_recovery_at(&config_dir).await?;
    let config = load_config().await?;
    let (store, mut sync_state) = load_store_and_state(runtime, &config).await?;
    let home = dirs::home_dir().ok_or_else(|| AppError::Config("home not found".into()))?;
    crate::sync_engine::apply::establish_baseline(
        &config,
        &mut sync_state,
        &store,
        &home,
        &config_dir,
    )
    .await
}

#[tauri::command]
pub(crate) async fn resume_sync_recovery(
    state: State<'_, AppRuntime>,
    task_id: String,
) -> Result<ApplySyncResponse> {
    log_command_result(
        "resume_sync_recovery",
        resume_sync_recovery_impl(&state, task_id).await,
    )
}

pub(crate) async fn resume_sync_recovery_impl(
    runtime: &AppRuntime,
    task_id: String,
) -> Result<ApplySyncResponse> {
    let _gate = runtime.gate.inner.write().await;
    let config_dir = config_dir()?;
    let pending = run_blocking({
        let config_dir = config_dir.clone();
        move || Ok(load_pending(&config_dir))
    })
    .await?
    .ok_or_else(|| AppError::RecoveryPending("no recovery is pending".into()))?;
    if pending.task_id != task_id {
        return Err(AppError::Blocked("recovery task id mismatch".into()));
    }
    // 先尝试本地 state_saving 恢复（rewrite state + clear）。
    let recovered = run_blocking({
        let config_dir = config_dir.clone();
        move || recover_pending(&config_dir)
    })
    .await?;
    match recovered {
        None => Ok(ApplySyncResponse::Applied {
            result: ApplyResult {
                applied: Vec::new(),
                state_updated: Vec::new(),
                warnings: vec!["recovery_completed".into()],
                remote_commit: pending.remote_candidate,
            },
        }),
        Some(journal) => {
            // remote phase：用一次不可变 snapshot 裁决。
            let config = load_config().await?;
            let (store, state) = load_store_and_state(runtime, &config).await?;
            resume_remote_recovery(&store, &config, &state, journal, &config_dir).await
        }
    }
}

/// 远端恢复裁决结果。
enum RemoteRecoveryDecision {
    /// HEAD == base：未发布，清 journal 并重建计划。
    Unpublished,
    /// HEAD == candidate 或 manifest hash 匹配：已发布，落盘预期状态。
    Published { commit_sha: String },
    /// HEAD 与已知结果都不匹配：保留 journal 并报告冲突。
    Conflict,
    /// 远端证据无法获取：保留 journal 并报告恢复。
    Unavailable,
}

/// 纯裁决：依据 journal 证据与不可变 snapshot 决定恢复动作。
/// 旧 journal（无 remote_base）从 next_state_bytes 读取 base commit。
fn reconcile_remote_journal(
    journal: &ApplyJournal,
    snapshot: Option<&RemoteSnapshot>,
) -> RemoteRecoveryDecision {
    let snapshot = match snapshot {
        Some(s) => s,
        None => return RemoteRecoveryDecision::Unavailable,
    };
    let base = journal.remote_base.clone().or_else(|| {
        let state: SyncState = serde_json::from_slice(&journal.next_state_bytes).ok()?;
        Some(state.remote.commit_sha)
    });
    let base = match base {
        Some(b) => b,
        None => return RemoteRecoveryDecision::Conflict,
    };
    if snapshot.commit_sha == base {
        return RemoteRecoveryDecision::Unpublished;
    }
    if journal.remote_candidate.as_deref() == Some(snapshot.commit_sha.as_str()) {
        return RemoteRecoveryDecision::Published {
            commit_sha: snapshot.commit_sha.clone(),
        };
    }
    if let Some(intended) = &journal.next_manifest_hash {
        if let Ok(bytes) = snapshot.manifest.validated_bytes() {
            let fetched = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
            if &fetched == intended {
                return RemoteRecoveryDecision::Published {
                    commit_sha: snapshot.commit_sha.clone(),
                };
            }
        }
    }
    RemoteRecoveryDecision::Conflict
}

/// 远端恢复编排：获取一次不可变 snapshot，按裁决 backup/clear/save 或重建计划。
async fn resume_remote_recovery<S: RemoteStore>(
    store: &S,
    config: &AppConfig,
    current_state: &SyncState,
    journal: ApplyJournal,
    config_dir: &std::path::Path,
) -> Result<ApplySyncResponse> {
    let snapshot = store.fetch_manifest().await.ok();
    let decision = reconcile_remote_journal(&journal, snapshot.as_ref());
    match decision {
        RemoteRecoveryDecision::Unavailable | RemoteRecoveryDecision::Conflict => {
            Ok(ApplySyncResponse::RecoveryRequired {
                recovery: journal_to_recovery(journal),
            })
        }
        RemoteRecoveryDecision::Unpublished => {
            backup_journal(config_dir)?;
            clear_journal(config_dir)?;
            let plan = build_plan(config, current_state, store).await?;
            Ok(ApplySyncResponse::PlanChanged {
                reason: PlanChangeReason::RemoteChanged,
                latest_plan: Box::new(plan),
            })
        }
        RemoteRecoveryDecision::Published { commit_sha } => {
            let mut intended: SyncState = serde_json::from_slice(&journal.next_state_bytes)
                .map_err(|e| AppError::Vault(format!("journal state decode failed: {e}")))?;
            intended.remote.commit_sha = commit_sha.clone();
            let state_dir = config_dir.to_path_buf();
            let state_to_save = intended.clone();
            let save_result =
                tauri::async_runtime::spawn_blocking(move || state_to_save.save_to(&state_dir))
                    .await
                    .map_err(|e| AppError::Vault(format!("state save task failed: {e}")))?;
            save_result?;
            backup_journal(config_dir)?;
            clear_journal(config_dir)?;
            let mut applied = journal.completed_action_ids.clone();
            applied.extend(journal.pending_action_ids.iter().cloned());
            Ok(ApplySyncResponse::Applied {
                result: ApplyResult {
                    applied,
                    state_updated: Vec::new(),
                    warnings: vec!["recovery_completed".into()],
                    remote_commit: Some(commit_sha),
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_send<T: Send>(_: T) {}

    #[test]
    fn sync_command_futures_are_send() {
        let runtime = AppRuntime::new().unwrap();
        let request = ApplySyncRequest {
            expected_remote_commit: "head".into(),
            plan_fingerprint: "fingerprint".into(),
            selected_action_ids: Vec::new(),
            decisions: std::collections::HashMap::new(),
            delete_guard_ack: false,
        };
        assert_send(get_sync_plan_impl(&runtime));
        assert_send(apply_sync_plan_impl(&runtime, request));
    }

    // ---- 远端恢复裁决 ----

    use crate::remote_store::{RemoteChanges, RemoteCommit};
    use crate::skill::SkillNamespace;
    use crate::sync_state::SkillSyncState;
    use crate::vault_manifest::VaultManifest;

    fn rec_remote() -> RemoteConfig {
        RemoteConfig {
            installation_id: 1,
            repository_id: 10,
            owner: "o".into(),
            repo: "r".into(),
            branch: "main".into(),
        }
    }

    fn rec_identity(commit: &str) -> RemoteIdentity {
        RemoteIdentity {
            provider: "github".into(),
            installation_id: 1,
            repository_id: 10,
            owner: "o".into(),
            repo: "r".into(),
            branch: "main".into(),
            commit_sha: commit.into(),
        }
    }

    fn rec_config() -> AppConfig {
        let mut c = AppConfig::default_config();
        c.remote = Some(rec_remote());
        c
    }

    fn rec_state(commit: &str) -> SyncState {
        SyncState::empty(rec_identity(commit))
    }

    /// 构造恢复 journal：base/candidate/manifest_hash 与 embedded state commit 可控。
    fn rec_journal(
        base: Option<&str>,
        candidate: Option<&str>,
        manifest_hash: Option<&str>,
        state_commit: &str,
    ) -> ApplyJournal {
        let state = rec_state(state_commit);
        let state_bytes = serde_json::to_vec(&state).unwrap();
        ApplyJournal {
            schema: 1,
            task_id: "t".into(),
            phase: "remote_outcome_unknown".into(),
            remote_candidate: candidate.map(String::from),
            next_state_bytes: state_bytes,
            next_state_hash: "sha256:x".into(),
            remote_base: base.map(String::from),
            next_manifest_hash: manifest_hash.map(String::from),
            completed_action_ids: vec![],
            pending_action_ids: vec![],
        }
    }

    fn snap(sha: &str, manifest: VaultManifest) -> RemoteSnapshot {
        RemoteSnapshot {
            manifest,
            commit_sha: sha.into(),
        }
    }

    #[test]
    fn reconcile_unpublished_when_head_equals_base() {
        let journal = rec_journal(Some("base"), Some("candidate"), None, "base");
        let snapshot = snap("base", VaultManifest::empty("d"));
        assert!(matches!(
            reconcile_remote_journal(&journal, Some(&snapshot)),
            RemoteRecoveryDecision::Unpublished
        ));
    }

    #[test]
    fn reconcile_published_when_head_equals_candidate() {
        let journal = rec_journal(Some("base"), Some("candidate"), None, "base");
        let snapshot = snap("candidate", VaultManifest::empty("d"));
        assert!(matches!(
            reconcile_remote_journal(&journal, Some(&snapshot)),
            RemoteRecoveryDecision::Published { commit_sha } if commit_sha == "candidate"
        ));
    }

    #[test]
    fn reconcile_published_when_manifest_hash_matches() {
        let manifest = VaultManifest::empty("d");
        let hash = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(manifest.validated_bytes().unwrap()))
        );
        let journal = rec_journal(Some("base"), Some("candidate"), Some(&hash), "base");
        let snapshot = snap("other", manifest);
        assert!(matches!(
            reconcile_remote_journal(&journal, Some(&snapshot)),
            RemoteRecoveryDecision::Published { commit_sha } if commit_sha == "other"
        ));
    }

    #[test]
    fn reconcile_conflict_when_head_matches_neither() {
        let journal = rec_journal(Some("base"), Some("candidate"), None, "base");
        let snapshot = snap("other", VaultManifest::empty("d"));
        assert!(matches!(
            reconcile_remote_journal(&journal, Some(&snapshot)),
            RemoteRecoveryDecision::Conflict
        ));
    }

    #[test]
    fn reconcile_unavailable_when_snapshot_missing() {
        let journal = rec_journal(Some("base"), Some("candidate"), None, "base");
        assert!(matches!(
            reconcile_remote_journal(&journal, None),
            RemoteRecoveryDecision::Unavailable
        ));
    }

    #[test]
    fn reconcile_legacy_journal_uses_embedded_state_base() {
        // legacy: remote_base None -> 从 next_state_bytes 读 base commit
        let journal = rec_journal(None, None, None, "base");
        let unpublished = snap("base", VaultManifest::empty("d"));
        assert!(matches!(
            reconcile_remote_journal(&journal, Some(&unpublished)),
            RemoteRecoveryDecision::Unpublished
        ));
        let conflict = snap("other", VaultManifest::empty("d"));
        assert!(matches!(
            reconcile_remote_journal(&journal, Some(&conflict)),
            RemoteRecoveryDecision::Conflict
        ));
    }

    struct RecoveryMockStore {
        snapshot: Option<RemoteSnapshot>,
    }

    #[async_trait::async_trait]
    impl RemoteStore for RecoveryMockStore {
        async fn fetch_manifest(&self) -> Result<RemoteSnapshot> {
            self.snapshot
                .clone()
                .ok_or_else(|| AppError::Vault("fetch unavailable".into()))
        }
        async fn fetch_blob(&self, _: &str, _: &str) -> Result<Vec<u8>> {
            Ok(vec![])
        }
        async fn commit_changes(&self, _: RemoteChanges) -> Result<RemoteCommit> {
            unreachable!("resume must not commit")
        }
    }

    #[tokio::test]
    async fn resume_unpublished_backs_up_clears_and_returns_plan_changed() {
        let cfgdir = tempfile::tempdir().unwrap();
        let journal = rec_journal(Some("commit-1"), Some("candidate"), None, "commit-1");
        crate::local_apply::save_journal(cfgdir.path(), &journal).unwrap();
        let store = RecoveryMockStore {
            snapshot: Some(snap("commit-1", VaultManifest::empty("d"))),
        };
        let config = rec_config();
        let state = rec_state("commit-1");
        let resp = resume_remote_recovery(&store, &config, &state, journal, cfgdir.path())
            .await
            .unwrap();
        assert!(matches!(resp, ApplySyncResponse::PlanChanged { .. }));
        // active journal 已清
        assert!(crate::local_apply::load_pending(cfgdir.path()).is_none());
        // 备份已写入 recovery-backups
        let backup_dir = cfgdir.path().join("recovery-backups");
        assert!(backup_dir.read_dir().unwrap().next().is_some());
    }

    #[tokio::test]
    async fn resume_published_saves_intended_state_and_returns_applied() {
        let cfgdir = tempfile::tempdir().unwrap();
        // 预期状态含一个 adoption skill
        let mut intended = rec_state("commit-1");
        intended.skills.insert(
            "codex:demo".into(),
            SkillSyncState {
                base_hash: "sha256:h".into(),
                last_remote_hash: "sha256:h".into(),
                last_synced_at: String::new(),
                namespace: SkillNamespace::Codex,
                relative_dir: "demo".into(),
            },
        );
        let state_bytes = serde_json::to_vec(&intended).unwrap();
        let journal = ApplyJournal {
            schema: 1,
            task_id: "t".into(),
            phase: "remote_outcome_unknown".into(),
            remote_candidate: Some("candidate".into()),
            next_state_bytes: state_bytes,
            next_state_hash: "sha256:x".into(),
            remote_base: Some("commit-1".into()),
            next_manifest_hash: None,
            completed_action_ids: vec!["codex:done".into()],
            pending_action_ids: vec!["codex:demo".into()],
        };
        crate::local_apply::save_journal(cfgdir.path(), &journal).unwrap();
        let store = RecoveryMockStore {
            snapshot: Some(snap("candidate", VaultManifest::empty("d"))),
        };
        let config = rec_config();
        let state = rec_state("commit-1");
        let resp = resume_remote_recovery(&store, &config, &state, journal, cfgdir.path())
            .await
            .unwrap();
        let remote_commit = match resp {
            ApplySyncResponse::Applied { result } => {
                assert!(result.applied.contains(&"codex:demo".to_string()));
                assert!(result.applied.contains(&"codex:done".to_string()));
                result.remote_commit
            }
            _ => panic!("expected Applied"),
        };
        assert_eq!(remote_commit.as_deref(), Some("candidate"));
        // 落盘状态：保留 adoption skill，commit_sha 替换为 candidate
        let saved = SyncState::load_from(cfgdir.path()).unwrap();
        assert!(saved.skills.contains_key("codex:demo"));
        assert_eq!(saved.remote.commit_sha, "candidate");
        // active journal 已清
        assert!(crate::local_apply::load_pending(cfgdir.path()).is_none());
    }

    #[tokio::test]
    async fn resume_conflict_retains_journal_and_returns_recovery_required() {
        let cfgdir = tempfile::tempdir().unwrap();
        let journal = rec_journal(Some("commit-1"), Some("candidate"), None, "commit-1");
        crate::local_apply::save_journal(cfgdir.path(), &journal).unwrap();
        let store = RecoveryMockStore {
            snapshot: Some(snap("other", VaultManifest::empty("d"))),
        };
        let config = rec_config();
        let state = rec_state("commit-1");
        let resp = resume_remote_recovery(&store, &config, &state, journal, cfgdir.path())
            .await
            .unwrap();
        assert!(matches!(resp, ApplySyncResponse::RecoveryRequired { .. }));
        // active journal 保留
        assert!(crate::local_apply::load_pending(cfgdir.path()).is_some());
    }

    #[tokio::test]
    async fn resume_unavailable_retains_journal_and_returns_recovery_required() {
        let cfgdir = tempfile::tempdir().unwrap();
        let journal = rec_journal(Some("commit-1"), Some("candidate"), None, "commit-1");
        crate::local_apply::save_journal(cfgdir.path(), &journal).unwrap();
        let store = RecoveryMockStore { snapshot: None };
        let config = rec_config();
        let state = rec_state("commit-1");
        let resp = resume_remote_recovery(&store, &config, &state, journal, cfgdir.path())
            .await
            .unwrap();
        assert!(matches!(resp, ApplySyncResponse::RecoveryRequired { .. }));
        assert!(crate::local_apply::load_pending(cfgdir.path()).is_some());
    }
}
