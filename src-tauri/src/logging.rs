use std::path::Path;
use std::time::{Duration, SystemTime};

use log::LevelFilter;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

pub(crate) const LOG_FILE_PREFIX: &str = "skill-sync";
const RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub(crate) fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    let file_name = format!(
        "{LOG_FILE_PREFIX}_{}",
        chrono::Local::now().format("%Y-%m-%d")
    );

    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some(file_name),
            }),
        ])
        .rotation_strategy(RotationStrategy::KeepAll)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .max_file_size(10 * 1024 * 1024)
        .level(level)
        .level_for("keyring", LevelFilter::Warn)
        .build()
}

pub(crate) fn cleanup_old_logs(log_dir: &Path) -> std::io::Result<usize> {
    cleanup_old_logs_at(log_dir, SystemTime::now())
}

fn cleanup_old_logs_at(log_dir: &Path, now: SystemTime) -> std::io::Result<usize> {
    if !log_dir.exists() {
        return Ok(0);
    }

    let mut removed = 0;
    for entry in std::fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        let is_application_log = metadata.is_file()
            && path.extension().is_some_and(|extension| extension == "log")
            && path
                .file_stem()
                .is_some_and(|stem| stem.to_string_lossy().starts_with(LOG_FILE_PREFIX));
        if is_application_log && is_log_expired(metadata.modified()?, now) {
            std::fs::remove_file(path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn is_log_expired(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .map(|age| age > RETENTION)
        .unwrap_or(false)
}

pub(crate) fn sanitize_for_log(message: &str) -> String {
    crate::errors::redact_sensitive(message)
}

pub(crate) fn log_app_error(operation: &str, error: &crate::errors::AppError) {
    log::error!(
        target: "skill-sync",
        "operation={operation} kind={} error={}",
        error.kind(),
        sanitize_for_log(&error.to_string())
    );
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use tempfile::tempdir;

    use super::{cleanup_old_logs_at, is_log_expired, sanitize_for_log};

    #[test]
    fn log_expiration_keeps_files_younger_than_seven_days() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);

        assert!(is_log_expired(
            now - Duration::from_secs(7 * 24 * 60 * 60 + 1),
            now
        ));
        assert!(!is_log_expired(
            now - Duration::from_secs(7 * 24 * 60 * 60),
            now
        ));
        assert!(!is_log_expired(now - Duration::from_secs(60), now));
    }

    #[test]
    fn log_sanitization_removes_credentials_and_device_codes() {
        let message = sanitize_for_log(
            r#"access_token=access-value refresh_token=refresh-value Authorization: Bearer bearer-value device_code=device-value user_code=user-value client_secret=secret-value private_key=key-value"#,
        );

        for secret in [
            "access-value",
            "refresh-value",
            "bearer-value",
            "device-value",
            "user-value",
            "secret-value",
            "key-value",
        ] {
            assert!(!message.contains(secret), "secret leaked: {secret}");
        }
        assert!(message.contains("[REDACTED]"));
    }

    #[test]
    fn cleanup_removes_old_application_log_files() {
        let directory = tempdir().expect("temporary log directory");
        let old_log = directory.path().join("skill-sync_old.log");
        std::fs::write(&old_log, "old").expect("old log");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&old_log)
            .expect("open old log")
            .set_modified(now - Duration::from_secs(8 * 24 * 60 * 60))
            .expect("set old log time");

        assert_eq!(
            cleanup_old_logs_at(directory.path(), now).expect("cleanup logs"),
            1
        );
        assert!(!old_log.exists());
    }
}
