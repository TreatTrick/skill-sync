#![cfg_attr(
    not(test),
    deny(clippy::expect_used, clippy::panic, clippy::unwrap_used)
)]

use tauri::Manager;

mod commands;
mod config;
mod detect;
mod errors;
mod github;
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
            commands::sync::get_app_state,
            commands::sync::save_config,
            commands::sync::scan_skills,
            commands::sync::get_sync_plan,
            commands::sync::apply_sync_plan,
            commands::sync::establish_baseline,
            commands::sync::resume_sync_recovery,
            commands::system::open_path,
            commands::github::start_github_device_flow,
            commands::github::poll_github_device_flow,
            commands::github::get_github_app_info,
            commands::github::list_github_installations,
            commands::github::list_installation_repositories,
            commands::github::discover_single_github_repository,
            commands::github::list_github_repository_branches,
            commands::github::check_github_vault,
            commands::github::initialize_github_vault,
            commands::github::bind_github_vault,
            commands::github::disconnect_github,
            commands::github::list_remote_skills,
            commands::github::upload_skills,
            commands::github::download_skills,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
