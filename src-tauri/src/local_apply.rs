// 本地 apply 事务：同盘 staging/replace/trash、durable state 写入、versioned journal 与恢复。
// Task 9 核心实现：journal 持久化与幂等 resume 简化（主要覆盖 StateSaving 恢复）。
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
