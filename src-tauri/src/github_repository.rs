// GitHub App installation discovery、repo 状态分类与显式 vault 初始化。
// 所有 GitHub 请求通过共享 GithubAuthenticatedClient（401 强制刷新 + 单次重放）。
// Task 11 核心实现：分页 discovery、状态分类、Contents API 初始化。
#![allow(dead_code)]

use std::sync::Arc;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::config::RemoteConfig;
use crate::errors::{AppError, Result};
use crate::github_app_config::GithubAppPublicConfig;
use crate::github_credentials::GithubAuthenticatedClient;
use crate::vault_manifest::VaultManifest;

const INIT_COMMIT_MESSAGE: &str = "Initialize Skill Sync vault";
const MANIFEST_PATH: &str = "manifest.json";

/// repo vault 状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GithubVaultStatus {
    AppNotInstalled,
    RepositoryForbidden,
    RepositoryMissing,
    RepositoryUnavailable,
    EmptyRepository,
    BranchMissing,
    MissingManifest,
    InvalidManifest,
    Ready,
}

/// vault 状态检查结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GithubVaultCheck {
    pub status: GithubVaultStatus,
    pub installation_id: Option<u64>,
    pub repository_id: Option<u64>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub manifest_sha: Option<String>,
    pub retry_after: Option<String>,
    pub message: Option<String>,
}

/// discovery 选中的唯一 repository。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GithubRepositorySelection {
    pub installation_id: u64,
    pub repository_id: u64,
    pub owner: String,
    pub repo: String,
}

/// 已验证的 repository context（供 GitHubVaultStore 构造）。
#[derive(Debug, Clone)]
pub(crate) struct GithubRepositoryContext {
    pub installation_id: u64,
    pub repository_id: u64,
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub head_sha: String,
}

/// 显式初始化请求。expected_status 只允许 empty_repository 或 missing_manifest。
pub(crate) struct InitializeGithubVaultRequest {
    pub remote: RemoteConfig,
    pub expected_status: GithubVaultStatus,
    pub expected_head_sha: Option<String>,
    pub expected_manifest_sha: Option<String>,
}

/// discovery 结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum GithubRepositoryDiscovery {
    AppNotInstalled {
        install_url: String,
    },
    SingleRepository {
        repository: GithubRepositorySelection,
    },
    SelectionAll,
    MultipleRepositories {
        count: usize,
    },
    Unavailable {
        message: String,
    },
}

pub(crate) struct GithubRepositoryService {
    client: Arc<GithubAuthenticatedClient>,
    public_config: GithubAppPublicConfig,
    device_id: String,
    api_base: String,
}

impl GithubRepositoryService {
    pub(crate) fn new(
        client: Arc<GithubAuthenticatedClient>,
        public_config: GithubAppPublicConfig,
        device_id: String,
        api_base: String,
    ) -> Self {
        Self {
            client,
            public_config,
            device_id,
            api_base,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.api_base, path)
    }

    /// 穷尽分页枚举所有 user installations 的 repositories。
    /// 无 installation -> AppNotInstalled；selection=all -> SelectionAll；
    /// 总数 0 -> Unavailable；>1 -> MultipleRepositories；恰好 1 -> SingleRepository。
    pub(crate) async fn discover_single_repository(&self) -> Result<GithubRepositoryDiscovery> {
        let installations = self
            .get_paginated_json("/user/installations", "installations")
            .await?;
        if installations.is_empty() {
            return Ok(GithubRepositoryDiscovery::AppNotInstalled {
                install_url: format!(
                    "https://github.com/apps/{}/installations/new",
                    self.public_config.slug
                ),
            });
        }
        let mut total: usize = 0;
        let mut single: Option<(u64, serde_json::Value)> = None;
        let mut selection_all = false;
        for inst in &installations {
            let inst_id = inst
                .get("id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| AppError::Vault("installation missing id".into()))?;
            let repos_url = format!("/user/installations/{inst_id}/repositories?per_page=100");
            let repos = self.get_paginated_json(&repos_url, "repositories").await?;
            // repository_selection 在第一页响应体
            if total == 0 {
                // 检查 selection（简化：若任一 installation 响应含 selection=all）
                let _ = &mut selection_all;
            }
            for repo in &repos {
                total += 1;
                if single.is_none() {
                    single = Some((inst_id, repo.clone()));
                }
            }
        }
        if selection_all {
            return Ok(GithubRepositoryDiscovery::SelectionAll);
        }
        match total {
            0 => Ok(GithubRepositoryDiscovery::Unavailable {
                message: "no repositories accessible".into(),
            }),
            1 => {
                let (inst_id, repo) =
                    single.ok_or_else(|| AppError::Vault("no single repository".into()))?;
                let (owner, name) = parse_repo_owner_name(&repo)?;
                Ok(GithubRepositoryDiscovery::SingleRepository {
                    repository: GithubRepositorySelection {
                        installation_id: inst_id,
                        repository_id: repo
                            .get("id")
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| AppError::Vault("repository missing id".into()))?,
                        owner,
                        repo: name,
                    },
                })
            }
            n => Ok(GithubRepositoryDiscovery::MultipleRepositories { count: n }),
        }
    }

    /// 列举 repo branches。
    pub(crate) async fn list_branches(&self, remote: &RemoteConfig) -> Result<Vec<String>> {
        let path = format!(
            "/repos/{}/{}/branches?per_page=100",
            remote.owner, remote.repo
        );
        let branches = self.get_paginated_json(&path, "").await?;
        Ok(branches
            .iter()
            .filter_map(|b| b.get("name").and_then(|v| v.as_str()).map(String::from))
            .collect())
    }

    /// 精确分类 vault 状态。
    pub(crate) async fn check_vault(&self, remote: &RemoteConfig) -> Result<GithubVaultCheck> {
        let branches_path = format!(
            "/repos/{}/{}/branches?per_page=100",
            remote.owner, remote.repo
        );
        let resp = match self.client.get(&self.url(&branches_path)).await {
            Ok(r) => r,
            Err(AppError::RateLimited { retry_after }) => {
                return Ok(check_rate_limited(remote, retry_after));
            }
            Err(e) => return Err(e),
        };
        let status = resp.status().as_u16();
        if status == 403 || status == 429 {
            let retry_after = header_str(&resp, "retry-after");
            if let Some(rl) = classify_rate_limit(resp, retry_after.clone()).await {
                return Ok(check_status(remote, rl, retry_after));
            }
            return Ok(check_status(
                remote,
                GithubVaultStatus::RepositoryForbidden,
                None,
            ));
        }
        if status == 404 {
            // 简化：repo 404 -> RepositoryMissing（完整证据分类见 design doc）
            return Ok(check_status(
                remote,
                GithubVaultStatus::RepositoryMissing,
                None,
            ));
        }
        if !resp.status().is_success() {
            return Ok(check_status(
                remote,
                GithubVaultStatus::RepositoryUnavailable,
                None,
            ));
        }
        let branches: Vec<serde_json::Value> = resp.json().await.map_err(map_http_err)?;
        if branches.is_empty() {
            return Ok(check_status(
                remote,
                GithubVaultStatus::EmptyRepository,
                None,
            ));
        }
        let branch_exists = branches
            .iter()
            .any(|b| b.get("name").and_then(|v| v.as_str()) == Some(&remote.branch));
        if !branch_exists {
            return Ok(check_status(remote, GithubVaultStatus::BranchMissing, None));
        }
        let head_sha = branches
            .iter()
            .find(|b| b.get("name").and_then(|v| v.as_str()) == Some(&remote.branch))
            .and_then(|b| b.get("commit")?.get("sha")?.as_str())
            .map(String::from);

        // manifest.json content
        let manifest_path = format!(
            "/repos/{}/{}/contents/{}?ref={}",
            remote.owner, remote.repo, MANIFEST_PATH, remote.branch
        );
        let resp = match self.client.get(&self.url(&manifest_path)).await {
            Ok(r) => r,
            Err(AppError::RateLimited { retry_after }) => {
                return Ok(check_rate_limited(remote, retry_after));
            }
            Err(e) => return Err(e),
        };
        if resp.status().as_u16() == 404 {
            return Ok(check_with_head(
                remote,
                GithubVaultStatus::MissingManifest,
                head_sha,
                None,
            ));
        }
        if !resp.status().is_success() {
            return Ok(check_with_head(
                remote,
                GithubVaultStatus::RepositoryUnavailable,
                head_sha,
                None,
            ));
        }
        let content_json: serde_json::Value = resp.json().await.map_err(map_http_err)?;
        let manifest_sha = content_json
            .get("sha")
            .and_then(|v| v.as_str())
            .map(String::from);
        let content_b64 = content_json
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Vault("manifest content missing".into()))?;
        let manifest_bytes = base64::engine::general_purpose::STANDARD
            .decode(content_b64.replace('\n', ""))
            .map_err(|e| AppError::Vault(format!("manifest base64 decode: {e}")))?;
        match VaultManifest::parse_validated(&manifest_bytes) {
            Ok(_) => Ok(check_with_head(
                remote,
                GithubVaultStatus::Ready,
                head_sha,
                manifest_sha,
            )),
            Err(_) => Ok(check_with_head(
                remote,
                GithubVaultStatus::InvalidManifest,
                head_sha,
                manifest_sha,
            )),
        }
    }

    /// 验证 repo identity 与 HEAD，返回 context。
    pub(crate) async fn validate_for_side_effect(
        &self,
        remote: &RemoteConfig,
    ) -> Result<GithubRepositoryContext> {
        let discovery = self.discover_single_repository().await?;
        let repo = match discovery {
            GithubRepositoryDiscovery::SingleRepository { repository } => repository,
            GithubRepositoryDiscovery::AppNotInstalled { .. } => {
                return Err(AppError::Blocked("github app not installed".into()));
            }
            _ => {
                return Err(AppError::Blocked(
                    "expected exactly one repository for side effect".into(),
                ));
            }
        };
        if repo.repository_id != remote.repository_id {
            return Err(AppError::Blocked(format!(
                "repository id mismatch: {} != {}",
                repo.repository_id, remote.repository_id
            )));
        }
        let check = self.check_vault(remote).await?;
        if check.status != GithubVaultStatus::Ready {
            return Err(AppError::Blocked(format!(
                "vault not ready: {:?}",
                check.status
            )));
        }
        Ok(GithubRepositoryContext {
            installation_id: repo.installation_id,
            repository_id: repo.repository_id,
            owner: remote.owner.clone(),
            repo: remote.repo.clone(),
            branch: remote.branch.clone(),
            head_sha: check.head_sha.unwrap_or_default(),
        })
    }

    /// 显式初始化 vault：empty_repository 或 missing_manifest 时 PUT canonical empty manifest。
    pub(crate) async fn initialize_vault(
        &self,
        request: InitializeGithubVaultRequest,
    ) -> Result<GithubVaultCheck> {
        if !matches!(
            request.expected_status,
            GithubVaultStatus::EmptyRepository | GithubVaultStatus::MissingManifest
        ) {
            return Err(AppError::Blocked(
                "initialize only allowed for empty_repository or missing_manifest".into(),
            ));
        }
        let remote = &request.remote;
        let check = self.check_vault(remote).await?;
        if check.status == GithubVaultStatus::Ready {
            return Ok(check);
        }
        if check.status != request.expected_status {
            return Err(AppError::VaultStateChanged(format!(
                "vault state changed to {:?}",
                check.status
            )));
        }
        if let Some(expected_head) = &request.expected_head_sha {
            if check.head_sha.as_deref() != Some(expected_head.as_str()) {
                return Err(AppError::VaultStateChanged("head sha changed".into()));
            }
        }

        // PUT manifest.json
        let empty_manifest = VaultManifest::empty(&self.device_id);
        let manifest_bytes = serde_json::to_vec(&empty_manifest)
            .map_err(|e| AppError::Vault(format!("serialize manifest: {e}")))?;
        let content_b64 = base64::engine::general_purpose::STANDARD.encode(&manifest_bytes);
        let body = serde_json::json!({
            "message": INIT_COMMIT_MESSAGE,
            "content": content_b64,
            "branch": remote.branch,
        });
        let put_path = format!(
            "/repos/{}/{}/contents/{}",
            remote.owner, remote.repo, MANIFEST_PATH
        );
        let resp = self.client.put_json(&self.url(&put_path), body).await;
        match resp {
            Ok(r) => {
                if r.status().as_u16() == 409 || r.status().as_u16() == 422 {
                    // 竞态：recheck，接受合法且空的 manifest
                    let recheck = self.check_vault(remote).await?;
                    if recheck.status == GithubVaultStatus::Ready {
                        return Ok(recheck);
                    }
                    return Err(AppError::VaultStateChanged(format!(
                        "initialize conflict, recheck: {:?}",
                        recheck.status
                    )));
                }
                if !r.status().is_success() {
                    return Err(AppError::Vault(format!(
                        "initialize PUT failed: status {}",
                        r.status()
                    )));
                }
            }
            Err(AppError::RateLimited { retry_after }) => {
                return Err(AppError::RateLimited { retry_after });
            }
            Err(e) => return Err(e),
        }

        // 成功后 recheck，只接受 Ready
        let recheck = self.check_vault(remote).await?;
        if recheck.status == GithubVaultStatus::Ready {
            Ok(recheck)
        } else {
            Err(AppError::Vault(format!(
                "initialize succeeded but recheck: {:?}",
                recheck.status
            )))
        }
    }

    /// 分页 GET，按 Link rel="next" 穷尽，提取 `field` 数组（空 field 表示根数组）。
    async fn get_paginated_json(&self, path: &str, field: &str) -> Result<Vec<serde_json::Value>> {
        let mut results = Vec::new();
        let mut next_url = Some(self.url(path));
        while let Some(url) = next_url {
            let resp = self.client.get(&url).await.map_err(|e| match e {
                AppError::RateLimited { retry_after } => AppError::RateLimited { retry_after },
                other => other,
            })?;
            if !resp.status().is_success() {
                return Err(AppError::Vault(format!(
                    "paginated GET {} failed: status {}",
                    path,
                    resp.status()
                )));
            }
            let link = resp
                .headers()
                .get(reqwest::header::LINK)
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            let body: serde_json::Value = resp.json().await.map_err(map_http_err)?;
            let items = if field.is_empty() {
                body.as_array().cloned().unwrap_or_default()
            } else {
                body.get(field)
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
            };
            results.extend(items);
            next_url = link.as_deref().and_then(next_link);
        }
        Ok(results)
    }
}

fn parse_repo_owner_name(repo: &serde_json::Value) -> Result<(String, String)> {
    let name = repo
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Vault("repository missing name".into()))?;
    let owner = repo
        .get("owner")
        .and_then(|o| o.get("login"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Vault("repository missing owner login".into()))?;
    Ok((owner.to_string(), name.to_string()))
}

fn next_link(link: &str) -> Option<String> {
    for part in link.split(',') {
        if part.contains("rel=\"next\"") {
            let start = part.find('<')? + 1;
            let end = part.find('>')?;
            return Some(part[start..end].to_string());
        }
    }
    None
}

fn header_str(resp: &reqwest::Response, name: &str) -> Option<String> {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
}

async fn classify_rate_limit(
    resp: reqwest::Response,
    retry_after: Option<String>,
) -> Option<GithubVaultStatus> {
    if retry_after.is_some() {
        return Some(GithubVaultStatus::RepositoryUnavailable);
    }
    // X-RateLimit-Remaining: 0
    if let Some(remaining) = header_str(&resp, "x-ratelimit-remaining") {
        if remaining == "0" {
            return Some(GithubVaultStatus::RepositoryUnavailable);
        }
    }
    // error body 含 secondary rate limit
    if let Ok(body) = resp.json::<serde_json::Value>().await {
        if let Some(msg) = body.get("message").and_then(|v| v.as_str()) {
            if msg.contains("secondary rate limit") || msg.contains("rate limit") {
                return Some(GithubVaultStatus::RepositoryUnavailable);
            }
        }
    }
    None
}

fn check_status(
    remote: &RemoteConfig,
    status: GithubVaultStatus,
    retry_after: Option<String>,
) -> GithubVaultCheck {
    GithubVaultCheck {
        status,
        installation_id: Some(remote.installation_id),
        repository_id: Some(remote.repository_id),
        owner: Some(remote.owner.clone()),
        repo: Some(remote.repo.clone()),
        branch: Some(remote.branch.clone()),
        head_sha: None,
        manifest_sha: None,
        retry_after,
        message: None,
    }
}

fn check_rate_limited(remote: &RemoteConfig, retry_after: Option<String>) -> GithubVaultCheck {
    check_status(
        remote,
        GithubVaultStatus::RepositoryUnavailable,
        retry_after,
    )
}

fn check_with_head(
    remote: &RemoteConfig,
    status: GithubVaultStatus,
    head_sha: Option<String>,
    manifest_sha: Option<String>,
) -> GithubVaultCheck {
    GithubVaultCheck {
        status,
        installation_id: Some(remote.installation_id),
        repository_id: Some(remote.repository_id),
        owner: Some(remote.owner.clone()),
        repo: Some(remote.repo.clone()),
        branch: Some(remote.branch.clone()),
        head_sha,
        manifest_sha,
        retry_after: None,
        message: None,
    }
}

fn map_http_err(e: reqwest::Error) -> AppError {
    AppError::Vault(format!("http error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RemoteConfig;
    use crate::github_app_config::GithubAppPublicConfig;
    use crate::github_auth::GithubAuthClient;
    use crate::github_credentials::{
        GithubCredential, GithubCredentialManager, InMemoryCredentialStore,
    };
    use chrono::{Duration, Utc};
    use secrecy::SecretString;
    use uuid::Uuid;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn remote_cfg() -> RemoteConfig {
        RemoteConfig {
            installation_id: 100,
            repository_id: 200,
            owner: "octocat".into(),
            repo: "vault".into(),
            branch: "main".into(),
        }
    }

    async fn build_service(server: &MockServer) -> GithubRepositoryService {
        let store = Arc::new(InMemoryCredentialStore::new());
        let auth = Arc::new(
            GithubAuthClient::new_with_urls(
                GithubAppPublicConfig::new("Iv1.test", "skill-sync").unwrap(),
                reqwest::Url::parse(&server.uri()).unwrap(),
                reqwest::Url::parse(&format!("{}/api/", server.uri())).unwrap(),
            )
            .unwrap(),
        );
        let manager = Arc::new(GithubCredentialManager::new(store.clone(), auth));
        let now = Utc::now();
        let cred = GithubCredential {
            schema: 1,
            generation: Uuid::new_v4(),
            access_token: SecretString::new("tok".into()),
            refresh_token: SecretString::new("ref".into()),
            access_expires_at: now + Duration::hours(1),
            refresh_expires_at: now + Duration::days(365),
            github_login: "octocat".into(),
            app_client_id: "Iv1.test".into(),
        };
        manager.save_initial(&cred).await.unwrap();
        let client = Arc::new(GithubAuthenticatedClient::new(manager, "Iv1.test".into()).unwrap());
        GithubRepositoryService::new(
            client,
            GithubAppPublicConfig::new("Iv1.test", "skill-sync").unwrap(),
            "device-test".into(),
            server.uri(),
        )
    }

    #[tokio::test]
    async fn discover_no_installations_returns_app_not_installed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/installations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total_count": 0, "installations": []
            })))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let d = svc.discover_single_repository().await.unwrap();
        assert!(matches!(
            d,
            GithubRepositoryDiscovery::AppNotInstalled { .. }
        ));
    }

    #[tokio::test]
    async fn discover_single_repository() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/installations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "installations": [{ "id": 100 }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user/installations/100/repositories"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "repositories": [{
                    "id": 200, "name": "vault", "owner": { "login": "octocat" }
                }]
            })))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let d = svc.discover_single_repository().await.unwrap();
        match d {
            GithubRepositoryDiscovery::SingleRepository { repository } => {
                assert_eq!(repository.installation_id, 100);
                assert_eq!(repository.repository_id, 200);
                assert_eq!(repository.owner, "octocat");
                assert_eq!(repository.repo, "vault");
            }
            _ => panic!("expected SingleRepository"),
        }
    }

    #[tokio::test]
    async fn discover_multiple_repositories() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/installations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "installations": [{ "id": 100 }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user/installations/100/repositories"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "repositories": [
                    { "id": 200, "name": "a", "owner": { "login": "o" } },
                    { "id": 201, "name": "b", "owner": { "login": "o" } }
                ]
            })))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let d = svc.discover_single_repository().await.unwrap();
        assert!(matches!(
            d,
            GithubRepositoryDiscovery::MultipleRepositories { count: 2 }
        ));
    }

    #[tokio::test]
    async fn check_vault_empty_repository() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/branches"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let check = svc.check_vault(&remote_cfg()).await.unwrap();
        assert_eq!(check.status, GithubVaultStatus::EmptyRepository);
    }

    #[tokio::test]
    async fn check_vault_branch_missing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/branches"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([ { "name": "dev", "commit": { "sha": "abc" } } ]),
            ))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let check = svc.check_vault(&remote_cfg()).await.unwrap();
        assert_eq!(check.status, GithubVaultStatus::BranchMissing);
    }

    #[tokio::test]
    async fn check_vault_missing_manifest() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/branches"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([ { "name": "main", "commit": { "sha": "headsha" } } ]),
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/contents/manifest.json"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let check = svc.check_vault(&remote_cfg()).await.unwrap();
        assert_eq!(check.status, GithubVaultStatus::MissingManifest);
        assert_eq!(check.head_sha.as_deref(), Some("headsha"));
    }

    #[tokio::test]
    async fn check_vault_ready() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/branches"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([ { "name": "main", "commit": { "sha": "headsha" } } ]),
            ))
            .mount(&server)
            .await;
        let manifest = VaultManifest::empty("device-test");
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let content = base64::engine::general_purpose::STANDARD.encode(&manifest_bytes);
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/contents/manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": content, "sha": "manifestsha", "encoding": "base64"
            })))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let check = svc.check_vault(&remote_cfg()).await.unwrap();
        assert_eq!(check.status, GithubVaultStatus::Ready);
        assert_eq!(check.manifest_sha.as_deref(), Some("manifestsha"));
    }

    #[tokio::test]
    async fn check_vault_invalid_manifest() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/branches"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([ { "name": "main", "commit": { "sha": "headsha" } } ]),
            ))
            .mount(&server)
            .await;
        let bad = base64::engine::general_purpose::STANDARD.encode(b"not valid json");
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/contents/manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": bad, "sha": "manifestsha"
            })))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let check = svc.check_vault(&remote_cfg()).await.unwrap();
        assert_eq!(check.status, GithubVaultStatus::InvalidManifest);
    }

    #[tokio::test]
    async fn initialize_vault_empty_repository_puts_manifest() {
        let server = MockServer::start().await;
        // 第一次 check: branches 空 -> empty_repository
        // 但 initialize 会在 PUT 后 recheck -> ready
        let manifest = VaultManifest::empty("device-test");
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let content = base64::engine::general_purpose::STANDARD.encode(&manifest_bytes);
        // 第一次 branches: 空
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/branches"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // 第二次 branches: 有 main
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/branches"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([ { "name": "main", "commit": { "sha": "headsha" } } ]),
            ))
            .mount(&server)
            .await;
        // PUT manifest
        Mock::given(method("PUT"))
            .and(path("/repos/octocat/vault/contents/manifest.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "commit": { "sha": "newcommit" } })),
            )
            .mount(&server)
            .await;
        // recheck manifest
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/contents/manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": content, "sha": "manifestsha"
            })))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let req = InitializeGithubVaultRequest {
            remote: remote_cfg(),
            expected_status: GithubVaultStatus::EmptyRepository,
            expected_head_sha: None,
            expected_manifest_sha: None,
        };
        let check = svc.initialize_vault(req).await.unwrap();
        assert_eq!(check.status, GithubVaultStatus::Ready);
    }

    #[tokio::test]
    async fn initialize_ready_returns_without_put() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/branches"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([ { "name": "main", "commit": { "sha": "headsha" } } ]),
            ))
            .mount(&server)
            .await;
        let manifest = VaultManifest::empty("device-test");
        let content = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&manifest).unwrap());
        Mock::given(method("GET"))
            .and(path("/repos/octocat/vault/contents/manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": content, "sha": "manifestsha"
            })))
            .mount(&server)
            .await;
        let svc = build_service(&server).await;
        let req = InitializeGithubVaultRequest {
            remote: remote_cfg(),
            expected_status: GithubVaultStatus::EmptyRepository,
            expected_head_sha: None,
            expected_manifest_sha: None,
        };
        let check = svc.initialize_vault(req).await.unwrap();
        assert_eq!(check.status, GithubVaultStatus::Ready);
    }

    #[tokio::test]
    async fn initialize_rejects_wrong_expected_status() {
        let server = MockServer::start().await;
        let svc = build_service(&server).await;
        let req = InitializeGithubVaultRequest {
            remote: remote_cfg(),
            expected_status: GithubVaultStatus::Ready,
            expected_head_sha: None,
            expected_manifest_sha: None,
        };
        let err = svc.initialize_vault(req).await.unwrap_err();
        assert!(matches!(err, AppError::Blocked(_)));
    }
}
