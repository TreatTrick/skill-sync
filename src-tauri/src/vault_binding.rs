use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

const CONFIG_FILE_NAME: &str = "config.yaml";
const STATE_FILE_NAME: &str = "sync_state.json";
const JOURNAL_FILE_NAME: &str = "bind-transaction.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BindingJournal {
    pub schema: u32,
    pub previous_config: Option<Vec<u8>>,
    pub previous_state: Option<Vec<u8>>,
    pub next_config: Vec<u8>,
    pub next_state: Option<Vec<u8>>,
    pub history_path: Option<String>,
    pub history_bytes: Option<Vec<u8>>,
}

pub(crate) struct VaultBindingStore;

impl VaultBindingStore {
    pub(crate) fn commit_bytes(
        config_dir: &Path,
        previous_config: Option<Vec<u8>>,
        previous_state: Option<Vec<u8>>,
        next_config: Vec<u8>,
        next_state: Option<Vec<u8>>,
        history: Option<(String, Vec<u8>)>,
    ) -> Result<()> {
        fs::create_dir_all(config_dir)?;
        let (history_path, history_bytes) = history
            .map(|(path, bytes)| (Some(path), Some(bytes)))
            .unwrap_or((None, None));
        let journal = BindingJournal {
            schema: 1,
            previous_config,
            previous_state,
            next_config: next_config.clone(),
            next_state: next_state.clone(),
            history_path,
            history_bytes,
        };
        write_journal(config_dir, &journal)?;

        durable_replace(&config_path(config_dir), &next_config)?;
        replace_optional(&state_path(config_dir), next_state.as_deref())?;
        complete_history(config_dir, &journal)?;
        clear_journal(config_dir)
    }

    pub(crate) fn recover_if_needed(config_dir: &Path) -> Result<()> {
        let journal = match load_journal(config_dir)? {
            Some(journal) => journal,
            None => return Ok(()),
        };
        if journal.schema != 1 {
            return Err(AppError::RecoveryPending(format!(
                "unsupported binding journal schema: {}",
                journal.schema
            )));
        }

        let current_config = read_optional(&config_path(config_dir))?;
        let current_state = read_optional(&state_path(config_dir))?;
        if current_config == Some(journal.next_config.clone())
            && current_state == journal.next_state
        {
            complete_history(config_dir, &journal)?;
            return clear_journal(config_dir);
        }

        replace_optional(&config_path(config_dir), journal.previous_config.as_deref())?;
        replace_optional(&state_path(config_dir), journal.previous_state.as_deref())?;
        clear_journal(config_dir)
    }
}

fn config_path(config_dir: &Path) -> PathBuf {
    config_dir.join(CONFIG_FILE_NAME)
}

fn state_path(config_dir: &Path) -> PathBuf {
    config_dir.join(STATE_FILE_NAME)
}

fn journal_path(config_dir: &Path) -> PathBuf {
    config_dir.join(JOURNAL_FILE_NAME)
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn write_journal(config_dir: &Path, journal: &BindingJournal) -> Result<()> {
    let bytes = serde_json::to_vec(journal)
        .map_err(|error| AppError::Vault(format!("binding journal encode failed: {error}")))?;
    durable_replace(&journal_path(config_dir), &bytes)
}

fn load_journal(config_dir: &Path) -> Result<Option<BindingJournal>> {
    let bytes = match read_optional(&journal_path(config_dir))? {
        Some(bytes) => bytes,
        None => return Ok(None),
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|error| {
        AppError::RecoveryPending(format!("binding journal decode failed: {error}"))
    })
}

fn complete_history(config_dir: &Path, journal: &BindingJournal) -> Result<()> {
    let (relative_path, bytes) = match (&journal.history_path, &journal.history_bytes) {
        (Some(path), Some(bytes)) => (path, bytes),
        (None, None) => return Ok(()),
        _ => {
            return Err(AppError::RecoveryPending(
                "binding journal history is incomplete".into(),
            ))
        }
    };
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::RecoveryPending(
            "binding journal history path is unsafe".into(),
        ));
    }
    let target = config_dir.join(relative);
    if read_optional(&target)?.as_deref() != Some(bytes.as_slice()) {
        durable_replace(&target, bytes)?;
    }
    Ok(())
}

fn replace_optional(target: &Path, bytes: Option<&[u8]>) -> Result<()> {
    match bytes {
        Some(bytes) => durable_replace(target, bytes),
        None => match fs::remove_file(target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        },
    }
}

fn clear_journal(config_dir: &Path) -> Result<()> {
    match fs::remove_file(journal_path(config_dir)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn durable_replace(target: &Path, bytes: &[u8]) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::Vault("binding target has no parent".into()))?;
    fs::create_dir_all(parent)?;
    let temp = target.with_extension("tmp");
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, target)?;
    if let Ok(dir) = fs::File::open(parent) {
        drop(dir.sync_all());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_commit_writes_new_config_and_state_and_archives_previous_state() {
        let dir = tempfile::tempdir().unwrap();
        let previous_config = b"version: 2\nremote: null\n".to_vec();
        let previous_state = b"old-state".to_vec();
        let next_config = b"version: 2\nremote:\n  repository_id: 2\n".to_vec();
        let next_state = b"new-state".to_vec();

        VaultBindingStore::commit_bytes(
            dir.path(),
            Some(previous_config),
            Some(previous_state.clone()),
            next_config.clone(),
            Some(next_state.clone()),
            Some(("history/old-state.json".into(), previous_state)),
        )
        .unwrap();

        assert_eq!(
            std::fs::read(dir.path().join("config.yaml")).unwrap(),
            next_config
        );
        assert_eq!(
            std::fs::read(dir.path().join("sync_state.json")).unwrap(),
            next_state
        );
        assert_eq!(
            std::fs::read(dir.path().join("history/old-state.json")).unwrap(),
            b"old-state"
        );
        assert!(!journal_path(dir.path()).exists());
    }

    #[test]
    fn recovery_rolls_back_mixed_config_and_state() {
        let dir = tempfile::tempdir().unwrap();
        let previous_config = b"old-config".to_vec();
        let previous_state = Some(b"old-state".to_vec());
        let next_config = b"new-config".to_vec();
        let next_state = Some(b"new-state".to_vec());
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join("config.yaml"), &next_config).unwrap();
        std::fs::write(
            dir.path().join("sync_state.json"),
            previous_state.as_ref().unwrap(),
        )
        .unwrap();
        write_journal(
            dir.path(),
            &BindingJournal {
                schema: 1,
                previous_config: Some(previous_config.clone()),
                previous_state: previous_state.clone(),
                next_config,
                next_state,
                history_path: None,
                history_bytes: None,
            },
        )
        .unwrap();

        VaultBindingStore::recover_if_needed(dir.path()).unwrap();

        assert_eq!(
            std::fs::read(dir.path().join("config.yaml")).unwrap(),
            previous_config
        );
        assert_eq!(
            std::fs::read(dir.path().join("sync_state.json")).unwrap(),
            b"old-state"
        );
        assert!(!journal_path(dir.path()).exists());
    }
}
