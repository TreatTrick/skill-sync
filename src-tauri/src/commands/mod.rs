#![allow(clippy::let_underscore_must_use)]

use std::process::Command;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;
use uuid::Uuid;

use crate::config::{AppConfig, RemoteConfig};
use crate::detect::ScanResult;
use crate::errors::{AppError, Result};
use crate::github::app_config::GithubAppPublicConfig;
use crate::github::auth::GithubAuthClient;
use crate::github::credentials::{
    CredentialStore, GithubAuthenticatedClient, GithubCredentialManager, KeyringCredentialStore,
};
use crate::github::repository::{
    GithubRepositoryDiscovery, GithubRepositoryService, GithubVaultCheck, GithubVaultStatus,
    InitializeGithubVaultRequest,
};
use crate::github::store::GitHubVaultStore;
use crate::local_apply::{
    backup_journal, clear_journal, load_pending, recover_pending, ApplyJournal,
};
use crate::logging;
use crate::remote_store::RemoteStore;
use crate::sync_engine::model::{ApplySyncResponse, RecoveryInfo, RecoveryPhase};
use crate::sync_state::{RemoteIdentity, SyncState};
use crate::vault_binding::VaultBindingStore;

pub(crate) mod github;
pub(crate) mod sync;
pub(crate) mod system;

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
        crate::sync_engine::apply::upload_skills(
            &skill_ids,
            &config,
            &mut sync_state,
            &store,
            &home,
            &config_dir,
        )
        .await
    } else {
        crate::sync_engine::apply::download_skills(
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
        GitHubVaultStore::new(runtime.client.clone(), context),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_engine::model::{
        ApplyResult, ApplySyncRequest, CommitSummary, PlanChangeReason, SyncDecision, SyncPlan,
    };
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
            commit_summary: CommitSummary {
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
            reason: PlanChangeReason::RemoteChanged,
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
                SyncDecision::KeepLocal,
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
}
