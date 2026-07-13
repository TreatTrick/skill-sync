// 系统命令：在平台文件管理器中打开路径。

use super::*;

#[tauri::command]
pub(crate) fn open_path(path: String) -> Result<()> {
    log_command_result("open_path", open_path_platform(&path))
}

#[cfg(target_os = "windows")]
fn open_path_platform(path: &str) -> Result<()> {
    Command::new("explorer")
        .arg(path)
        .spawn()
        .map_err(|error| AppError::Io(error.to_string()))?;
    Ok(())
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
