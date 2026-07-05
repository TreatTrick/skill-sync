mod backup;
mod commands;
mod config;
mod detect;
mod errors;
mod git_store;
mod manifest;
mod skill;
mod sync_engine;

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
