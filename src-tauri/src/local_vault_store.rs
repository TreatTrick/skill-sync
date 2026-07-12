// 本地 vault store：用本地文件系统模拟远端 GitHub vault。Task 8 接入前为 dead code，整模块 allow。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use crate::errors::{AppError, Result};
use crate::remote_store::{
    validate_blob_write, RemoteChanges, RemoteCommit, RemoteSnapshot, RemoteStore,
};
use crate::vault_manifest::VaultManifest;

const MANIFEST_FILE: &str = "manifest.json";
const COMMIT_MARKER: &str = ".local-vault-commit";

/// 空仓库的初始 commit 标识（40 个 0，模拟 git null SHA）。
const INITIAL_COMMIT_SHA: &str = "0000000000000000000000000000000000000000";

/// 本地 vault store：用本地文件系统模拟远端 GitHub vault，供测试/开发使用。
/// 同一路径的所有实例共享一个 process-wide `RwLock`，保证 manifest 与 commit marker
/// 的读写原子性。
pub(crate) struct LocalVaultStore {
    vault_dir: PathBuf,
    device_id: String,
    lock: Arc<RwLock<()>>,
}

impl LocalVaultStore {
    pub(crate) fn open(vault_dir: PathBuf, device_id: String) -> Result<Self> {
        fs::create_dir_all(&vault_dir)?;
        let lock = lock_for_path(&vault_dir);
        Ok(Self {
            vault_dir,
            device_id,
            lock,
        })
    }
}

#[async_trait]
impl RemoteStore for LocalVaultStore {
    async fn fetch_manifest(&self) -> Result<RemoteSnapshot> {
        let _guard = self.lock.read().await;
        let vault_dir = self.vault_dir.clone();
        let device_id = self.device_id.clone();
        let result: Result<RemoteSnapshot> = tauri::async_runtime::spawn_blocking(move || {
            read_manifest_blocking(&vault_dir, &device_id)
        })
        .await
        .map_err(|e| AppError::Vault(format!("blocking task failed: {e}")))?;
        result
    }

    async fn fetch_blob(&self, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>> {
        let _guard = self.lock.read().await;
        let vault_dir = self.vault_dir.clone();
        let blob_path = blob_path.to_string();
        let expected_hash = expected_hash.to_string();
        let result: Result<Vec<u8>> = tauri::async_runtime::spawn_blocking(move || {
            read_blob_blocking(&vault_dir, &blob_path, &expected_hash)
        })
        .await
        .map_err(|e| AppError::Vault(format!("blocking task failed: {e}")))?;
        result
    }

    async fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit> {
        let _guard = self.lock.write().await;
        let vault_dir = self.vault_dir.clone();
        let result: Result<RemoteCommit> =
            tauri::async_runtime::spawn_blocking(move || commit_blocking(&vault_dir, changes))
                .await
                .map_err(|e| AppError::Vault(format!("blocking task failed: {e}")))?;
        result
    }
}

/// 读取 manifest 与 commit marker，必须在同一 blocking closure（持读锁）内完成，
/// 保证二者来自同一次事务。
fn read_manifest_blocking(vault_dir: &Path, device_id: &str) -> Result<RemoteSnapshot> {
    let manifest_path = vault_dir.join(MANIFEST_FILE);
    let commit_path = vault_dir.join(COMMIT_MARKER);
    let manifest = if manifest_path.exists() {
        let bytes = fs::read(&manifest_path)?;
        VaultManifest::parse_validated(&bytes)?
    } else {
        VaultManifest::empty(device_id)
    };
    let commit_sha = read_commit_marker(&commit_path)?;
    Ok(RemoteSnapshot {
        manifest,
        commit_sha,
    })
}

fn read_blob_blocking(vault_dir: &Path, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>> {
    let path = vault_dir.join(blob_path);
    let bytes = fs::read(&path)?;
    let computed = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    if computed != expected_hash {
        return Err(AppError::Vault(format!(
            "blob bytes hash mismatch for {blob_path:?}"
        )));
    }
    Ok(bytes)
}

/// base commit 校验、blob/manifest 原子写入、commit marker 更新作为一个完整 blocking
/// transaction（持写锁）执行。任何写入前先校验全部 BlobWrite，失败无副作用。
fn commit_blocking(vault_dir: &Path, changes: RemoteChanges) -> Result<RemoteCommit> {
    for blob in &changes.blobs {
        validate_blob_write(blob)?;
    }
    let commit_path = vault_dir.join(COMMIT_MARKER);
    let current_sha = read_commit_marker(&commit_path)?;
    if current_sha != changes.base_commit_sha {
        return Err(AppError::RemoteChanged(format!(
            "base commit changed: expected {}, have {current_sha}",
            changes.base_commit_sha
        )));
    }
    for blob in &changes.blobs {
        let blob_path = vault_dir.join(&blob.path);
        if let Some(parent) = blob_path.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(&blob_path, &blob.bytes)?;
    }
    let manifest_bytes = serde_json::to_vec(&changes.next_manifest)
        .map_err(|e| AppError::Vault(format!("failed to serialize manifest: {e}")))?;
    atomic_write(&vault_dir.join(MANIFEST_FILE), &manifest_bytes)?;
    let new_sha = uuid::Uuid::new_v4().to_string();
    atomic_write(&commit_path, new_sha.as_bytes())?;
    Ok(RemoteCommit {
        commit_sha: new_sha,
    })
}

fn read_commit_marker(path: &Path) -> Result<String> {
    if path.exists() {
        let text = fs::read_to_string(path)?;
        Ok(text.trim().to_string())
    } else {
        Ok(INITIAL_COMMIT_SHA.into())
    }
}

/// 同盘 temp + rename 原子写入。
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| AppError::Vault(format!("invalid path: {path:?}")))?;
    let tmp = path.with_file_name(format!("{}.tmp", file_name.to_string_lossy()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// ---- process-wide path-keyed lock registry ----

type Registry = Mutex<HashMap<PathBuf, Weak<RwLock<()>>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 返回 vault path 对应的共享 `Arc<RwLock<()>>`；两个独立 `open` 指向同一路径拿到同一 Arc。
/// 清理已无强引用的 Weak entry，避免开发测试反复创建路径导致无界增长。
fn lock_for_path(path: &Path) -> Arc<RwLock<()>> {
    let guard = match registry().lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(), // 恢复 poison
    };
    let mut map = guard;
    if let Some(weak) = map.get(path) {
        if let Some(arc) = weak.upgrade() {
            return arc;
        }
    }
    map.retain(|_, w| w.strong_count() > 0);
    let arc = Arc::new(RwLock::new(()));
    map.insert(path.to_path_buf(), Arc::downgrade(&arc));
    arc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_store::BlobWrite;
    use crate::skill::SkillNamespace;
    use crate::vault_manifest::VaultSkill;
    use std::collections::HashSet;
    use tempfile::tempdir;

    /// 构造一个单 skill 的 RemoteChanges，blob 内容为 `bytes`，manifest 中该 skill 的
    /// hash/blob 与之严格匹配。
    fn build_changes(before: &RemoteSnapshot, device_id: &str, bytes: &[u8]) -> RemoteChanges {
        let hash = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
        let blob_path = format!("blobs/sha256/{}.skill.zip", &hash["sha256:".len()..]);
        let mut next_manifest = VaultManifest::empty(device_id);
        next_manifest.skills.insert(
            "codex:demo".into(),
            VaultSkill {
                id: "codex:demo".into(),
                name: "demo".into(),
                description: "d".into(),
                namespace: SkillNamespace::Codex,
                folder_name: "demo".into(),
                hash: hash.clone(),
                blob: blob_path.clone(),
                size: bytes.len() as u64,
                updated_at: "2026-07-07T13:00:00Z".into(),
                updated_by: device_id.into(),
            },
        );
        RemoteChanges {
            base_commit_sha: before.commit_sha.clone(),
            blobs: vec![BlobWrite {
                path: blob_path,
                bytes: bytes.to_vec(),
                expected_hash: hash,
            }],
            next_manifest,
            commit_message: "test".into(),
        }
    }

    fn manifest_hash_of(manifest: &VaultManifest) -> String {
        let bytes = serde_json::to_vec(manifest).unwrap();
        format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
    }

    /// 测试 fixture：持有 tempdir、store 与 observed 一致性对照表。
    struct LocalVaultFixture {
        path: PathBuf,
        store: LocalVaultStore,
        /// commit_sha -> 观察到的 manifest hash 集合；同一 sha 出现 >1 个 hash 即混合。
        observed: Arc<Mutex<HashMap<String, HashSet<String>>>>,
        _dir: tempfile::TempDir,
    }

    impl LocalVaultFixture {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let store = LocalVaultStore::open(dir.path().to_path_buf(), "device-a".into()).unwrap();
            Self {
                path: dir.path().to_path_buf(),
                store,
                observed: Arc::new(Mutex::new(HashMap::new())),
                _dir: dir,
            }
        }

        fn with_raw_blob(blob_path: &str, bytes: &[u8]) -> Self {
            let fixture = Self::new();
            let full = fixture.path.join(blob_path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, bytes).unwrap();
            fixture
        }

        fn one_skill_changes(&self, before: &RemoteSnapshot, bytes: &[u8]) -> RemoteChanges {
            self.changes_from(before, bytes)
        }

        fn changes_from(&self, before: &RemoteSnapshot, bytes: &[u8]) -> RemoteChanges {
            build_changes(before, "device-a", bytes)
        }

        async fn commit_from(&self, before: &RemoteSnapshot, bytes: &[u8]) -> Result<RemoteCommit> {
            let changes = self.changes_from(before, bytes);
            self.store.commit_changes(changes).await
        }

        /// `snapshot` 的 manifest 与 commit 是否来自同一次事务：同一 commit_sha 不应观察到
        /// 多个不同 manifest hash。
        fn manifest_matches_commit(&self, snapshot: &RemoteSnapshot) -> bool {
            let h = manifest_hash_of(&snapshot.manifest);
            let mut obs = self.observed.lock().unwrap();
            let entry = obs.entry(snapshot.commit_sha.clone()).or_default();
            entry.insert(h);
            entry.len() == 1
        }

        fn spawn_sequential_commits(self: Arc<Self>, n: usize) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async move {
                let mut current = self.store.fetch_manifest().await.unwrap();
                for i in 0..n {
                    let bytes = format!("zip-{i}").into_bytes();
                    let changes = self.changes_from(&current, &bytes);
                    let mhash = manifest_hash_of(&changes.next_manifest);
                    let commit = self.store.commit_changes(changes.clone()).await.unwrap();
                    {
                        let mut obs = self.observed.lock().unwrap();
                        obs.entry(commit.commit_sha.clone())
                            .or_default()
                            .insert(mhash);
                    }
                    current.commit_sha = commit.commit_sha;
                }
            })
        }

        fn spawn_snapshot_probe(
            self: Arc<Self>,
            n: usize,
        ) -> tokio::task::JoinHandle<Vec<RemoteSnapshot>> {
            tokio::spawn(async move {
                let mut out = Vec::new();
                for _ in 0..n {
                    out.push(self.store.fetch_manifest().await.unwrap());
                }
                out
            })
        }
    }

    #[tokio::test]
    async fn empty_local_vault_returns_empty_manifest() {
        let dir = tempdir().unwrap();
        let store = LocalVaultStore::open(dir.path().into(), "device-a".into()).unwrap();
        let snapshot = store.fetch_manifest().await.unwrap();
        assert!(snapshot.manifest.skills.is_empty());
        assert!(!snapshot.commit_sha.is_empty());
    }

    #[tokio::test]
    async fn commit_changes_writes_blob_and_manifest_once() {
        let fixture = LocalVaultFixture::new();
        let before = fixture.store.fetch_manifest().await.unwrap();
        let changes = fixture.one_skill_changes(&before, b"canonical zip bytes");
        let committed = fixture.store.commit_changes(changes.clone()).await.unwrap();
        let after = fixture.store.fetch_manifest().await.unwrap();
        assert_eq!(after.commit_sha, committed.commit_sha);
        assert_ne!(after.commit_sha, before.commit_sha);
        assert_eq!(after.manifest, changes.next_manifest);
        assert_eq!(
            fixture
                .store
                .fetch_blob(&changes.blobs[0].path, &changes.blobs[0].expected_hash)
                .await
                .unwrap(),
            b"canonical zip bytes"
        );
    }

    #[tokio::test]
    async fn changed_base_commit_returns_remote_changed() {
        let fixture = LocalVaultFixture::new();
        let stale = fixture.store.fetch_manifest().await.unwrap();
        fixture.commit_from(&stale, b"first").await.unwrap();
        let err = fixture.commit_from(&stale, b"stale").await.unwrap_err();
        assert!(matches!(err, AppError::RemoteChanged(_)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_fetch_never_observes_mixed_manifest_and_commit() {
        let fixture = Arc::new(LocalVaultFixture::new());
        let writer = fixture.clone().spawn_sequential_commits(100);
        let reader = fixture.clone().spawn_snapshot_probe(500);
        writer.await.unwrap();
        let observed = reader.await.unwrap();
        assert!(observed
            .iter()
            .all(|snapshot| fixture.manifest_matches_commit(snapshot)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn separate_store_instances_for_same_path_share_lock() {
        let fixture = LocalVaultFixture::new();
        let second = LocalVaultStore::open(fixture.path.clone(), "device-b".into()).unwrap();
        let base = fixture.store.fetch_manifest().await.unwrap();
        let (left, right) = tokio::join!(
            fixture
                .store
                .commit_changes(fixture.changes_from(&base, b"left")),
            second.commit_changes(fixture.changes_from(&base, b"right")),
        );
        assert_eq!(
            [left.is_ok(), right.is_ok()]
                .into_iter()
                .filter(|ok| *ok)
                .count(),
            1
        );
        assert_eq!(
            [left, right]
                .into_iter()
                .filter(|r| matches!(r, Err(AppError::RemoteChanged(_))))
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn fetch_blob_rejects_bytes_that_do_not_match_expected_hash() {
        let fixture = LocalVaultFixture::with_raw_blob("blobs/sha256/bad.skill.zip", b"corrupt");
        let err = fixture
            .store
            .fetch_blob("blobs/sha256/bad.skill.zip", "sha256:expected")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Vault(_)));
    }
}
