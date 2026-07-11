use crate::errors::{AppError, Result};

/// 校验单个 portable path 组件（如 skill 的 `folder_name`）。
/// 拒绝空、`.`、`..`、NUL、路径分隔符、Windows 保留字符/设备名以及末尾空格或 `.`。
/// 本任务只提供单段组件校验；完整路径与 NFC 折叠 collision key 在 Task 4 补全。
pub(crate) fn validate_component(name: &str) -> Result<()> {
    let invalid = |reason: &str| -> AppError {
        AppError::Vault(format!("invalid path component {name:?}: {reason}"))
    };

    if name.is_empty() || name == "." || name == ".." {
        return Err(invalid("empty or reserved"));
    }
    if name.contains('\0') || name.contains('/') || name.contains('\\') {
        return Err(invalid("contains NUL or path separator"));
    }
    // Windows 保留字符；`:` 同时排除盘符绝对路径（如 `C:`）。
    const RESERVED: &[char] = &['<', '>', ':', '"', '|', '?', '*'];
    if name.chars().any(|c| RESERVED.contains(&c)) {
        return Err(invalid("contains reserved character"));
    }
    // Windows 设备名（CON/PRN/AUX/NUL/COM1-9/LPT1-9），按扩展名前的 stem 判断。
    let upper = name.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or(upper.as_str());
    const DEVICES: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if DEVICES.contains(&stem) {
        return Err(invalid("reserved device name"));
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Err(invalid("trailing space or dot"));
    }
    Ok(())
}
