#![cfg_attr(
    not(test),
    deny(clippy::expect_used, clippy::panic, clippy::unwrap_used)
)]

use tauri::Manager;

mod commands;
mod config;
mod detect;
mod errors;
mod github_app_config;
mod github_auth;
mod github_credentials;
mod github_repository;
mod github_store;
mod ignore;
mod local_apply;
#[cfg(test)]
mod local_vault_store;
mod logging;
mod pack;
mod portable_path;
mod remote_store;
mod skill;
mod sync_engine;
mod sync_state;
mod vault_binding;
mod vault_manifest;

// Tauri 事件循环启动失败后应用无法继续运行，此处允许以明确错误退出。
#[allow(clippy::expect_used)]
pub fn run() {
    let runtime = match commands::AppRuntime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to initialize application state: {error}");
            return;
        }
    };
    tauri::Builder::default()
        .plugin(logging::plugin())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            match app.path().app_log_dir() {
                Ok(log_dir) => match logging::cleanup_old_logs(&log_dir) {
                    Ok(removed) => log::info!(
                        target: "skill-sync",
                        "log cleanup completed removed_files={removed}"
                    ),
                    Err(error) => log::warn!(
                        target: "skill-sync",
                        "log cleanup failed error={error}"
                    ),
                },
                Err(error) => log::warn!(
                    target: "skill-sync",
                    "log directory unavailable error={error}"
                ),
            }
            log::info!(target: "skill-sync", "application started");
            Ok(())
        })
        .manage(runtime)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::save_config,
            commands::scan_skills,
            commands::get_sync_plan,
            commands::apply_sync_plan,
            commands::resume_sync_recovery,
            commands::open_path,
            commands::start_github_device_flow,
            commands::poll_github_device_flow,
            commands::get_github_app_info,
            commands::list_github_installations,
            commands::list_installation_repositories,
            commands::discover_single_github_repository,
            commands::list_github_repository_branches,
            commands::check_github_vault,
            commands::initialize_github_vault,
            commands::bind_github_vault,
            commands::disconnect_github,
            commands::list_remote_skills,
            commands::upload_skills,
            commands::download_skills,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
