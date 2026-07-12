// GitHub credential：versioned credential DTO、CredentialStore trait（内存 + keyring）、
// 单飞刷新的 GithubCredentialManager，以及共享 GithubAuthenticatedClient（401 强制刷新 + 单次重放）。
// 不给 GithubCredential/SecretString 派生 Serialize；keyring 内部用私有 StoredGithubCredential
// 作为唯一 JSON 边界，只在 blocking closure 内 ExposeSecret 转换。
#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::errors::{AppError, Result};
use crate::github_auth::GithubAuthClient;

const SCHEMA: u32 = 1;
const ACCESS_SKEW_SECS: i64 = 300;
const KEYRING_SERVICE: &str = "skill-sync";
const KEYRING_USER: &str = "github-credential";

/// versioned GitHub credential。token 用 SecretString，不派生 Serialize。
#[derive(Debug, Clone)]
pub(crate) struct GithubCredential {
    pub schema: u32,
    pub generation: Uuid,
    pub access_token: SecretString,
    pub refresh_token: SecretString,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub github_login: String,
    pub app_client_id: String,
}

impl GithubCredential {
    /// access token 是否在 5 分钟 skew 内过期（需要刷新）。
    pub(crate) fn needs_refresh(&self) -> bool {
        Utc::now() > self.access_expires_at - Duration::seconds(ACCESS_SKEW_SECS)
    }

    /// refresh token 是否已过期（需要重新授权）。
    pub(crate) fn refresh_expired(&self) -> bool {
        Utc::now() > self.refresh_expires_at
    }
}

/// keyring JSON 边界：token 为 String，不派生 Debug，避免明文泄露。
#[derive(Serialize, Deserialize)]
struct StoredGithubCredential {
    schema: u32,
    generation: Uuid,
    access_token: String,
    refresh_token: String,
    access_expires_at: DateTime<Utc>,
    refresh_expires_at: DateTime<Utc>,
    github_login: String,
    app_client_id: String,
}

fn to_stored(c: &GithubCredential) -> StoredGithubCredential {
    StoredGithubCredential {
        schema: c.schema,
        generation: c.generation,
        access_token: c.access_token.expose_secret().to_string(),
        refresh_token: c.refresh_token.expose_secret().to_string(),
        access_expires_at: c.access_expires_at,
        refresh_expires_at: c.refresh_expires_at,
        github_login: c.github_login.clone(),
        app_client_id: c.app_client_id.clone(),
    }
}

fn from_stored(s: StoredGithubCredential) -> GithubCredential {
    GithubCredential {
        schema: s.schema,
        generation: s.generation,
        access_token: SecretString::new(s.access_token.into()),
        refresh_token: SecretString::new(s.refresh_token.into()),
        access_expires_at: s.access_expires_at,
        refresh_expires_at: s.refresh_expires_at,
        github_login: s.github_login,
        app_client_id: s.app_client_id,
    }
}

/// credential 存储抽象。
#[async_trait]
pub(crate) trait CredentialStore: Send + Sync {
    async fn load(&self) -> Result<Option<GithubCredential>>;
    async fn replace(&self, credential: &GithubCredential) -> Result<()>;
    async fn clear(&self) -> Result<()>;
}

/// 内存 store（测试用）：默认运行的完整授权与轮换测试使用它。
pub(crate) struct InMemoryCredentialStore {
    inner: Arc<Mutex<Option<GithubCredential>>>,
}

impl InMemoryCredentialStore {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl CredentialStore for InMemoryCredentialStore {
    async fn load(&self) -> Result<Option<GithubCredential>> {
        Ok(self.inner.lock().await.clone())
    }
    async fn replace(&self, credential: &GithubCredential) -> Result<()> {
        *self.inner.lock().await = Some(credential.clone());
        Ok(())
    }
    async fn clear(&self) -> Result<()> {
        *self.inner.lock().await = None;
        Ok(())
    }
}

/// OS keyring adapter：load/replace/clear 各自在 spawn_blocking closure 内完成。
pub(crate) struct KeyringCredentialStore {
    service: String,
    user: String,
}

impl KeyringCredentialStore {
    pub(crate) fn new() -> Self {
        Self {
            service: KEYRING_SERVICE.into(),
            user: KEYRING_USER.into(),
        }
    }
}

#[async_trait]
impl CredentialStore for KeyringCredentialStore {
    async fn load(&self) -> Result<Option<GithubCredential>> {
        let service = self.service.clone();
        let user = self.user.clone();
        let result: Result<Option<GithubCredential>> =
            tauri::async_runtime::spawn_blocking(move || {
                let entry = keyring::Entry::new(&service, &user)
                    .map_err(|e| AppError::Auth(format!("keyring entry: {e}")))?;
                match entry.get_password() {
                    Ok(json) => {
                        let stored: StoredGithubCredential = serde_json::from_str(&json)
                            .map_err(|e| AppError::Auth(format!("keyring decode: {e}")))?;
                        let cred = from_stored(stored);
                        Ok(Some(cred))
                    }
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(e) => Err(AppError::Auth(format!("keyring load: {e}"))),
                }
            })
            .await
            .map_err(|e| AppError::Auth(format!("keyring task failed: {e}")))?;
        result
    }

    async fn replace(&self, credential: &GithubCredential) -> Result<()> {
        let service = self.service.clone();
        let user = self.user.clone();
        let cred = credential.clone();
        let result: Result<()> = tauri::async_runtime::spawn_blocking(move || {
            let stored = to_stored(&cred);
            let json = serde_json::to_string(&stored)
                .map_err(|e| AppError::Auth(format!("keyring encode: {e}")))?;
            let entry = keyring::Entry::new(&service, &user)
                .map_err(|e| AppError::Auth(format!("keyring entry: {e}")))?;
            entry
                .set_password(&json)
                .map_err(|e| AppError::Auth(format!("keyring replace: {e}")))
        })
        .await
        .map_err(|e| AppError::Auth(format!("keyring task failed: {e}")))?;
        result
    }

    async fn clear(&self) -> Result<()> {
        let service = self.service.clone();
        let user = self.user.clone();
        let result: Result<()> = tauri::async_runtime::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &user)
                .map_err(|e| AppError::Auth(format!("keyring entry: {e}")))?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(AppError::Auth(format!("keyring clear: {e}"))),
            }
        })
        .await
        .map_err(|e| AppError::Auth(format!("keyring task failed: {e}")))?;
        result
    }
}

/// credential 管理器：持 refresh_lock 实现单飞刷新；valid_credential 按 5 分钟 skew 判断。
pub(crate) struct GithubCredentialManager {
    store: Arc<dyn CredentialStore>,
    auth: Arc<GithubAuthClient>,
    refresh_lock: Mutex<()>,
}

impl GithubCredentialManager {
    pub(crate) fn new(store: Arc<dyn CredentialStore>, auth: Arc<GithubAuthClient>) -> Self {
        Self {
            store,
            auth,
            refresh_lock: Mutex::new(()),
        }
    }

    /// 返回有效 credential；临近过期则在锁内单飞刷新。app client id 不一致直接要求重新授权。
    pub(crate) async fn valid_credential(&self, app_client_id: &str) -> Result<GithubCredential> {
        let _guard = self.refresh_lock.lock().await;
        let current = self
            .store
            .load()
            .await?
            .ok_or_else(|| AppError::ReauthorizationRequired("no credential stored".into()))?;
        if current.app_client_id != app_client_id {
            drop(self.store.clear().await);
            return Err(AppError::ReauthorizationRequired(
                "credential app client id mismatch".into(),
            ));
        }
        if current.refresh_expired() {
            drop(self.store.clear().await);
            return Err(AppError::ReauthorizationRequired(
                "refresh token expired".into(),
            ));
        }
        if !current.needs_refresh() {
            return Ok(current);
        }
        self.refresh_locked(&current).await
    }

    /// 强制刷新：锁内若 keyring generation 已变化则复用新 token，否则无视 access expiry 强制刷新。
    pub(crate) async fn force_refresh(
        &self,
        rejected_generation: Uuid,
        app_client_id: &str,
    ) -> Result<GithubCredential> {
        let _guard = self.refresh_lock.lock().await;
        let current = self
            .store
            .load()
            .await?
            .ok_or_else(|| AppError::ReauthorizationRequired("no credential".into()))?;
        if current.generation != rejected_generation {
            return Ok(current);
        }
        if current.app_client_id != app_client_id || current.refresh_expired() {
            drop(self.store.clear().await);
            return Err(AppError::ReauthorizationRequired(
                "cannot force refresh (app id mismatch or refresh expired)".into(),
            ));
        }
        self.refresh_locked(&current).await
    }

    async fn refresh_locked(&self, current: &GithubCredential) -> Result<GithubCredential> {
        let refreshed = self.auth.refresh(current).await?;
        if let Err(e) = self.store.replace(&refreshed).await {
            drop(self.store.clear().await);
            return Err(AppError::CredentialPersistenceFailed(format!(
                "replace failed: {e}"
            )));
        }
        Ok(refreshed)
    }

    pub(crate) async fn save_initial(&self, credential: &GithubCredential) -> Result<()> {
        self.store.replace(credential).await
    }

    pub(crate) async fn clear(&self) -> Result<()> {
        self.store.clear().await
    }
}

/// 共享 GitHub API 客户端：所有生产 GitHub 请求通过它发送。首次 401 调用 force_refresh
/// 并单次重放；第二次 401 best-effort clear 并返回 ReauthorizationRequired。
pub(crate) struct GithubAuthenticatedClient {
    manager: Arc<GithubCredentialManager>,
    http: reqwest::Client,
    app_client_id: String,
    api_base_url: reqwest::Url,
}

impl GithubAuthenticatedClient {
    pub(crate) fn new(
        manager: Arc<GithubCredentialManager>,
        app_client_id: String,
    ) -> Result<Self> {
        let api_base_url = reqwest::Url::parse("https://api.github.com/")
            .map_err(|e| AppError::Auth(format!("invalid GitHub API URL: {e}")))?;
        Self::new_with_api_base(manager, app_client_id, api_base_url)
    }

    pub(crate) fn new_with_api_base(
        manager: Arc<GithubCredentialManager>,
        app_client_id: String,
        api_base_url: reqwest::Url,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("skill-sync")
            .build()
            .map_err(|e| AppError::Auth(format!("reqwest client build failed: {e}")))?;
        Ok(Self {
            manager,
            http,
            app_client_id,
            api_base_url,
        })
    }

    pub(crate) async fn get_path(&self, path: &str) -> Result<reqwest::Response> {
        let url = self.resolve_path(path)?;
        self.get(url.as_str()).await
    }

    pub(crate) async fn post_json_path(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<reqwest::Response> {
        let url = self.resolve_path(path)?;
        self.post_json(url.as_str(), body).await
    }

    pub(crate) async fn patch_json_path(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<reqwest::Response> {
        let url = self.resolve_path(path)?;
        self.patch_json(url.as_str(), body).await
    }

    fn resolve_path(&self, path: &str) -> Result<reqwest::Url> {
        self.api_base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| AppError::Auth(format!("invalid GitHub API path: {e}")))
    }

    pub(crate) async fn get(&self, url: &str) -> Result<reqwest::Response> {
        let cred = self.manager.valid_credential(&self.app_client_id).await?;
        let resp = self
            .http
            .get(url)
            .bearer_auth(cred.access_token.expose_secret())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(map_http_err)?;
        if resp.status().as_u16() == 401 {
            return self.retry_get(url, cred.generation).await;
        }
        Ok(resp)
    }

    async fn retry_get(&self, url: &str, rejected: Uuid) -> Result<reqwest::Response> {
        let cred = self
            .manager
            .force_refresh(rejected, &self.app_client_id)
            .await?;
        if cred.generation == rejected {
            drop(self.manager.clear().await);
            return Err(AppError::ReauthorizationRequired(
                "refresh did not produce new generation".into(),
            ));
        }
        let resp = self
            .http
            .get(url)
            .bearer_auth(cred.access_token.expose_secret())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(map_http_err)?;
        if resp.status().as_u16() == 401 {
            drop(self.manager.clear().await);
            return Err(AppError::ReauthorizationRequired("second 401".into()));
        }
        Ok(resp)
    }

    /// PUT JSON。写请求只有明确 401（GitHub 未执行）才重放一次；409/422/网络错误不重试。
    pub(crate) async fn put_json(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<reqwest::Response> {
        let cred = self.manager.valid_credential(&self.app_client_id).await?;
        let resp = self
            .http
            .put(url)
            .bearer_auth(cred.access_token.expose_secret())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&body)
            .send()
            .await
            .map_err(map_http_err)?;
        if resp.status().as_u16() == 401 {
            let cred2 = self
                .manager
                .force_refresh(cred.generation, &self.app_client_id)
                .await?;
            if cred2.generation == cred.generation {
                drop(self.manager.clear().await);
                return Err(AppError::ReauthorizationRequired(
                    "refresh did not produce new generation".into(),
                ));
            }
            let resp2 = self
                .http
                .put(url)
                .bearer_auth(cred2.access_token.expose_secret())
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .json(&body)
                .send()
                .await
                .map_err(map_http_err)?;
            if resp2.status().as_u16() == 401 {
                drop(self.manager.clear().await);
                return Err(AppError::ReauthorizationRequired(
                    "second 401 on put".into(),
                ));
            }
            return Ok(resp2);
        }
        Ok(resp)
    }

    /// POST JSON（create blob/tree/commit）。与 put_json 相同的 401 重放逻辑。
    pub(crate) async fn post_json(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<reqwest::Response> {
        let cred = self.manager.valid_credential(&self.app_client_id).await?;
        let resp = self
            .http
            .post(url)
            .bearer_auth(cred.access_token.expose_secret())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&body)
            .send()
            .await
            .map_err(map_http_err)?;
        if resp.status().as_u16() == 401 {
            let cred2 = self
                .manager
                .force_refresh(cred.generation, &self.app_client_id)
                .await?;
            if cred2.generation == cred.generation {
                drop(self.manager.clear().await);
                return Err(AppError::ReauthorizationRequired(
                    "refresh did not produce new generation".into(),
                ));
            }
            let resp2 = self
                .http
                .post(url)
                .bearer_auth(cred2.access_token.expose_secret())
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .json(&body)
                .send()
                .await
                .map_err(map_http_err)?;
            if resp2.status().as_u16() == 401 {
                drop(self.manager.clear().await);
                return Err(AppError::ReauthorizationRequired(
                    "second 401 on post".into(),
                ));
            }
            return Ok(resp2);
        }
        Ok(resp)
    }

    /// PATCH JSON（update ref）。与 put_json 相同的 401 重放逻辑。
    pub(crate) async fn patch_json(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<reqwest::Response> {
        let cred = self.manager.valid_credential(&self.app_client_id).await?;
        let resp = self
            .http
            .patch(url)
            .bearer_auth(cred.access_token.expose_secret())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&body)
            .send()
            .await
            .map_err(map_http_err)?;
        if resp.status().as_u16() == 401 {
            let cred2 = self
                .manager
                .force_refresh(cred.generation, &self.app_client_id)
                .await?;
            if cred2.generation == cred.generation {
                drop(self.manager.clear().await);
                return Err(AppError::ReauthorizationRequired(
                    "refresh did not produce new generation".into(),
                ));
            }
            let resp2 = self
                .http
                .patch(url)
                .bearer_auth(cred2.access_token.expose_secret())
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .json(&body)
                .send()
                .await
                .map_err(map_http_err)?;
            if resp2.status().as_u16() == 401 {
                drop(self.manager.clear().await);
                return Err(AppError::ReauthorizationRequired(
                    "second 401 on patch".into(),
                ));
            }
            return Ok(resp2);
        }
        Ok(resp)
    }
}

fn map_http_err(e: reqwest::Error) -> AppError {
    AppError::Auth(format!("http error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github_app_config::GithubAppPublicConfig;
    use crate::github_auth::InternalPollResult;
    use secrecy::SecretString;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn fresh_credential(login: &str, app: &str, access_age_secs: i64) -> GithubCredential {
        let now = Utc::now();
        GithubCredential {
            schema: SCHEMA,
            generation: Uuid::new_v4(),
            access_token: SecretString::new("tok".into()),
            refresh_token: SecretString::new("ref".into()),
            access_expires_at: now + Duration::seconds(access_age_secs),
            refresh_expires_at: now + Duration::days(365),
            github_login: login.into(),
            app_client_id: app.into(),
        }
    }

    fn auth_client(server: &MockServer) -> Arc<GithubAuthClient> {
        let web = reqwest::Url::parse(&server.uri()).unwrap();
        let api = reqwest::Url::parse(&format!("{}/api/", server.uri())).unwrap();
        Arc::new(
            GithubAuthClient::new_with_urls(
                GithubAppPublicConfig::new("Iv1.test", "skill-sync").unwrap(),
                web,
                api,
            )
            .unwrap(),
        )
    }

    #[tokio::test]
    async fn valid_credential_reuses_non_expired() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let server = MockServer::start().await;
        let manager = GithubCredentialManager::new(store.clone(), auth_client(&server));
        let cred = fresh_credential("octocat", "Iv1.test", 3600);
        manager.save_initial(&cred).await.unwrap();
        let got = manager.valid_credential("Iv1.test").await.unwrap();
        assert_eq!(got.generation, cred.generation);
    }

    #[tokio::test]
    async fn app_client_id_mismatch_requires_reauth() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let server = MockServer::start().await;
        let manager = GithubCredentialManager::new(store.clone(), auth_client(&server));
        let cred = fresh_credential("octocat", "Iv1.old", 3600);
        manager.save_initial(&cred).await.unwrap();
        let err = manager.valid_credential("Iv1.test").await.unwrap_err();
        assert!(matches!(err, AppError::ReauthorizationRequired(_)));
        assert!(store.load().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn refresh_expired_requires_reauth() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let server = MockServer::start().await;
        let manager = GithubCredentialManager::new(store.clone(), auth_client(&server));
        let mut cred = fresh_credential("octocat", "Iv1.test", -3600);
        cred.refresh_expires_at = Utc::now() - Duration::days(1);
        manager.save_initial(&cred).await.unwrap();
        let err = manager.valid_credential("Iv1.test").await.unwrap_err();
        assert!(matches!(err, AppError::ReauthorizationRequired(_)));
    }

    #[tokio::test]
    async fn near_expiry_single_flight_refresh() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let server = MockServer::start().await;
        // refresh endpoint 返回新 token
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "tok2", "refresh_token": "ref2",
                "expires_in": 28800, "refresh_token_expires_in": 15897600
            })))
            .mount(&server)
            .await;
        let manager = Arc::new(GithubCredentialManager::new(
            store.clone(),
            auth_client(&server),
        ));
        let cred = fresh_credential("octocat", "Iv1.test", -10); // 已过期 -> 需刷新
        manager.save_initial(&cred).await.unwrap();

        // 20 个并发 caller 共享一次 refresh
        let mut handles = Vec::new();
        for _ in 0..20 {
            let m = manager.clone();
            handles.push(tokio::spawn(async move {
                m.valid_credential("Iv1.test").await.unwrap()
            }));
        }
        let mut gens = Vec::new();
        for h in handles {
            gens.push(h.await.unwrap().generation);
        }
        assert!(gens.iter().all(|g| *g == gens[0]));
        assert_ne!(gens[0], cred.generation);
        assert_eq!(
            store
                .load()
                .await
                .unwrap()
                .unwrap()
                .access_token
                .expose_secret(),
            "tok2"
        );
    }

    #[tokio::test]
    async fn http_refresh_failure_preserves_old_credential() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let server = MockServer::start().await;
        // refresh 返回错误
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "bad_refresh_token"
            })))
            .mount(&server)
            .await;
        let manager = GithubCredentialManager::new(store.clone(), auth_client(&server));
        let cred = fresh_credential("octocat", "Iv1.test", -10);
        manager.save_initial(&cred).await.unwrap();
        let err = manager.valid_credential("Iv1.test").await.unwrap_err();
        assert!(matches!(err, AppError::ReauthorizationRequired(_)));
        // 旧 credential 仍在
        assert!(store.load().await.unwrap().is_some());
    }

    #[tokio::test]
    async fn force_refresh_reuses_if_generation_changed() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let server = MockServer::start().await;
        let manager = GithubCredentialManager::new(store.clone(), auth_client(&server));
        let cred = fresh_credential("octocat", "Iv1.test", 3600);
        manager.save_initial(&cred).await.unwrap();
        // 用过期的 rejected_generation -> load 当前（generation 不同）直接复用
        let got = manager
            .force_refresh(Uuid::new_v4(), "Iv1.test")
            .await
            .unwrap();
        assert_eq!(got.generation, cred.generation);
    }

    #[tokio::test]
    async fn authenticated_client_retries_on_401() {
        let store = Arc::new(InMemoryCredentialStore::new());
        let server = MockServer::start().await;
        // refresh endpoint
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "tok2", "refresh_token": "ref2",
                "expires_in": 28800, "refresh_token_expires_in": 15897600
            })))
            .mount(&server)
            .await;
        // GET /api/resource 第一次 401，第二次 200
        Mock::given(method("GET"))
            .and(path("/api/resource"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/resource"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let manager = Arc::new(GithubCredentialManager::new(
            store.clone(),
            auth_client(&server),
        ));
        let cred = fresh_credential("octocat", "Iv1.test", 3600);
        manager.save_initial(&cred).await.unwrap();
        let client = GithubAuthenticatedClient::new(manager, "Iv1.test".into()).unwrap();
        let resp = client
            .get(&format!("{}/api/resource", server.uri()))
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        // 刷新后 credential generation 已变
        assert_ne!(
            store.load().await.unwrap().unwrap().generation,
            cred.generation
        );
    }

    #[tokio::test]
    async fn credential_debug_does_not_expose_token() {
        let mut cred = fresh_credential("octocat", "Iv1.test", 3600);
        cred.access_token = SecretString::new("ACCESS-SECRET-VALUE".into());
        cred.refresh_token = SecretString::new("REFRESH-SECRET-VALUE".into());
        let debug = format!("{cred:?}");
        assert!(!debug.contains("ACCESS-SECRET-VALUE"));
        assert!(!debug.contains("REFRESH-SECRET-VALUE"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn internal_poll_result_debug() {
        // 确保 InternalPollResult 可 Debug（不含 token 在 Pending/Denied 分支）
        let pending = InternalPollResult::Pending { interval: 5 };
        let debug = format!("{pending:?}");
        assert!(debug.contains("Pending"));
    }
}
