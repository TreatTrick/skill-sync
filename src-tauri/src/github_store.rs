// GitHub vault store：通过共享 authenticated client 读写已验证的 vault。

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::errors::{AppError, Result};
use crate::github_credentials::GithubAuthenticatedClient;
use crate::github_repository::GithubRepositoryContext;
use crate::remote_store::{
    blob_path_for_hash, validate_blob_write, RemoteChanges, RemoteCommit, RemoteSnapshot,
    RemoteStore,
};
use crate::vault_manifest::VaultManifest;

pub(crate) struct GitHubVaultStore {
    api: Arc<GithubAuthenticatedClient>,
    repository: GithubRepositoryContext,
}

impl GitHubVaultStore {
    pub(crate) fn new(
        api: Arc<GithubAuthenticatedClient>,
        repository: GithubRepositoryContext,
    ) -> Self {
        Self { api, repository }
    }
}

#[async_trait]
impl RemoteStore for GitHubVaultStore {
    async fn fetch_manifest(&self) -> Result<RemoteSnapshot> {
        let commit_sha = self.fetch_head().await?;
        if commit_sha != self.repository.head_sha {
            return Err(AppError::RemoteChanged(format!(
                "repository HEAD changed from {} to {}",
                self.repository.head_sha, commit_sha
            )));
        }
        let content_path = self.contents_path("manifest.json", &commit_sha);
        let response = self.api.get_path(&content_path).await?;
        let body = response_json(response, "fetch manifest").await?;
        let bytes = decode_content(&body, "manifest")?;
        let manifest = VaultManifest::parse_validated(&bytes)
            .map_err(|_| AppError::VaultStateChanged("remote manifest is invalid".into()))?;
        Ok(RemoteSnapshot {
            manifest,
            commit_sha,
        })
    }

    async fn fetch_blob(&self, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>> {
        let expected_path = blob_path_for_hash(expected_hash)?;
        if blob_path != expected_path {
            return Err(AppError::Vault(format!(
                "blob path {blob_path:?} != expected {expected_path:?}"
            )));
        }
        let response = self
            .api
            .get_path(&self.contents_path(blob_path, &self.repository.branch))
            .await?;
        let body = response_json(response, "fetch blob").await?;
        let bytes = decode_content(&body, "blob")?;
        let actual_hash = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        if actual_hash != expected_hash {
            return Err(AppError::Vault(format!(
                "blob hash mismatch for {blob_path:?}"
            )));
        }
        Ok(bytes)
    }

    async fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit> {
        let mut blob_paths = HashSet::new();
        for blob in &changes.blobs {
            validate_blob_write(blob)?;
            if blob.path == "manifest.json" || !blob_paths.insert(blob.path.as_str()) {
                return Err(AppError::Vault(format!(
                    "duplicate or reserved blob path: {}",
                    blob.path
                )));
            }
        }
        let manifest_bytes = changes
            .next_manifest
            .validated_bytes()
            .map_err(|_| AppError::Vault("next manifest is invalid".into()))?;

        let current_head = self.fetch_head().await?;
        if current_head != changes.base_commit_sha {
            return Err(AppError::RemoteChanged(format!(
                "base commit changed from {} to {}",
                changes.base_commit_sha, current_head
            )));
        }
        let base_commit = response_json(
            self.api
                .get_path(&self.git_path(&format!("commits/{}", changes.base_commit_sha)))
                .await?,
            "fetch base commit",
        )
        .await?;
        let base_tree = required_string(&base_commit, "/tree/sha", "base tree")?;

        let mut tree_entries = Vec::with_capacity(changes.blobs.len() + 1);
        for blob in &changes.blobs {
            let body = serde_json::json!({
                "content": base64::engine::general_purpose::STANDARD.encode(&blob.bytes),
                "encoding": "base64",
            });
            let created = response_json(
                self.api
                    .post_json_path(&self.git_path("blobs"), body)
                    .await?,
                "create blob",
            )
            .await?;
            let sha = required_string(&created, "/sha", "created blob sha")?;
            tree_entries.push(serde_json::json!({
                "path": blob.path,
                "mode": "100644",
                "type": "blob",
                "sha": sha,
            }));
        }

        let manifest_blob = response_json(
            self.api
                .post_json_path(
                    &self.git_path("blobs"),
                    serde_json::json!({
                        "content": base64::engine::general_purpose::STANDARD.encode(&manifest_bytes),
                        "encoding": "base64",
                    }),
                )
                .await?,
            "create manifest blob",
        )
        .await?;
        let manifest_sha = required_string(&manifest_blob, "/sha", "manifest blob sha")?;
        tree_entries.push(serde_json::json!({
            "path": "manifest.json",
            "mode": "100644",
            "type": "blob",
            "sha": manifest_sha,
        }));

        let tree = response_json(
            self.api
                .post_json_path(
                    &self.git_path("trees"),
                    serde_json::json!({
                        "base_tree": base_tree,
                        "tree": tree_entries,
                    }),
                )
                .await?,
            "create tree",
        )
        .await?;
        let tree_sha = required_string(&tree, "/sha", "created tree sha")?;
        let commit = response_json(
            self.api
                .post_json_path(
                    &self.git_path("commits"),
                    serde_json::json!({
                        "message": changes.commit_message,
                        "tree": tree_sha,
                        "parents": [changes.base_commit_sha],
                    }),
                )
                .await?,
            "create commit",
        )
        .await?;
        let candidate_sha = required_string(&commit, "/sha", "candidate commit sha")?;

        let latest_head = self.fetch_head().await?;
        if latest_head != changes.base_commit_sha {
            return Err(AppError::RemoteChanged(format!(
                "base commit changed before ref update: {latest_head}"
            )));
        }

        let update = self
            .api
            .patch_json_path(
                &self.ref_path(),
                serde_json::json!({
                    "sha": candidate_sha,
                    "force": false,
                }),
            )
            .await;
        match update {
            Ok(response) => {
                if matches!(response.status().as_u16(), 409 | 422) {
                    return Err(AppError::RemoteChanged(
                        "branch update rejected because remote changed".into(),
                    ));
                }
                if response.status().is_success() {
                    return Ok(RemoteCommit {
                        commit_sha: candidate_sha,
                    });
                }
                // 其他不成功 PATCH 响应：转换为原始错误后按不可变 HEAD 裁决。
                let original = response_error(response, "update branch ref").await;
                self.reconcile_update(&changes.base_commit_sha, &candidate_sha, original)
                    .await
            }
            Err(error) => {
                // 传输错误：按不可变 HEAD 裁决（candidate=成功，base=原始错误，其他=RemoteChanged）。
                self.reconcile_update(&changes.base_commit_sha, &candidate_sha, error)
                    .await
            }
        }
    }
}

impl GitHubVaultStore {
    fn repository_prefix(&self) -> String {
        format!("/repos/{}/{}", self.repository.owner, self.repository.repo)
    }

    fn ref_path(&self) -> String {
        format!(
            "{}/git/refs/heads/{}",
            self.repository_prefix(),
            self.repository.branch
        )
    }

    fn head_path(&self) -> String {
        format!(
            "{}/git/ref/heads/{}",
            self.repository_prefix(),
            self.repository.branch
        )
    }

    fn git_path(&self, resource: &str) -> String {
        format!("{}/git/{}", self.repository_prefix(), resource)
    }

    fn contents_path(&self, path: &str, reference: &str) -> String {
        // 静态 URL 解析不可失败；用 Url 仅为了正确编码 ref 查询参数。
        #[allow(clippy::expect_used)]
        let mut url = reqwest::Url::parse("https://api.github.com").expect("static URL is valid");
        url.set_path(&format!("{}/contents/{}", self.repository_prefix(), path));
        url.query_pairs_mut().append_pair("ref", reference);
        format!("{}?{}", url.path(), url.query().unwrap_or_default())
    }

    async fn fetch_head(&self) -> Result<String> {
        let response = self.api.get_path(&self.head_path()).await?;
        let body = response_json(response, "fetch branch ref").await?;
        required_string(&body, "/object/sha", "branch head sha")
    }

    /// PATCH 结果不明时按不可变 HEAD 裁决：candidate=已发布成功，base=未发布（返回原始错误），
    /// 其他 HEAD=远端被他人改动，HEAD 不可读=RemoteOutcomeUnknown。
    async fn reconcile_update(
        &self,
        base_commit_sha: &str,
        candidate_sha: &str,
        original_error: AppError,
    ) -> Result<RemoteCommit> {
        match self.fetch_head().await {
            Ok(head) if head == candidate_sha => Ok(RemoteCommit {
                commit_sha: candidate_sha.to_string(),
            }),
            Ok(head) if head == base_commit_sha => Err(original_error),
            Ok(head) => Err(AppError::RemoteChanged(format!(
                "branch update outcome is unknown; current head is {head}"
            ))),
            Err(_) => Err(AppError::RemoteOutcomeUnknown {
                base_commit_sha: base_commit_sha.to_string(),
                candidate_commit_sha: candidate_sha.to_string(),
            }),
        }
    }
}

async fn response_json(response: reqwest::Response, operation: &str) -> Result<serde_json::Value> {
    if !response.status().is_success() {
        return Err(response_error(response, operation).await);
    }
    response
        .json()
        .await
        .map_err(|e| AppError::Vault(format!("{operation} returned invalid JSON: {e}")))
}

async fn response_error(response: reqwest::Response, operation: &str) -> AppError {
    let status = response.status();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let rate_limit_remaining = response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let body = response.text().await.unwrap_or_default().to_lowercase();
    if matches!(status.as_u16(), 403 | 429)
        && (retry_after.is_some()
            || rate_limit_remaining.as_deref() == Some("0")
            || body.contains("rate limit")
            || body.contains("secondary rate limit"))
    {
        return AppError::RateLimited { retry_after };
    }
    match status.as_u16() {
        401 => AppError::ReauthorizationRequired(format!("{operation} returned unauthorized")),
        404 => AppError::VaultStateChanged(format!("{operation} resource not found")),
        _ => AppError::Vault(format!("{operation} failed with status {status}")),
    }
}

fn decode_content(body: &serde_json::Value, resource: &str) -> Result<Vec<u8>> {
    if body.get("encoding").and_then(|v| v.as_str()) != Some("base64") {
        return Err(AppError::Vault(format!(
            "{resource} response is not base64 encoded"
        )));
    }
    let content = body
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Vault(format!("{resource} response missing content")))?;
    let compact = content.replace(['\r', '\n'], "");
    base64::engine::general_purpose::STANDARD
        .decode(compact)
        .map_err(|e| AppError::Vault(format!("decode {resource} content: {e}")))
}

fn required_string(body: &serde_json::Value, pointer: &str, field: &str) -> Result<String> {
    body.pointer(pointer)
        .and_then(|value| value.as_str())
        .map(String::from)
        .ok_or_else(|| AppError::Vault(format!("response missing {field}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    use base64::Engine;
    use chrono::{Duration, Utc};
    use secrecy::SecretString;
    use sha2::Digest;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::config::RemoteConfig;
    use crate::github_app_config::GithubAppPublicConfig;
    use crate::github_auth::GithubAuthClient;
    use crate::github_credentials::{
        GithubCredential, GithubCredentialManager, InMemoryCredentialStore,
    };
    use crate::remote_store::{BlobWrite, RemoteChanges};
    use crate::vault_manifest::VaultManifest;

    fn remote() -> RemoteConfig {
        RemoteConfig {
            installation_id: 10,
            repository_id: 20,
            owner: "owner".into(),
            repo: "vault".into(),
            branch: "main".into(),
        }
    }

    async fn store(server: &MockServer) -> GitHubVaultStore {
        let credential_store = Arc::new(InMemoryCredentialStore::new());
        let auth = Arc::new(
            GithubAuthClient::new_with_urls(
                GithubAppPublicConfig::new("Iv1.test", "skill-sync").unwrap(),
                reqwest::Url::parse(&server.uri()).unwrap(),
                reqwest::Url::parse(&format!("{}/api/", server.uri())).unwrap(),
            )
            .unwrap(),
        );
        let manager = Arc::new(GithubCredentialManager::new(credential_store, auth));
        manager
            .save_initial(&GithubCredential {
                schema: 1,
                generation: Uuid::new_v4(),
                access_token: SecretString::new("token".into()),
                refresh_token: SecretString::new("refresh".into()),
                access_expires_at: Utc::now() + Duration::hours(1),
                refresh_expires_at: Utc::now() + Duration::days(1),
                github_login: "octocat".into(),
                app_client_id: "Iv1.test".into(),
            })
            .await
            .unwrap();
        let api = Arc::new(
            GithubAuthenticatedClient::new_with_api_base(
                manager,
                "Iv1.test".into(),
                reqwest::Url::parse(&format!("{}/", server.uri())).unwrap(),
            )
            .unwrap(),
        );
        let cfg = remote();
        GitHubVaultStore::new(
            api,
            GithubRepositoryContext {
                installation_id: cfg.installation_id,
                owner: cfg.owner,
                repo: cfg.repo,
                branch: cfg.branch,
                head_sha: "head".into(),
            },
        )
    }

    fn manifest_bytes() -> Vec<u8> {
        serde_json::to_vec(&VaultManifest::empty("device-test")).unwrap()
    }

    fn content_body(bytes: &[u8], sha: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "file",
            "encoding": "base64",
            "content": base64::engine::general_purpose::STANDARD.encode(bytes),
            "sha": sha,
        })
    }

    #[tokio::test]
    async fn fetch_manifest_reads_head_and_validated_manifest() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": { "sha": "head" }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/contents/manifest.json"))
            .and(query_param("ref", "head"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(content_body(&manifest_bytes(), "msha")),
            )
            .mount(&server)
            .await;

        let snapshot = store(&server).await.fetch_manifest().await.unwrap();
        assert_eq!(snapshot.commit_sha, "head");
        assert_eq!(snapshot.manifest.updated_by, "device-test");
    }

    #[tokio::test]
    async fn fetch_manifest_pins_contents_to_observed_head() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": { "sha": "head-a" }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/contents/manifest.json"))
            .and(query_param("ref", "head-a"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(content_body(&manifest_bytes(), "msha")),
            )
            .mount(&server)
            .await;
        let mut vault_store = store(&server).await;
        vault_store.repository.head_sha = "head-a".into();

        let snapshot = vault_store.fetch_manifest().await.unwrap();

        assert_eq!(snapshot.commit_sha, "head-a");
        assert_eq!(snapshot.manifest.updated_by, "device-test");
    }

    #[tokio::test]
    async fn fetch_manifest_rejects_changed_head_before_reading_manifest() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": { "sha": "new-head" }
            })))
            .mount(&server)
            .await;

        let result = store(&server).await.fetch_manifest().await;
        assert!(matches!(
            result,
            Err(crate::errors::AppError::RemoteChanged(_))
        ));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn fetch_manifest_maps_invalid_json_to_vault_state_changed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": { "sha": "head" }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/contents/manifest.json"))
            .and(query_param("ref", "head"))
            .respond_with(ResponseTemplate::new(200).set_body_json(content_body(b"{}", "msha")))
            .mount(&server)
            .await;

        let result = store(&server).await.fetch_manifest().await;
        assert!(matches!(
            result,
            Err(crate::errors::AppError::VaultStateChanged(_))
        ));
    }

    #[tokio::test]
    async fn fetch_blob_returns_bytes_after_hash_validation() {
        let server = MockServer::start().await;
        let bytes = b"blob-bytes";
        let hash = format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)));
        let blob_path = format!("blobs/sha256/{}.skill.zip", &hash[7..]);
        Mock::given(method("GET"))
            .and(path(format!("/repos/owner/vault/contents/{blob_path}")))
            .and(query_param("ref", "main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(content_body(bytes, "blob-sha")))
            .mount(&server)
            .await;

        let actual = store(&server)
            .await
            .fetch_blob(&blob_path, &hash)
            .await
            .unwrap();
        assert_eq!(actual, bytes);
    }

    #[tokio::test]
    async fn fetch_blob_preserves_special_characters_in_branch_ref() {
        let server = MockServer::start().await;
        let bytes = b"blob-bytes";
        let hash = format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)));
        let blob_path = format!("blobs/sha256/{}.skill.zip", &hash[7..]);
        let branch = "feature/hash#amp&percent%";
        Mock::given(method("GET"))
            .and(path(format!("/repos/owner/vault/contents/{blob_path}")))
            .and(query_param("ref", branch))
            .respond_with(ResponseTemplate::new(200).set_body_json(content_body(bytes, "blob-sha")))
            .mount(&server)
            .await;
        let mut vault_store = store(&server).await;
        vault_store.repository.branch = branch.into();

        let actual = vault_store.fetch_blob(&blob_path, &hash).await.unwrap();

        assert_eq!(actual, bytes);
    }

    #[tokio::test]
    async fn fetch_blob_rejects_bytes_with_wrong_hash() {
        let server = MockServer::start().await;
        let expected_hash =
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let blob_path = format!("blobs/sha256/{}.skill.zip", &expected_hash[7..]);
        Mock::given(method("GET"))
            .and(path(format!("/repos/owner/vault/contents/{blob_path}")))
            .and(query_param("ref", "main"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(content_body(b"not-the-expected-bytes", "blob-sha")),
            )
            .mount(&server)
            .await;

        let result = store(&server)
            .await
            .fetch_blob(&blob_path, expected_hash)
            .await;
        assert!(matches!(result, Err(crate::errors::AppError::Vault(_))));
    }

    #[tokio::test]
    async fn authenticated_blob_fetch_retries_once_after_401() {
        let server = MockServer::start().await;
        let bytes = b"blob-bytes";
        let hash = format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)));
        let blob_path = format!("blobs/sha256/{}.skill.zip", &hash[7..]);
        Mock::given(method("GET"))
            .and(path(format!("/repos/owner/vault/contents/{blob_path}")))
            .and(query_param("ref", "main"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/owner/vault/contents/{blob_path}")))
            .and(query_param("ref", "main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(content_body(bytes, "blob-sha")))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new-token",
                "refresh_token": "new-refresh",
                "expires_in": 3600,
                "refresh_token_expires_in": 86400
            })))
            .mount(&server)
            .await;

        let actual = store(&server)
            .await
            .fetch_blob(&blob_path, &hash)
            .await
            .unwrap();
        assert_eq!(actual, bytes);
    }

    #[tokio::test]
    async fn rate_limited_fetch_preserves_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("retry-after", "30")
                    .set_body_json(serde_json::json!({"message": "rate limit"})),
            )
            .mount(&server)
            .await;

        let result = store(&server).await.fetch_manifest().await;
        assert!(matches!(
            result,
            Err(crate::errors::AppError::RateLimited {
                retry_after: Some(value)
            }) if value == "30"
        ));
    }

    async fn mount_head(server: &MockServer, sha: &str) {
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": { "sha": sha }
            })))
            .mount(server)
            .await;
    }

    /// 5xx PATCH 响应经 response_error 转换后的原始错误。
    fn response_error_5xx() -> AppError {
        AppError::Vault("update branch ref failed with status 503 Server Error".into())
    }

    /// 传输错误（如超时）经 map_http_err 转换后的原始错误。
    fn transport_error() -> AppError {
        AppError::Auth("http error: timeout".into())
    }

    #[tokio::test]
    async fn reconcile_candidate_head_succeeds_for_response_and_transport_errors() {
        for original in [response_error_5xx(), transport_error()] {
            let server = MockServer::start().await;
            mount_head(&server, "candidate").await;
            let result = store(&server)
                .await
                .reconcile_update("base", "candidate", original)
                .await
                .unwrap();
            assert_eq!(result.commit_sha, "candidate");
        }
    }

    #[tokio::test]
    async fn reconcile_base_head_returns_original_error_for_both_error_types() {
        let server = MockServer::start().await;
        mount_head(&server, "base").await;
        let vault_store = store(&server).await;

        let result = vault_store
            .reconcile_update("base", "candidate", response_error_5xx())
            .await;
        assert!(matches!(result, Err(AppError::Vault(msg)) if msg.contains("503")));

        let result = vault_store
            .reconcile_update("base", "candidate", transport_error())
            .await;
        assert!(matches!(result, Err(AppError::Auth(msg)) if msg == "http error: timeout"));
    }

    #[tokio::test]
    async fn reconcile_other_head_returns_remote_changed_for_both_error_types() {
        let server = MockServer::start().await;
        mount_head(&server, "other").await;
        let vault_store = store(&server).await;
        for original in [response_error_5xx(), transport_error()] {
            let result = vault_store
                .reconcile_update("base", "candidate", original)
                .await;
            assert!(matches!(result, Err(AppError::RemoteChanged(_))));
        }
    }

    #[tokio::test]
    async fn reconcile_unreadable_head_returns_outcome_unknown_for_both_error_types() {
        for original in [response_error_5xx(), transport_error()] {
            let server = MockServer::start().await;
            let result = store(&server)
                .await
                .reconcile_update("base", "candidate", original)
                .await;
            assert!(matches!(
                result,
                Err(AppError::RemoteOutcomeUnknown {
                    base_commit_sha,
                    candidate_commit_sha
                }) if base_commit_sha == "base" && candidate_commit_sha == "candidate"
            ));
        }
    }

    async fn store_with_client(server: &MockServer, http: reqwest::Client) -> GitHubVaultStore {
        let credential_store = Arc::new(InMemoryCredentialStore::new());
        let auth = Arc::new(
            GithubAuthClient::new_with_urls(
                GithubAppPublicConfig::new("Iv1.test", "skill-sync").unwrap(),
                reqwest::Url::parse(&server.uri()).unwrap(),
                reqwest::Url::parse(&format!("{}/api/", server.uri())).unwrap(),
            )
            .unwrap(),
        );
        let manager = Arc::new(GithubCredentialManager::new(credential_store, auth));
        manager
            .save_initial(&GithubCredential {
                schema: 1,
                generation: Uuid::new_v4(),
                access_token: SecretString::new("token".into()),
                refresh_token: SecretString::new("refresh".into()),
                access_expires_at: Utc::now() + Duration::hours(1),
                refresh_expires_at: Utc::now() + Duration::days(1),
                github_login: "octocat".into(),
                app_client_id: "Iv1.test".into(),
            })
            .await
            .unwrap();
        let api = Arc::new(GithubAuthenticatedClient::new_with_client(
            manager,
            "Iv1.test".into(),
            reqwest::Url::parse(&format!("{}/", server.uri())).unwrap(),
            http,
        ));
        let cfg = remote();
        GitHubVaultStore::new(
            api,
            GithubRepositoryContext {
                installation_id: cfg.installation_id,
                owner: cfg.owner,
                repo: cfg.repo,
                branch: cfg.branch,
                head_sha: "head".into(),
            },
        )
    }

    /// 完整 commit_changes 传输测试：注入短超时 client，PATCH 响应延迟超过超时 -> 传输错误 ->
    /// 走 reconciliation；两次 preflight HEAD 读返回 base，reconciliation 读返回 candidate，
    /// 证明真实 PATCH 传输失败能到达 reconciliation 并按 HEAD 结果裁决。
    #[tokio::test]
    async fn commit_changes_reconciles_transport_failure_to_candidate_head() {
        let server = MockServer::start().await;
        let head_reads = Arc::new(AtomicUsize::new(0));
        let head_reads_clone = head_reads.clone();
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(move |_request: &wiremock::Request| {
                let count = head_reads_clone.fetch_add(1, Ordering::SeqCst);
                let sha = if count < 2 { "base" } else { "candidate" };
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "object": { "sha": sha }
                }))
            })
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/commits/base"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tree": { "sha": "base-tree" }
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/blobs"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "manifest-blob"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/trees"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "candidate-tree"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/commits"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "candidate"
            })))
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/repos/owner/vault/git/refs/heads/main"))
            .respond_with(
                ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(500)),
            )
            .mount(&server)
            .await;

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(50))
            .build()
            .unwrap();
        let vault_store = store_with_client(&server, http).await;
        let result = vault_store
            .commit_changes(RemoteChanges {
                base_commit_sha: "base".into(),
                blobs: vec![],
                next_manifest: VaultManifest::empty("device-test"),
                commit_message: "sync".into(),
            })
            .await
            .unwrap();
        assert_eq!(result.commit_sha, "candidate");
        assert_eq!(head_reads.load(Ordering::SeqCst), 3);
    }

    async fn mount_commit_pipeline(server: &MockServer, update_status: u16) {
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": { "sha": "base" }
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/commits/base"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tree": { "sha": "base-tree" }
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/blobs"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "manifest-blob"
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/trees"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "candidate-tree"
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/commits"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "candidate"
            })))
            .mount(server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/repos/owner/vault/git/refs/heads/main"))
            .and(body_json(serde_json::json!({
                "sha": "candidate",
                "force": false,
            })))
            .respond_with(ResponseTemplate::new(update_status))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn update_ref_conflicts_map_to_remote_changed() {
        for status in [409, 422] {
            let server = MockServer::start().await;
            mount_commit_pipeline(&server, status).await;
            let result = store(&server)
                .await
                .commit_changes(RemoteChanges {
                    base_commit_sha: "base".into(),
                    blobs: vec![],
                    next_manifest: VaultManifest::empty("device-test"),
                    commit_message: "sync".into(),
                })
                .await;
            assert!(
                matches!(result, Err(crate::errors::AppError::RemoteChanged(_))),
                "status {status}"
            );
        }
    }

    #[tokio::test]
    async fn non_ref_conflicts_map_to_vault_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": { "sha": "base" }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/commits/base"))
            .respond_with(ResponseTemplate::new(409))
            .mount(&server)
            .await;

        let result = store(&server)
            .await
            .commit_changes(RemoteChanges {
                base_commit_sha: "base".into(),
                blobs: vec![],
                next_manifest: VaultManifest::empty("device-test"),
                commit_message: "sync".into(),
            })
            .await;
        assert!(matches!(result, Err(crate::errors::AppError::Vault(_))));
    }

    #[tokio::test]
    async fn authenticated_commit_retries_post_after_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/blobs"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new-token",
                "refresh_token": "new-refresh",
                "expires_in": 3600,
                "refresh_token_expires_in": 86400
            })))
            .mount(&server)
            .await;
        mount_commit_pipeline(&server, 200).await;

        let result = store(&server)
            .await
            .commit_changes(RemoteChanges {
                base_commit_sha: "base".into(),
                blobs: vec![],
                next_manifest: VaultManifest::empty("device-test"),
                commit_message: "sync".into(),
            })
            .await
            .unwrap();
        assert_eq!(result.commit_sha, "candidate");
    }

    #[tokio::test]
    async fn authenticated_commit_clears_after_second_post_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/blobs"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new-token",
                "refresh_token": "new-refresh",
                "expires_in": 3600,
                "refresh_token_expires_in": 86400
            })))
            .mount(&server)
            .await;
        mount_commit_pipeline(&server, 200).await;

        let result = store(&server)
            .await
            .commit_changes(RemoteChanges {
                base_commit_sha: "base".into(),
                blobs: vec![],
                next_manifest: VaultManifest::empty("device-test"),
                commit_message: "sync".into(),
            })
            .await;
        assert!(matches!(
            result,
            Err(crate::errors::AppError::ReauthorizationRequired(_))
        ));
    }

    #[tokio::test]
    async fn commit_changes_updates_ref_without_force() {
        let server = MockServer::start().await;
        let blob_bytes = b"upload-bytes".to_vec();
        let blob_hash = format!("sha256:{}", hex::encode(sha2::Sha256::digest(&blob_bytes)));
        let blob_path = format!("blobs/sha256/{}.skill.zip", &blob_hash[7..]);
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": { "sha": "base" }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/vault/git/commits/base"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tree": { "sha": "base-tree" }
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/blobs"))
            .and(body_json(serde_json::json!({
                "content": base64::engine::general_purpose::STANDARD.encode(&blob_bytes),
                "encoding": "base64",
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "upload-blob"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/blobs"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "manifest-blob"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/trees"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "candidate-tree"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/owner/vault/git/commits"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "sha": "candidate"
            })))
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/repos/owner/vault/git/refs/heads/main"))
            .and(body_json(serde_json::json!({
                "sha": "candidate",
                "force": false,
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let result = store(&server)
            .await
            .commit_changes(RemoteChanges {
                base_commit_sha: "base".into(),
                blobs: vec![BlobWrite {
                    path: blob_path,
                    bytes: blob_bytes,
                    expected_hash: blob_hash,
                }],
                next_manifest: VaultManifest::empty("device-test"),
                commit_message: "sync".into(),
            })
            .await
            .unwrap();
        assert_eq!(result.commit_sha, "candidate");
    }

    #[tokio::test]
    async fn commit_changes_rejects_blob_path_hash_mismatch_before_network() {
        let server = MockServer::start().await;
        let result = store(&server)
            .await
            .commit_changes(RemoteChanges {
                base_commit_sha: "base".into(),
                blobs: vec![BlobWrite {
                    path: "blobs/other.zip".into(),
                    bytes: b"bytes".to_vec(),
                    expected_hash:
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .into(),
                }],
                next_manifest: VaultManifest::empty("device-test"),
                commit_message: "sync".into(),
            })
            .await;
        assert!(matches!(result, Err(crate::errors::AppError::Vault(_))));
        assert_eq!(server.received_requests().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn commit_changes_rejects_duplicate_or_manifest_blob_paths_before_network() {
        let server = MockServer::start().await;
        let bytes = b"blob".to_vec();
        let expected_hash = format!("sha256:{}", hex::encode(sha2::Sha256::digest(&bytes)));
        let valid_blob = || BlobWrite {
            path: format!("blobs/sha256/{}.skill.zip", &expected_hash[7..]),
            bytes: bytes.clone(),
            expected_hash: expected_hash.clone(),
        };
        let duplicate = store(&server)
            .await
            .commit_changes(RemoteChanges {
                base_commit_sha: "base".into(),
                blobs: vec![valid_blob(), valid_blob()],
                next_manifest: VaultManifest::empty("device-test"),
                commit_message: "sync".into(),
            })
            .await;
        assert!(matches!(duplicate, Err(crate::errors::AppError::Vault(_))));
        assert_eq!(server.received_requests().await.unwrap().len(), 0);

        let server = MockServer::start().await;
        let manifest_path = store(&server)
            .await
            .commit_changes(RemoteChanges {
                base_commit_sha: "base".into(),
                blobs: vec![BlobWrite {
                    path: "manifest.json".into(),
                    bytes,
                    expected_hash,
                }],
                next_manifest: VaultManifest::empty("device-test"),
                commit_message: "sync".into(),
            })
            .await;
        assert!(matches!(
            manifest_path,
            Err(crate::errors::AppError::Vault(_))
        ));
        assert_eq!(server.received_requests().await.unwrap().len(), 0);
    }
}
