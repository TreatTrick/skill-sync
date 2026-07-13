// GitHub 命令：设备流认证、仓库发现与 vault 初始化/绑定、断开，以及 list/upload/download。

use super::*;
use crate::github::auth::{DeviceFlowStart, InternalPollResult};

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
