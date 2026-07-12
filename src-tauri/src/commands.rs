#![allow(clippy::let_underscore_must_use)]

use std::process::Command;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::config::{AppConfig, RemoteConfig};
use crate::detect::ScanResult;
use crate::errors::{AppError, Result};
use crate::github_app_config::GithubAppPublicConfig;
use crate::github_auth::{DeviceFlowStart, GithubAuthClient, InternalPollResult};
use crate::github_credentials::{
    CredentialStore, GithubAuthenticatedClient, GithubCredentialManager, KeyringCredentialStore,
};
use crate::github_repository::{
    GithubRepositoryDiscovery, GithubRepositoryService, GithubVaultCheck, GithubVaultStatus,
    InitializeGithubVaultRequest,
};
use crate::github_store::GitHubVaultStore;
use crate::local_apply::{
    backup_journal, clear_journal, load_pending, recover_pending, ApplyJournal,
};
use crate::logging;
use crate::remote_store::{RemoteSnapshot, RemoteStore};
use crate::sync_engine::vault::{
    self, ApplyResult, ApplySyncRequest, ApplySyncResponse, PlanChangeReason, RecoveryInfo,
    RecoveryPhase, SyncPlan,
};
use crate::sync_state::{RemoteIdentity, SyncState};
use crate::vault_binding::VaultBindingStore;
use sha2::{Digest, Sha256};

#[derive(Default)]
pub(crate) struct SyncOperationGate {
    pub(crate) inner: tokio::sync::RwLock<()>,
}

pub(crate) struct AppRuntime {
    pub(crate) gate: SyncOperationGate,
    pub(crate) app_config: GithubAppPublicConfig,
    pub(crate) app_configured: bool,
    pub(crate) auth: Arc<GithubAuthClient>,
    pub(crate) credentials: Arc<GithubCredentialManager>,
    pub(crate) client: Arc<GithubAuthenticatedClient>,
    pub(crate) repository: Arc<GithubRepositoryService>,
}

fn log_command_result<T>(command: &str, result: Result<T>) -> Result<T> {
    result.inspect_err(|error| {
        logging::log_app_error(command, error);
    })
}

impl AppRuntime {
    pub(crate) fn new() -> Result<Self> {
        let (app_config, app_configured) = match GithubAppPublicConfig::embedded() {
            Ok(config) => (config, true),
            Err(_) => (
                GithubAppPublicConfig::new("unconfigured", "skill-sync")?,
                false,
            ),
        };
        let auth = Arc::new(GithubAuthClient::new(app_config.clone())?);
        let credential_store: Arc<dyn CredentialStore> = Arc::new(KeyringCredentialStore::new());
        let credentials = Arc::new(GithubCredentialManager::new(credential_store, auth.clone()));
        let client = Arc::new(GithubAuthenticatedClient::new(
            credentials.clone(),
            app_config.client_id.clone(),
        )?);
        let device_id = AppConfig::load()
            .map(|config| config.device_id)
            .unwrap_or_else(|_| Uuid::new_v4().to_string());
        let repository = Arc::new(GithubRepositoryService::new(
            client.clone(),
            app_config.clone(),
            device_id,
            "https://api.github.com".into(),
        ));
        Ok(Self {
            gate: SyncOperationGate::default(),
            app_config,
            app_configured,
            auth,
            credentials,
            client,
            repository,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CredentialStatus {
    Disconnected,
    Valid,
    #[allow(dead_code)]
    Refreshing,
    ReauthorizationRequired,
}

#[derive(Serialize)]
pub(crate) struct AppState {
    pub configured: bool,
    pub config: AppConfig,
    pub github_authorized: bool,
    pub github_user: Option<String>,
    pub github_app_slug: Option<String>,
    pub credential_status: CredentialStatus,
    pub installation_id: Option<u64>,
    pub repository_id: Option<u64>,
    pub remote_owner: Option<String>,
    pub remote_repo: Option<String>,
    pub remote_branch: Option<String>,
    pub vault_status: Option<GithubVaultStatus>,
    pub device_name: String,
    pub remote_commit: Option<String>,
    pub pending_recovery: Option<RecoveryInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GithubAppInfo {
    pub configured: bool,
    pub app_slug: Option<String>,
    pub install_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum GithubDeviceFlowPollResponse {
    Pending { interval: u64 },
    SlowDown { interval: u64 },
    Authorized { github_user: String },
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RemoteBindingKey {
    pub installation_id: u64,
    pub repository_id: u64,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BindGithubVaultRequest {
    pub remote: RemoteConfig,
    pub expected_head_sha: String,
    pub expected_manifest_sha: String,
    pub expected_previous_binding: Option<RemoteBindingKey>,
    pub confirm_rebind: bool,
}

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
    vault::build_plan(&config, &state, &store).await
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
    vault::apply_plan(
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
            let plan = vault::build_plan(config, current_state, store).await?;
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

#[tauri::command]
pub(crate) fn open_path(path: String) -> Result<()> {
    log_command_result("open_path", open_path_platform(&path))
}

#[tauri::command]
pub(crate) async fn start_github_device_flow(
    state: State<'_, AppRuntime>,
) -> Result<DeviceFlowStart> {
    let result = if !state.app_configured {
        Err(AppError::NotConfigured(
            "github app public config is not embedded".into(),
        ))
    } else {
        state.auth.start().await
    };
    log_command_result("start_github_device_flow", result)
}

#[tauri::command]
pub(crate) async fn poll_github_device_flow(
    state: State<'_, AppRuntime>,
    device_code: String,
    _interval: u64,
) -> Result<GithubDeviceFlowPollResponse> {
    let result = async {
        match state.auth.poll(&device_code).await {
            Ok(InternalPollResult::Pending { interval }) => {
                Ok(GithubDeviceFlowPollResponse::Pending { interval })
            }
            Ok(InternalPollResult::SlowDown { interval }) => {
                Ok(GithubDeviceFlowPollResponse::SlowDown { interval })
            }
            Ok(InternalPollResult::Denied) => Ok(GithubDeviceFlowPollResponse::Denied),
            Ok(InternalPollResult::Success {
                access_token,
                refresh_token,
                access_expires_in,
                refresh_token_expires_in,
            }) => {
                let credential = state
                    .auth
                    .build_credential(
                        access_token,
                        refresh_token,
                        access_expires_in,
                        refresh_token_expires_in,
                    )
                    .await?;
                let github_user = credential.github_login.clone();
                state.credentials.save_initial(&credential).await?;
                Ok(GithubDeviceFlowPollResponse::Authorized { github_user })
            }
            Err(error) => Err(error),
        }
    }
    .await;
    log_command_result("poll_github_device_flow", result)
}

#[tauri::command]
pub(crate) fn get_github_app_info(state: State<'_, AppRuntime>) -> GithubAppInfo {
    GithubAppInfo {
        configured: state.app_configured,
        app_slug: state.app_configured.then(|| state.app_config.slug.clone()),
        install_url: state.app_configured.then(|| {
            format!(
                "https://github.com/apps/{}/installations/new",
                state.app_config.slug
            )
        }),
    }
}

#[tauri::command]
pub(crate) async fn list_github_installations(
    state: State<'_, AppRuntime>,
) -> Result<Vec<serde_json::Value>> {
    log_command_result(
        "list_github_installations",
        list_json(&state.client, "/user/installations").await,
    )
}

#[tauri::command]
pub(crate) async fn list_installation_repositories(
    state: State<'_, AppRuntime>,
    installation_id: u64,
) -> Result<Vec<serde_json::Value>> {
    log_command_result(
        "list_installation_repositories",
        list_json(
            &state.client,
            &format!("/user/installations/{installation_id}/repositories"),
        )
        .await,
    )
}

#[tauri::command]
pub(crate) async fn discover_single_github_repository(
    state: State<'_, AppRuntime>,
) -> Result<GithubRepositoryDiscovery> {
    log_command_result(
        "discover_single_github_repository",
        state.repository.discover_single_repository().await,
    )
}

#[tauri::command]
pub(crate) async fn list_github_repository_branches(
    state: State<'_, AppRuntime>,
    remote: RemoteConfig,
) -> Result<Vec<String>> {
    log_command_result(
        "list_github_repository_branches",
        state.repository.list_branches(&remote).await,
    )
}

#[tauri::command]
pub(crate) async fn check_github_vault(
    state: State<'_, AppRuntime>,
    remote: RemoteConfig,
) -> Result<GithubVaultCheck> {
    let result = async {
        let _gate = state.gate.inner.read().await;
        ensure_preview_has_no_pending_recovery().await?;
        state.repository.check_vault(&remote).await
    }
    .await;
    log_command_result("check_github_vault", result)
}

#[tauri::command]
pub(crate) async fn initialize_github_vault(
    state: State<'_, AppRuntime>,
    request: InitializeGithubVaultRequest,
) -> Result<GithubVaultCheck> {
    let result = async {
        let _gate = state.gate.inner.write().await;
        ensure_no_pending_recovery().await?;
        state.repository.initialize_vault(request).await
    }
    .await;
    log_command_result("initialize_github_vault", result)
}

#[tauri::command]
pub(crate) async fn bind_github_vault(
    state: State<'_, AppRuntime>,
    request: BindGithubVaultRequest,
) -> Result<GithubVaultCheck> {
    log_command_result(
        "bind_github_vault",
        bind_github_vault_impl(&state, request).await,
    )
}

pub(crate) async fn bind_github_vault_impl(
    runtime: &AppRuntime,
    request: BindGithubVaultRequest,
) -> Result<GithubVaultCheck> {
    let _gate = runtime.gate.inner.write().await;
    ensure_no_pending_recovery().await?;
    let config = load_config().await?;
    validate_expected_binding(
        config.remote.as_ref(),
        request.expected_previous_binding.as_ref(),
    )?;
    let check = runtime.repository.check_vault(&request.remote).await?;
    if check.status != GithubVaultStatus::Ready
        || check.head_sha.as_deref() != Some(request.expected_head_sha.as_str())
        || check.manifest_sha.as_deref() != Some(request.expected_manifest_sha.as_str())
    {
        let latest_check = match serde_json::to_value(&check) {
            Ok(value) => value,
            Err(_) => serde_json::json!({ "status": "unavailable" }),
        };
        return Err(AppError::VaultStateChangedWithCheck {
            message: "expected vault state is stale".into(),
            latest_check,
        });
    }
    let context = runtime
        .repository
        .validate_for_side_effect(&request.remote)
        .await?;
    if context.installation_id != request.remote.installation_id {
        return Err(AppError::Blocked("installation identity mismatch".into()));
    }

    let config_dir = config_dir()?;
    let (previous_config, previous_state, next_config, next_state, history) = run_blocking({
        let config_dir = config_dir.clone();
        let config = config.clone();
        let request = request.clone();
        move || {
            let previous_config = std::fs::read(
                AppConfig::config_path()
                    .ok_or_else(|| AppError::Config("cannot determine config path".into()))?,
            )
            .ok();
            let previous_state = std::fs::read(config_dir.join("sync_state.json")).ok();
            let same_binding = config.remote.as_ref().map(remote_binding_key)
                == Some(remote_binding_key(&request.remote));
            if config.remote.is_some() && !same_binding && !request.confirm_rebind {
                return Err(AppError::Blocked("confirm_rebind is required".into()));
            }
            let next_state = if same_binding {
                let mut state = SyncState::load_from(&config_dir)?;
                state.remote = remote_identity(&request.remote, request.expected_head_sha.clone());
                Some(serde_json::to_vec(&state).map_err(|e| AppError::Vault(e.to_string()))?)
            } else {
                let state = SyncState::empty(remote_identity(
                    &request.remote,
                    request.expected_head_sha.clone(),
                ));
                Some(serde_json::to_vec(&state).map_err(|e| AppError::Vault(e.to_string()))?)
            };
            let mut next_config = config;
            next_config.remote = Some(request.remote);
            let next_config = serde_yaml::to_string(&next_config)
                .map(|text| text.into_bytes())
                .map_err(|e| AppError::Config(e.to_string()))?;
            let history = if !same_binding {
                previous_state
                    .clone()
                    .map(|bytes| (format!("history/rebind-{}.json", Uuid::new_v4()), bytes))
            } else {
                None
            };
            Ok((
                previous_config,
                previous_state,
                next_config,
                next_state,
                history,
            ))
        }
    })
    .await?;
    run_blocking(move || {
        VaultBindingStore::commit_bytes(
            &config_dir,
            previous_config,
            previous_state,
            next_config,
            next_state,
            history,
        )
    })
    .await?;
    Ok(check)
}

#[tauri::command]
pub(crate) async fn disconnect_github(
    state: State<'_, AppRuntime>,
    expected_repository_id: u64,
) -> Result<()> {
    let result = async {
        let _gate = state.gate.inner.write().await;
        ensure_no_pending_recovery().await?;
        let config = load_config().await?;
        let remote = config
            .remote
            .as_ref()
            .ok_or_else(|| AppError::NotConfigured("github vault is not bound".into()))?;
        if remote.repository_id != expected_repository_id {
            return Err(AppError::Blocked("repository identity mismatch".into()));
        }
        let config_dir = config_dir()?;
        let (previous_config, previous_state, next_config) = run_blocking({
            let config_dir = config_dir.clone();
            let config = config.clone();
            move || {
                let previous_config = std::fs::read(
                    AppConfig::config_path()
                        .ok_or_else(|| AppError::Config("cannot determine config path".into()))?,
                )
                .ok();
                let previous_state = std::fs::read(config_dir.join("sync_state.json")).ok();
                let mut next_config = config;
                next_config.remote = None;
                let next_config = serde_yaml::to_string(&next_config)
                    .map(|text| text.into_bytes())
                    .map_err(|e| AppError::Config(e.to_string()))?;
                Ok((previous_config, previous_state, next_config))
            }
        })
        .await?;
        let history = previous_state
            .clone()
            .map(|bytes| (format!("history/disconnect-{}.json", Uuid::new_v4()), bytes));
        run_blocking(move || {
            VaultBindingStore::commit_bytes(
                &config_dir,
                previous_config,
                previous_state,
                next_config,
                None,
                history,
            )
        })
        .await?;
        drop(state.credentials.clear().await);
        Ok(())
    }
    .await;
    log_command_result("disconnect_github", result)
}

#[tauri::command]
pub(crate) async fn list_remote_skills(
    state: State<'_, AppRuntime>,
) -> Result<Vec<crate::vault_manifest::VaultSkill>> {
    let result = async {
        let _gate = state.gate.inner.read().await;
        ensure_preview_has_no_pending_recovery().await?;
        let config = load_config().await?;
        let (store, _) = load_store_and_state(&state, &config).await?;
        Ok(store
            .fetch_manifest()
            .await?
            .manifest
            .skills
            .into_values()
            .collect())
    }
    .await;
    log_command_result("list_remote_skills", result)
}

#[tauri::command]
pub(crate) async fn upload_skills(
    state: State<'_, AppRuntime>,
    skill_ids: Vec<String>,
) -> Result<ApplySyncResponse> {
    log_command_result("upload_skills", batch_sync(&state, skill_ids, true).await)
}

#[tauri::command]
pub(crate) async fn download_skills(
    state: State<'_, AppRuntime>,
    skill_ids: Vec<String>,
) -> Result<ApplySyncResponse> {
    log_command_result(
        "download_skills",
        batch_sync(&state, skill_ids, false).await,
    )
}

async fn batch_sync(
    runtime: &AppRuntime,
    skill_ids: Vec<String>,
    upload: bool,
) -> Result<ApplySyncResponse> {
    let _gate = runtime.gate.inner.write().await;
    let config_dir = config_dir()?;
    ensure_no_pending_recovery_at(&config_dir).await?;
    let config = load_config().await?;
    let (store, mut sync_state) = load_store_and_state(runtime, &config).await?;
    let home = dirs::home_dir().ok_or_else(|| AppError::Config("home not found".into()))?;
    if upload {
        vault::upload_skills(
            &skill_ids,
            &config,
            &mut sync_state,
            &store,
            &home,
            &config_dir,
        )
        .await
    } else {
        vault::download_skills(
            &skill_ids,
            &config,
            &mut sync_state,
            &store,
            &home,
            &config_dir,
        )
        .await
    }
}

async fn load_store_and_state(
    runtime: &AppRuntime,
    config: &AppConfig,
) -> Result<(GitHubVaultStore, SyncState)> {
    let remote = config
        .remote
        .as_ref()
        .ok_or_else(|| {
            AppError::NotConfigured("remote not configured; onboarding required".into())
        })?
        .clone();
    let config_dir = config_dir()?;
    let state_remote = remote.clone();
    let state = run_blocking(move || {
        let expected = remote_identity(&state_remote, String::new());
        SyncState::load_and_validate(&config_dir, &expected)
    })
    .await?;
    let context = runtime.repository.validate_for_side_effect(&remote).await?;
    Ok((
        GitHubVaultStore::new(runtime.client.clone(), context, config.device_id.clone()),
        state,
    ))
}

async fn load_config() -> Result<AppConfig> {
    run_blocking(AppConfig::load).await
}

async fn ensure_no_pending_recovery() -> Result<()> {
    ensure_no_pending_recovery_at(&config_dir()?).await
}

async fn ensure_no_pending_recovery_at(config_dir: &std::path::Path) -> Result<()> {
    let config_dir = config_dir.to_path_buf();
    let pending = run_blocking(move || recover_pending(&config_dir)).await?;
    if let Some(journal) = pending {
        return Err(AppError::RecoveryPending(format!(
            "recovery task {} is pending",
            journal.task_id
        )));
    }
    Ok(())
}

async fn ensure_preview_has_no_pending_recovery() -> Result<()> {
    let config_dir = config_dir()?;
    let pending = run_blocking(move || Ok(load_pending(&config_dir))).await?;
    if let Some(journal) = pending {
        return Err(AppError::RecoveryPending(format!(
            "recovery task {} is pending",
            journal.task_id
        )));
    }
    Ok(())
}

fn config_dir() -> Result<std::path::PathBuf> {
    AppConfig::config_dir().ok_or_else(|| AppError::Config("cannot determine config dir".into()))
}

async fn run_blocking<T, F>(operation: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| AppError::Other(format!("blocking task failed: {error}")))?
}

async fn list_json(
    client: &GithubAuthenticatedClient,
    path: &str,
) -> Result<Vec<serde_json::Value>> {
    let response = client.get_path(path).await?;
    if !response.status().is_success() {
        return Err(AppError::Vault(format!(
            "GitHub request failed: {}",
            response.status()
        )));
    }
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|error| AppError::Vault(format!("GitHub response decode failed: {error}")))?;
    value
        .get("installations")
        .or_else(|| value.get("repositories"))
        .and_then(|items| items.as_array())
        .cloned()
        .or_else(|| value.as_array().cloned())
        .ok_or_else(|| AppError::Vault("GitHub response is not an array".into()))
}

fn remote_binding_key(remote: &RemoteConfig) -> RemoteBindingKey {
    RemoteBindingKey {
        installation_id: remote.installation_id,
        repository_id: remote.repository_id,
        branch: remote.branch.clone(),
    }
}

fn validate_expected_binding(
    current: Option<&RemoteConfig>,
    expected: Option<&RemoteBindingKey>,
) -> Result<()> {
    if current.map(remote_binding_key) != expected.cloned() {
        return Err(AppError::Blocked("stale binding".into()));
    }
    Ok(())
}

fn remote_identity(remote: &RemoteConfig, commit_sha: String) -> RemoteIdentity {
    RemoteIdentity {
        provider: "github".into(),
        installation_id: remote.installation_id,
        repository_id: remote.repository_id,
        owner: remote.owner.clone(),
        repo: remote.repo.clone(),
        branch: remote.branch.clone(),
        commit_sha,
    }
}

fn journal_to_recovery(journal: crate::local_apply::ApplyJournal) -> RecoveryInfo {
    let phase = match journal.phase.as_str() {
        "remote_committing" | "remote_outcome_unknown" => RecoveryPhase::RemoteOutcomeUnknown,
        "local_replace_failed" => RecoveryPhase::LocalReplaceFailed,
        "trash_move_failed" => RecoveryPhase::TrashMoveFailed,
        "state_saving" => RecoveryPhase::StateSaveFailed,
        _ => RecoveryPhase::RemoteOutcomeUnknown,
    };
    RecoveryInfo {
        task_id: journal.task_id,
        phase,
        remote_commit: journal.remote_candidate,
        completed_action_ids: journal.completed_action_ids,
        pending_action_ids: journal.pending_action_ids,
        message: "recovery is pending; resume is required".into(),
    }
}

#[cfg(target_os = "windows")]
fn open_path_platform(path: &str) -> Result<()> {
    Command::new("explorer")
        .arg(path)
        .spawn()
        .map_err(|error| AppError::Io(error.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Notify;

    fn empty_plan() -> SyncPlan {
        SyncPlan {
            entries: Vec::new(),
            uploads: Vec::new(),
            downloads: Vec::new(),
            delete_remote: Vec::new(),
            delete_local: Vec::new(),
            conflicts: Vec::new(),
            blocked: Vec::new(),
            warnings: Vec::new(),
            delete_guard_tripped: false,
            expected_remote_commit: "head".into(),
            plan_fingerprint: "fingerprint".into(),
            base_adoptions: Vec::new(),
            base_removals: Vec::new(),
            will_create_commit: false,
            commit_summary: vault::CommitSummary {
                uploads: 0,
                downloads: 0,
                delete_remote: 0,
                delete_local: 0,
                local_state_updates: 0,
            },
        }
    }

    #[test]
    fn apply_response_variants_serialize_with_status_tags() {
        let applied = serde_json::to_value(ApplySyncResponse::Applied {
            result: ApplyResult {
                applied: vec!["agents:demo".into()],
                state_updated: Vec::new(),
                warnings: Vec::new(),
                remote_commit: Some("commit".into()),
            },
        })
        .unwrap();
        assert_eq!(applied["status"], "applied");

        let changed = serde_json::to_value(ApplySyncResponse::PlanChanged {
            reason: vault::PlanChangeReason::RemoteChanged,
            latest_plan: Box::new(empty_plan()),
        })
        .unwrap();
        assert_eq!(changed["status"], "plan_changed");

        let recovery = serde_json::to_value(ApplySyncResponse::RecoveryRequired {
            recovery: RecoveryInfo {
                task_id: "task".into(),
                phase: RecoveryPhase::RemoteOutcomeUnknown,
                remote_commit: None,
                completed_action_ids: Vec::new(),
                pending_action_ids: Vec::new(),
                message: "pending".into(),
            },
        })
        .unwrap();
        assert_eq!(recovery["status"], "recovery_required");
    }

    #[test]
    fn device_flow_public_response_contains_no_token_fields() {
        let value = serde_json::to_string(&GithubDeviceFlowPollResponse::Authorized {
            github_user: "octocat".into(),
        })
        .unwrap();
        assert!(!value.contains("access_token"));
        assert!(!value.contains("refresh_token"));
    }

    #[test]
    fn apply_request_roundtrip_preserves_all_fields() {
        let request = ApplySyncRequest {
            expected_remote_commit: "head".into(),
            plan_fingerprint: "fingerprint".into(),
            selected_action_ids: vec!["local_update:agents:demo".into()],
            decisions: std::collections::HashMap::from([(
                "agents:demo".into(),
                vault::SyncDecision::KeepLocal,
            )]),
            delete_guard_ack: true,
        };
        let encoded = serde_json::to_vec(&request).unwrap();
        let decoded: ApplySyncRequest = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn app_state_serialization_contains_no_credential_fields() {
        let config = AppConfig::default_config();
        let state = AppState {
            configured: false,
            config,
            github_authorized: false,
            github_user: None,
            github_app_slug: None,
            credential_status: CredentialStatus::Disconnected,
            installation_id: None,
            repository_id: None,
            remote_owner: None,
            remote_repo: None,
            remote_branch: None,
            vault_status: None,
            device_name: "device".into(),
            remote_commit: None,
            pending_recovery: None,
        };
        let value = serde_json::to_string(&state).unwrap();
        assert!(!value.contains("access_token"));
        assert!(!value.contains("refresh_token"));
    }

    #[test]
    fn expected_previous_branch_mismatch_is_rejected() {
        let current = RemoteConfig {
            installation_id: 1,
            repository_id: 2,
            owner: "owner".into(),
            repo: "repo".into(),
            branch: "main".into(),
        };
        let expected = RemoteBindingKey {
            installation_id: 1,
            repository_id: 2,
            branch: "other".into(),
        };
        assert!(validate_expected_binding(Some(&current), Some(&expected)).is_err());
    }

    #[tokio::test]
    async fn operation_gate_serializes_writers_without_sleep() {
        let gate = Arc::new(SyncOperationGate::default());
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let first_started = Arc::new(Notify::new());
        let second_attempted = Arc::new(Notify::new());
        let release_first = Arc::new(Notify::new());

        let first = {
            let gate = gate.clone();
            let active = active.clone();
            let max_active = max_active.clone();
            let first_started = first_started.clone();
            let release_first = release_first.clone();
            tokio::spawn(async move {
                let _guard = gate.inner.write().await;
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(now, Ordering::SeqCst);
                first_started.notify_one();
                release_first.notified().await;
                active.fetch_sub(1, Ordering::SeqCst);
            })
        };
        first_started.notified().await;

        let second = {
            let gate = gate.clone();
            let active = active.clone();
            let max_active = max_active.clone();
            let second_attempted = second_attempted.clone();
            tokio::spawn(async move {
                second_attempted.notify_one();
                let _guard = gate.inner.write().await;
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(now, Ordering::SeqCst);
                active.fetch_sub(1, Ordering::SeqCst);
            })
        };
        second_attempted.notified().await;
        release_first.notify_one();
        first.await.unwrap();
        second.await.unwrap();
        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

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

    // ---- Task 6: 远端恢复裁决 ----

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
            branch: "main".into(),
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

#[cfg(target_os = "macos")]
fn open_path_platform(path: &str) -> Result<()> {
    Command::new("open")
        .arg(path)
        .spawn()
        .map_err(|error| AppError::Io(error.to_string()))?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_path_platform(path: &str) -> Result<()> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|error| AppError::Io(error.to_string()))?;
    Ok(())
}
