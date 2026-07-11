#![cfg_attr(
    not(test),
    deny(clippy::expect_used, clippy::panic, clippy::unwrap_used)
)]

mod backup;
mod commands;
mod config;
mod detect;
mod errors;
mod git_store;
mod github_app_config;
mod github_auth;
mod github_credentials;
mod github_repository;
mod github_store;
mod ignore;
mod local_apply;
mod local_vault_store;
mod manifest;
mod pack;
mod portable_path;
mod remote_store;
mod skill;
mod sync_engine;
mod sync_state;
mod vault_manifest;

// Tauri 事件循环启动失败后应用无法继续运行，此处允许以明确错误退出。
#[allow(clippy::expect_used)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::save_config,
            commands::check_git,
            commands::check_remote,
            commands::prepare_repo,
            commands::scan_skills,
            commands::get_sync_plan,
            commands::apply_sync_plan,
            commands::list_backups,
            commands::restore_backup,
            commands::open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
