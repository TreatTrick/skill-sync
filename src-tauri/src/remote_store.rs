// 远端 vault 适配器 trait 与 DTO。Task 8/10 接入前为 dead code，整模块 allow。
#![allow(dead_code)]

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::errors::{AppError, Result};
use crate::vault_manifest::VaultManifest;

/// 远端 vault 的只读 snapshot：解析后的 manifest + 当前 commit SHA + 分支。
#[allow(dead_code)] // branch 由 Task 10 github_store 填充真实分支
#[derive(Debug, Clone)]
pub(crate) struct RemoteSnapshot {
    pub manifest: VaultManifest,
    pub commit_sha: String,
    pub branch: String,
}

/// 待写入远端的 blob：path 必须由 `expected_hash` 推导，bytes 的 SHA-256 必须等于 `expected_hash`。
#[derive(Debug, Clone)]
pub(crate) struct BlobWrite {
    pub path: String,
    pub bytes: Vec<u8>,
    pub expected_hash: String,
}

/// 一次远端提交：base commit、新 blob、next manifest 与 commit message。
#[allow(dead_code)] // commit_message 由 Task 10 github_store 用作真实提交信息
#[derive(Debug, Clone)]
pub(crate) struct RemoteChanges {
    pub base_commit_sha: String,
    pub blobs: Vec<BlobWrite>,
    pub next_manifest: VaultManifest,
    pub commit_message: String,
}

/// 提交后的 snapshot 版本标识。
#[derive(Debug, Clone)]
pub(crate) struct RemoteCommit {
    pub commit_sha: String,
}

/// 远端 vault 适配器 trait。所有方法直接 `.await`，实现方不得创建 runtime 或同步阻塞。
#[async_trait]
pub(crate) trait RemoteStore: Send + Sync {
    async fn fetch_manifest(&self) -> Result<RemoteSnapshot>;
    async fn fetch_blob(&self, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>>;
    async fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit>;
}

/// 校验 `sha256:[0-9a-f]{64}` 格式，返回 hex 部分。
pub(crate) fn validate_hash_format(hash: &str) -> Result<&str> {
    let hex = hash
        .strip_prefix("sha256:")
        .ok_or_else(|| AppError::Vault(format!("hash missing sha256: prefix: {hash:?}")))?;
    if hex.len() != 64 || !hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(AppError::Vault(format!(
            "hash must be sha256:64 lowercase hex: {hash:?}"
        )));
    }
    Ok(hex)
}

/// 由 hash 推导的 blob path：`blobs/sha256/<hex>.skill.zip`。
pub(crate) fn blob_path_for_hash(hash: &str) -> Result<String> {
    let hex = validate_hash_format(hash)?;
    Ok(format!("blobs/sha256/{hex}.skill.zip"))
}

/// 校验单个 `BlobWrite`：hash 格式、path 严格等于推导值、bytes SHA-256 等于 expected_hash。
/// 任一失败返回 `Vault`，调用方据此保证不发生部分提交。
pub(crate) fn validate_blob_write(blob: &BlobWrite) -> Result<()> {
    let expected_path = blob_path_for_hash(&blob.expected_hash)?;
    if blob.path != expected_path {
        return Err(AppError::Vault(format!(
            "blob path {:?} != expected {expected_path:?}",
            blob.path
        )));
    }
    let computed = format!("sha256:{}", hex::encode(Sha256::digest(&blob.bytes)));
    if computed != blob.expected_hash {
        return Err(AppError::Vault(format!(
            "blob bytes hash mismatch for {:?}",
            blob.path
        )));
    }
    Ok(())
}
