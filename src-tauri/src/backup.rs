use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupEntry {
    pub id: String,
    pub skill_id: String,
    pub original_path: String,
    pub created_at: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize)]
struct BackupMeta {
    skill_id: String,
    original_path: String,
    created_at: String,
}

const META_FILE: &str = "meta.json";

/// Backups live under the app data dir: `<data>/skill-sync/backups/<id>/`.
pub(crate) fn backup_root() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .map(|d| d.join("skill-sync").join("backups"))
        .ok_or_else(|| AppError::Other("cannot determine data dir".into()))?;
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(crate) fn create_backup(skill_path: &Path, skill_id: &str) -> Result<BackupEntry> {
    let root = backup_root()?;
    let now = Utc::now();
    let id = now.timestamp_nanos_opt().unwrap_or(0).to_string();
    let target = root.join(&id);
    fs::create_dir_all(&target)?;
    if skill_path.is_dir() {
        copy_tree(skill_path, &target)?;
    }
    let meta = BackupMeta {
        skill_id: skill_id.to_string(),
        original_path: skill_path.to_string_lossy().to_string(),
        created_at: now.to_rfc3339(),
    };
    fs::write(target.join(META_FILE), serde_json::to_string_pretty(&meta)?)?;
    let size = dir_size(&target);
    Ok(BackupEntry {
        id,
        skill_id: meta.skill_id,
        original_path: meta.original_path,
        created_at: meta.created_at,
        size,
    })
}

pub(crate) fn list_backups() -> Result<Vec<BackupEntry>> {
    let root = backup_root()?;
    let mut entries = Vec::new();
    if !root.exists() {
        return Ok(entries);
    }
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta: BackupMeta = match fs::read_to_string(path.join(META_FILE)) {
            Ok(t) => serde_json::from_str(&t).unwrap_or(BackupMeta {
                skill_id: String::new(),
                original_path: String::new(),
                created_at: String::new(),
            }),
            Err(_) => continue,
        };
        entries.push(BackupEntry {
            id: entry.file_name().to_string_lossy().to_string(),
            skill_id: meta.skill_id,
            original_path: meta.original_path,
            created_at: meta.created_at,
            size: dir_size(&path),
        });
    }
    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(entries)
}

pub(crate) fn restore_backup(backup_id: &str, target: &Path) -> Result<()> {
    let root = backup_root()?;
    let src = root.join(backup_id);
    if !src.exists() {
        return Err(AppError::Other(format!("backup not found: {backup_id}")));
    }
    if target.exists() {
        fs::remove_dir_all(target)?;
    }
    fs::create_dir_all(target)?;
    copy_tree(&src, target)?;
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else if from.file_name().map(|f| f != META_FILE).unwrap_or(true) {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn create_list_restore_roundtrip() {
        let skill = tempdir().unwrap();
        fs::write(skill.path().join("SKILL.md"), "---\nname: x\n---\n").unwrap();
        fs::write(skill.path().join("note.txt"), "hello").unwrap();

        let entry = create_backup(skill.path(), "codex/x").unwrap();
        assert_eq!(entry.skill_id, "codex/x");
        assert!(entry.size > 0);

        let list = list_backups().unwrap();
        assert!(list.iter().any(|b| b.id == entry.id));

        // restore into a fresh dir
        let target = tempdir().unwrap();
        let dest = target.path().join("x");
        restore_backup(&entry.id, &dest).unwrap();
        assert!(dest.join("SKILL.md").exists());
        assert!(dest.join("note.txt").exists());
        // meta.json must not leak into the restored skill
        assert!(!dest.join(META_FILE).exists());
    }

    #[test]
    fn restore_missing_backup_errors() {
        let target = tempdir().unwrap();
        assert!(restore_backup("does-not-exist-123", target.path()).is_err());
    }
}
