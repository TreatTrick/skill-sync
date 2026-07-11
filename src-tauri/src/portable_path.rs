use std::collections::HashSet;

use unicode_normalization::UnicodeNormalization;

use crate::errors::{AppError, Result};

/// 校验单个 portable path 组件（如 skill 的 `folder_name`）。
/// 拒绝空、`.`、`..`、NUL、路径分隔符、Windows 保留字符/设备名以及末尾空格或 `.`。
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

/// 规范化相对路径：Unicode NFC + 反斜杠转 `/`。
pub(crate) fn normalize(rel: &str) -> String {
    rel.replace('\\', "/").nfc().collect::<String>()
}

/// 校验完整相对路径：先规范化，拒绝绝对路径，逐段调用 `validate_component`。
pub(crate) fn validate_portable_path(rel: &str) -> Result<()> {
    let normalized = normalize(rel);
    if normalized.is_empty() {
        return Err(AppError::Vault("portable path is empty".into()));
    }
    if normalized.starts_with('/') {
        return Err(AppError::Vault(format!(
            "portable path must be relative: {rel:?}"
        )));
    }
    for component in normalized.split('/') {
        validate_component(component)?;
    }
    Ok(())
}

/// 完整路径的 NFC + lowercase 折叠 collision key。
pub(crate) fn collision_key(rel: &str) -> String {
    normalize(rel).to_lowercase()
}

/// 批量校验 portable path：每条路径合法，且无精确重复或 NFC+lowercase 折叠 collision。
pub(crate) fn validate_portable_paths(paths: &[impl AsRef<str>]) -> Result<()> {
    let mut seen: HashSet<String> = HashSet::new();
    for p in paths {
        let s = p.as_ref();
        validate_portable_path(s)?;
        let key = collision_key(s);
        if !seen.insert(key.clone()) {
            return Err(AppError::Vault(format!("portable path collision: {key:?}")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_path_validator_rejects_exact_case_and_unicode_collisions() {
        for paths in [
            vec!["same", "same"],
            vec!["Docs/a", "docs/A"],
            unicode_nfc_collision_paths(),
        ] {
            assert!(
                validate_portable_paths(&paths).is_err(),
                "expected collision: {paths:?}"
            );
        }
    }

    #[test]
    fn portable_path_validator_accepts_distinct_paths() {
        assert!(validate_portable_paths(&["SKILL.md", "scripts/run.sh", "docs/A"]).is_ok());
    }

    #[test]
    fn validate_portable_path_rejects_absolute_and_traversal() {
        assert!(validate_portable_path("/abs/path").is_err());
        assert!(validate_portable_path("a/../b").is_err());
        assert!(validate_portable_path("a/b/").is_err());
        assert!(validate_portable_path("CON/config").is_err());
    }

    /// 两个 NFC 等价但字节不同的路径：`café`（precomposed）与 `cafe\u{0301}`（decomposed）。
    fn unicode_nfc_collision_paths() -> Vec<&'static str> {
        vec!["café/x", "cafe\u{0301}/x"]
    }
}
