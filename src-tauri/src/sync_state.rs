use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};
use crate::skill::SkillNamespace;

const STATE_FILE_NAME: &str = "sync_state.json";
#[cfg(test)]
const HISTORY_DIR_NAME: &str = "history";

/// 远端 vault 的稳定 identity：provider 常量 + installation/repository/branch + commit SHA。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RemoteIdentity {
    /// 固定 "github"。
    pub provider: String,
    pub installation_id: u64,
    pub repository_id: u64,
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub commit_sha: String,
}

/// 单个 skill 的本机同步状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SkillSyncState {
    pub base_hash: String,
    pub last_remote_hash: String,
    pub last_synced_at: String,
    pub namespace: SkillNamespace,
    pub relative_dir: String,
}

/// 本机 `sync_state.json` 状态：远端 identity + 每个 skill 的 base 协调信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncState {
    pub remote: RemoteIdentity,
    pub skills: BTreeMap<String, SkillSyncState>,
}

impl SyncState {
    pub(crate) fn empty(remote: RemoteIdentity) -> Self {
        Self {
            remote,
            skills: BTreeMap::new(),
        }
    }

    pub(crate) fn load_from(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join(STATE_FILE_NAME);
        let bytes = std::fs::read(&path)
            .map_err(|e| AppError::Vault(format!("sync state not found: {e}")))?;
        let state: SyncState = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::Vault(format!("invalid sync state: {e}")))?;
        Ok(state)
    }

    /// 普通 sync 的只读入口：installation/repository/branch 任一不一致即 blocked，
    /// 不移动或保存任何文件。
    pub(crate) fn load_and_validate(config_dir: &Path, remote: &RemoteIdentity) -> Result<Self> {
        let state = Self::load_from(config_dir)?;
        if state.remote.installation_id != remote.installation_id
            || state.remote.repository_id != remote.repository_id
            || state.remote.branch != remote.branch
        {
            return Err(AppError::Blocked(
                "sync state remote identity mismatch".into(),
            ));
        }
        Ok(state)
    }

    /// Onboarding 显式切换远端时调用：把旧 state 原子归档到 history，再写入空 base。
    #[cfg(test)]
    pub(crate) fn rebind_remote(config_dir: &Path, remote: RemoteIdentity) -> Result<Self> {
        let path = config_dir.join(STATE_FILE_NAME);
        if path.exists() {
            let old = Self::load_from(config_dir)?;
            let history_dir = config_dir.join(HISTORY_DIR_NAME);
            std::fs::create_dir_all(&history_dir)?;
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| AppError::Vault(format!("system clock error: {e}")))?
                .as_nanos();
            let rid = old.remote.repository_id;
            let history_path = history_dir.join(format!("{rid}-{timestamp}.json"));
            // 同盘 rename 即原子归档。
            std::fs::rename(&path, &history_path)?;
        }
        let state = Self::empty(remote);
        state.save_to(config_dir)?;
        Ok(state)
    }

    pub(crate) fn save_to(&self, config_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(config_dir)?;
        let target = config_dir.join(STATE_FILE_NAME);
        let bytes = serde_json::to_vec(self)
            .map_err(|e| AppError::Vault(format!("failed to serialize sync state: {e}")))?;
        durable_replace(&target, &bytes)
    }

    #[cfg(test)]
    pub(crate) fn remove_skill(&mut self, skill_id: &str) {
        self.skills.remove(skill_id);
    }
}

/// 持久化写入：同目录 temp + 文件 fsync + 原子替换 + 父目录 fsync（或平台等价 durable replace）。
fn durable_replace(target: &Path, bytes: &[u8]) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::Vault("target has no parent directory".into()))?;
    let temp = target.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    // rename 在 Unix 与 Windows 上都能原子替换既有目标
    //（Windows 经 MoveFileEx + MOVEFILE_REPLACE_EXISTING）。
    std::fs::rename(&temp, target)?;
    // 父目录 fsync（Windows 上对目录 sync_all 大致 no-op，best-effort 忽略错误）。
    if let Ok(dir) = std::fs::File::open(parent) {
        drop(dir.sync_all());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const HASH_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn remote_identity(installation_id: u64, repository_id: u64, branch: &str) -> RemoteIdentity {
        RemoteIdentity {
            provider: "github".into(),
            installation_id,
            repository_id,
            owner: "example".into(),
            repo: "agent-skills".into(),
            branch: branch.into(),
            commit_sha: "github-head-sha".into(),
        }
    }

    fn tracked_skill() -> SkillSyncState {
        SkillSyncState {
            base_hash: HASH_A.into(),
            last_remote_hash: HASH_A.into(),
            last_synced_at: "2026-07-07T13:00:00Z".into(),
            namespace: SkillNamespace::Codex,
            relative_dir: "ponytail".into(),
        }
    }

    fn save_tracked_state(dir: &Path, remote: RemoteIdentity) {
        let mut state = SyncState::empty(remote);
        state
            .skills
            .insert("codex:ponytail".into(), tracked_skill());
        state.save_to(dir).unwrap();
    }

    fn history_files(dir: &Path) -> Vec<PathBuf> {
        let history_dir = dir.join(HISTORY_DIR_NAME);
        std::fs::read_dir(&history_dir)
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default()
    }

    #[test]
    fn sync_state_roundtrip_preserves_base_hash() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = SyncState::empty(remote_identity(123456, 987654, "main"));
        state
            .skills
            .insert("codex:ponytail".into(), tracked_skill());
        state.save_to(dir.path()).unwrap();
        let back = SyncState::load_from(dir.path()).unwrap();
        assert_eq!(back.skills["codex:ponytail"].base_hash, HASH_A);
        assert_eq!(back.remote, state.remote);
        assert_eq!(
            back.skills["codex:ponytail"].namespace,
            SkillNamespace::Codex
        );
        assert_eq!(back.skills["codex:ponytail"].relative_dir, "ponytail");
    }

    #[test]
    fn ordinary_load_blocks_identity_mismatch_without_archiving() {
        let dir = tempfile::tempdir().unwrap();
        save_tracked_state(dir.path(), remote_identity(1, 10, "main"));
        for remote in [
            remote_identity(2, 10, "main"),
            remote_identity(1, 20, "main"),
            remote_identity(1, 10, "other"),
        ] {
            assert!(SyncState::load_and_validate(dir.path(), &remote).is_err());
            assert!(history_files(dir.path()).is_empty());
        }
    }

    #[test]
    fn explicit_rebind_archives_old_base_and_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        save_tracked_state(dir.path(), remote_identity(1, 10, "main"));
        let state = SyncState::rebind_remote(dir.path(), remote_identity(2, 20, "main")).unwrap();
        assert!(state.skills.is_empty());
        assert_eq!(state.remote.repository_id, 20);
        assert_eq!(history_files(dir.path()).len(), 1);
    }

    #[test]
    fn remove_skill_drops_entry() {
        let mut state = SyncState::empty(remote_identity(1, 10, "main"));
        state
            .skills
            .insert("codex:ponytail".into(), tracked_skill());
        assert!(state.skills.contains_key("codex:ponytail"));
        state.remove_skill("codex:ponytail");
        assert!(!state.skills.contains_key("codex:ponytail"));
    }
}
