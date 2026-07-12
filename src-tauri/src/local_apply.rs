// 本地 apply 事务：同盘 staging/replace/trash、durable state 写入、versioned journal 与恢复。
// Task 9 核心实现：journal 持久化与幂等 resume 简化（主要覆盖 StateSaving 恢复）。
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};
use crate::skill::{fixed_skill_roots, SkillNamespace};

const JOURNAL_FILE: &str = "apply-transaction.json";

/// 固定 namespace root 路径（`~/.agents/skills` 等）。
pub(crate) fn namespace_root(home: &Path, namespace: SkillNamespace) -> Result<PathBuf> {
    fixed_skill_roots(home)
        .into_iter()
        .find(|r| r.namespace == namespace)
        .map(|r| r.path)
        .ok_or_else(|| AppError::Vault(format!("no fixed root for namespace {namespace:?}")))
}

/// `<root>/.skill-sync-staging/<task>/<folder>`
pub(crate) fn stage_dir(root: &Path, task_id: &str, folder: &str) -> PathBuf {
    root.join(".skill-sync-staging").join(task_id).join(folder)
}

/// `<root>/.skill-sync-rollback/<task>/<folder>`
pub(crate) fn rollback_dir(root: &Path, task_id: &str, folder: &str) -> PathBuf {
    root.join(".skill-sync-rollback").join(task_id).join(folder)
}

/// `<root>/.skill-sync-trash/<task>/<folder>`
pub(crate) fn trash_dir(root: &Path, task_id: &str, folder: &str) -> PathBuf {
    root.join(".skill-sync-trash").join(task_id).join(folder)
}

/// 同盘 rename 替换：target 存在则先 target -> rollback，再 stage -> target。
/// 第二次 rename 失败时立即 rollback -> target 恢复。
pub(crate) fn commit_staged(stage: &Path, target: &Path, rollback: &Path) -> Result<()> {
    if target.exists() {
        if rollback.exists() {
            drop(fs::remove_dir_all(rollback));
        }
        fs::rename(target, rollback)?;
        if let Err(e) = fs::rename(stage, target) {
            drop(fs::rename(rollback, target));
            return Err(AppError::Vault(format!("replace target failed: {e}")));
        }
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(stage, target)?;
    }
    Ok(())
}

/// target -> trash 同盘 rename。trash 必须与 target 同 namespace root/文件系统。
pub(crate) fn move_to_trash(target: &Path, trash: &Path) -> Result<()> {
    if let Some(parent) = trash.parent() {
        fs::create_dir_all(parent)?;
    }
    if trash.exists() {
        drop(fs::remove_dir_all(trash));
    }
    fs::rename(target, trash).map_err(|e| AppError::Vault(format!("trash move failed: {e}")))
}

/// 清空目录内所有内容（保留目录本身）。
pub(crate) fn clean_dir_contents(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            drop(if p.is_dir() {
                fs::remove_dir_all(&p)
            } else {
                fs::remove_file(&p)
            });
        }
    }
}

/// 持久化写入：同目录 temp + flush/fsync file + atomic rename + parent fsync。
pub(crate) fn durable_replace(target: &Path, bytes: &[u8]) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::Vault("target has no parent directory".into()))?;
    fs::create_dir_all(parent)?;
    let tmp = target.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, target)?;
    if let Ok(d) = fs::File::open(parent) {
        drop(d.sync_all());
    }
    Ok(())
}

// ---- versioned journal ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApplyJournal {
    pub schema: u32,
    pub task_id: String,
    pub phase: String,
    pub remote_candidate: Option<String>,
    pub next_state_bytes: Vec<u8>,
    pub next_state_hash: String,
    /// 远端 base commit（remote_committing/remote_outcome_unknown 证据）。
    /// 旧 schema-1 journal 缺省为 None。
    #[serde(default)]
    pub remote_base: Option<String>,
    /// 预期 next_manifest 的确定性 SHA-256。
    #[serde(default)]
    pub next_manifest_hash: Option<String>,
    /// 已完成本地副作用的 action（skill）ID。
    #[serde(default)]
    pub completed_action_ids: Vec<String>,
    /// 待远端发布的 action（skill）ID。
    #[serde(default)]
    pub pending_action_ids: Vec<String>,
}

pub(crate) fn journal_path(config_dir: &Path) -> PathBuf {
    config_dir.join(JOURNAL_FILE)
}

pub(crate) fn save_journal(config_dir: &Path, journal: &ApplyJournal) -> Result<()> {
    let bytes = serde_json::to_vec(journal)
        .map_err(|e| AppError::Vault(format!("journal serialize failed: {e}")))?;
    durable_replace(&journal_path(config_dir), &bytes)
}

pub(crate) fn load_journal(config_dir: &Path) -> Option<ApplyJournal> {
    let bytes = fs::read(journal_path(config_dir)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn load_pending(config_dir: &Path) -> Option<ApplyJournal> {
    load_journal(config_dir)
}

pub(crate) fn clear_journal(config_dir: &Path) -> Result<()> {
    let path = journal_path(config_dir);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// 把 active journal 原样复制到 `recovery-backups/<timestamp>-apply-transaction.json`。
/// 备份字节与 active journal 完全一致；`recovery-backups` 已是文件时 create_dir_all 失败，
/// active journal 不受影响（仅被读取）。成功返回备份路径。
pub(crate) fn backup_journal(config_dir: &Path) -> Result<PathBuf> {
    let active = journal_path(config_dir);
    let bytes = fs::read(&active)?;
    let backup_dir = config_dir.join("recovery-backups");
    fs::create_dir_all(&backup_dir)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| AppError::Vault(format!("system clock error: {e}")))?
        .as_nanos();
    let backup_path = backup_dir.join(format!("{timestamp}-apply-transaction.json"));
    durable_replace(&backup_path, &bytes)?;
    Ok(backup_path)
}

/// 恢复 pending journal。StateSaving phase：按 next_state 重写 sync_state 并清 journal。
/// 其他 phase（RemoteOutcomeUnknown/LocalReplaceFailed/TrashMoveFailed）：返回 journal 信息
/// 供上层报告恢复状态，不重复远端提交或本地动作。证据不一致时 fail closed。
pub(crate) fn recover_pending(config_dir: &Path) -> Result<Option<ApplyJournal>> {
    let journal = match load_journal(config_dir) {
        Some(j) => j,
        None => return Ok(None),
    };
    if journal.schema != 1 {
        return Err(AppError::RecoveryPending(format!(
            "unsupported journal schema: {}",
            journal.schema
        )));
    }
    if journal.phase == "state_saving" {
        let state: crate::sync_state::SyncState = serde_json::from_slice(&journal.next_state_bytes)
            .map_err(|e| AppError::Vault(format!("journal state decode failed: {e}")))?;
        state.save_to(config_dir)?;
        clear_journal(config_dir)?;
        return Ok(None);
    }
    Ok(Some(journal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_reads_schema_one_fixture_without_evidence_fields() {
        // 旧 schema-1 journal 不含新增证据字段；#[serde(default)] 保证可读。
        let fixture = serde_json::json!({
            "schema": 1,
            "task_id": "t-legacy",
            "phase": "state_saving",
            "remote_candidate": null,
            "next_state_bytes": Vec::<u8>::new(),
            "next_state_hash": "sha256:legacy",
        });
        let bytes = serde_json::to_vec(&fixture).unwrap();
        let journal: ApplyJournal = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(journal.task_id, "t-legacy");
        assert_eq!(journal.remote_base, None);
        assert_eq!(journal.next_manifest_hash, None);
        assert!(journal.completed_action_ids.is_empty());
        assert!(journal.pending_action_ids.is_empty());
    }

    #[test]
    fn journal_roundtrips_recovery_evidence_fields() {
        let journal = ApplyJournal {
            schema: 1,
            task_id: "t".into(),
            phase: "remote_outcome_unknown".into(),
            remote_candidate: Some("candidate".into()),
            next_state_bytes: vec![1, 2, 3],
            next_state_hash: "sha256:abc".into(),
            remote_base: Some("base".into()),
            next_manifest_hash: Some("sha256:manifest".into()),
            completed_action_ids: vec!["codex:done".into()],
            pending_action_ids: vec!["codex:pending".into()],
        };
        let bytes = serde_json::to_vec(&journal).unwrap();
        let back: ApplyJournal = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.remote_base.as_deref(), Some("base"));
        assert_eq!(back.next_manifest_hash.as_deref(), Some("sha256:manifest"));
        assert_eq!(back.completed_action_ids, vec!["codex:done".to_string()]);
        assert_eq!(back.pending_action_ids, vec!["codex:pending".to_string()]);
    }

    #[test]
    fn backup_journal_copies_exact_active_bytes_before_clear() {
        let dir = tempfile::tempdir().unwrap();
        // legacy journal（无新证据字段）
        let legacy = serde_json::json!({
            "schema": 1,
            "task_id": "t-legacy",
            "phase": "remote_committing",
            "remote_candidate": null,
            "next_state_bytes": Vec::<u8>::new(),
            "next_state_hash": "sha256:x",
        });
        let bytes = serde_json::to_vec(&legacy).unwrap();
        std::fs::write(journal_path(dir.path()), &bytes).unwrap();

        let backup_path = backup_journal(dir.path()).unwrap();
        let backup_bytes = std::fs::read(&backup_path).unwrap();
        // 备份字节与 active journal 完全一致
        assert_eq!(backup_bytes, bytes);
        // active journal 仍存在（仅被读取，未被改动）
        assert!(journal_path(dir.path()).exists());
    }

    #[test]
    fn backup_journal_fails_when_recovery_backups_is_a_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(journal_path(dir.path()), b"{}").unwrap();
        // recovery-backups 预创建为文件 -> create_dir_all 失败
        std::fs::write(dir.path().join("recovery-backups"), b"x").unwrap();

        assert!(backup_journal(dir.path()).is_err());
        // active journal 不受影响
        assert!(journal_path(dir.path()).exists());
    }
}
