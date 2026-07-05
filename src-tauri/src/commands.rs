use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

use crate::backup::{list_backups as list_backups_impl, restore_backup as restore_backup_impl, BackupEntry};
use crate::config::{expand_path, AppConfig};
use crate::detect::scan_local_skills;
use crate::errors::{AppError, Result};
use crate::git_store::GitStore;
use crate::skill::Skill;
use crate::sync_engine::{apply_plan, build_plan, ApplyResult, SyncPlan};

#[derive(Serialize)]
pub struct AppState {
    pub configured: bool,
    pub config: AppConfig,
    pub git_available: bool,
    pub git_version: String,
}

#[derive(Serialize)]
pub struct GitCheck {
    pub available: bool,
    pub version: String,
}

#[derive(Serialize)]
pub struct RemoteCheck {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct ScanResult {
    pub skills: Vec<Skill>,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub fn get_app_state() -> Result<AppState> {
    let config = AppConfig::load()?;
    let git = GitStore::check_git();
    Ok(AppState {
        configured: config.is_configured(),
        config,
        git_available: git.is_ok(),
        git_version: git.unwrap_or_default(),
    })
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<()> {
    config.save()
}

#[tauri::command]
pub fn check_git() -> Result<GitCheck> {
    match GitStore::check_git() {
        Ok(v) => Ok(GitCheck {
            available: true,
            version: v,
        }),
        Err(_) => Ok(GitCheck {
            available: false,
            version: String::new(),
        }),
    }
}

#[tauri::command]
pub fn check_remote(remote: String) -> Result<RemoteCheck> {
    match GitStore::check_remote(&remote) {
        Ok(()) => Ok(RemoteCheck {
            ok: true,
            message: String::new(),
        }),
        Err(e) => Ok(RemoteCheck {
            ok: false,
            message: e.to_string(),
        }),
    }
}

/// Initialize (or open) the sync repo, attach the remote, and persist the
/// repository settings. Called from onboarding. An empty `local_path` means
/// "manage automatically" — the repo lives under the config dir.
#[tauri::command]
pub fn prepare_repo(local_path: String, remote: String, branch: String) -> Result<()> {
    let mut config = AppConfig::load()?;
    let repo_path = if local_path.trim().is_empty() {
        AppConfig::default_repo_path()?
    } else {
        expand_path(&local_path)?
    };
    let git = GitStore::new(&repo_path);
    git.init()?;
    if !remote.trim().is_empty() {
        git.set_remote(&remote)?;
    }
    config.repository.local_path = local_path;
    config.repository.remote = remote;
    config.repository.branch = if branch.trim().is_empty() {
        "main".into()
    } else {
        branch
    };
    config.save()?;
    Ok(())
}

#[tauri::command]
pub fn scan_skills() -> Result<ScanResult> {
    let config = AppConfig::load()?;
    let (skills, warnings) = scan_local_skills(&config)?;
    Ok(ScanResult { skills, warnings })
}

#[tauri::command]
pub fn get_sync_plan() -> Result<SyncPlan> {
    let config = AppConfig::load()?;
    if !config.is_configured() {
        return Err(AppError::NotConfigured("repository not configured".into()));
    }
    let (skills, _w) = scan_local_skills(&config)?;
    build_plan(&config, &skills)
}

/// Recompute the plan fresh (pull first), then apply it. `decisions` maps a
/// conflict skill id to `"local"` or `"remote"`; missing entries skip.
#[tauri::command]
pub fn apply_sync_plan(decisions: HashMap<String, String>) -> Result<ApplyResult> {
    let config = AppConfig::load()?;
    if !config.is_configured() {
        return Err(AppError::NotConfigured("repository not configured".into()));
    }
    let (skills, _w) = scan_local_skills(&config)?;
    let plan = build_plan(&config, &skills)?;
    apply_plan(&config, &plan, &decisions)
}

#[tauri::command]
pub fn list_backups() -> Result<Vec<BackupEntry>> {
    list_backups_impl()
}

#[tauri::command]
pub fn restore_backup(backup_id: String, target_path: String) -> Result<()> {
    restore_backup_impl(&backup_id, &PathBuf::from(&target_path))
}

#[tauri::command]
pub fn open_path(path: String) -> Result<()> {
    open_path_platform(&path)
}

#[cfg(target_os = "windows")]
fn open_path_platform(p: &str) -> Result<()> {
    Command::new("explorer")
        .arg(p)
        .spawn()
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_path_platform(p: &str) -> Result<()> {
    Command::new("open")
        .arg(p)
        .spawn()
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_path_platform(p: &str) -> Result<()> {
    Command::new("xdg-open")
        .arg(p)
        .spawn()
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}
