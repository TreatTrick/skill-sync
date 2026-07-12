// GitHub App Device Flow 授权与 refresh。不发送 client secret / OAuth scope。
// 内部 token 用 secrecy::SecretString，禁止 Debug 输出明文。
#![allow(dead_code)]

use chrono::{Duration, Utc};
use secrecy::{ExposeSecret, SecretString};
use uuid::Uuid;

use crate::errors::{AppError, Result};
use crate::github_app_config::GithubAppPublicConfig;
use crate::github_credentials::GithubCredential;

const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
const REFRESH_GRANT_TYPE: &str = "refresh_token";

/// Device Flow 启动响应（公共，不含任何 token）。
#[derive(Debug, Clone)]
pub(crate) struct DeviceFlowStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// poll 内部结果：包含 token 的成功分支用 SecretString，不对外暴露明文。
#[derive(Debug)]
pub(crate) enum InternalPollResult {
    Pending {
        interval: u64,
    },
    SlowDown {
        interval: u64,
    },
    Success {
        access_token: SecretString,
        refresh_token: SecretString,
        access_expires_in: u64,
        refresh_token_expires_in: u64,
    },
    Denied,
}

pub(crate) struct GithubAuthClient {
    client: reqwest::Client,
    public_config: GithubAppPublicConfig,
    web_base_url: reqwest::Url,
    api_base_url: reqwest::Url,
}

impl GithubAuthClient {
    pub(crate) fn new(public_config: GithubAppPublicConfig) -> Result<Self> {
        Self::new_with_urls(
            public_config,
            reqwest::Url::parse("https://github.com/")
                .map_err(|e| AppError::Auth(format!("invalid url: {e}")))?,
            reqwest::Url::parse("https://api.github.com/")
                .map_err(|e| AppError::Auth(format!("invalid url: {e}")))?,
        )
    }

    pub(crate) fn new_with_urls(
        public_config: GithubAppPublicConfig,
        web_base_url: reqwest::Url,
        api_base_url: reqwest::Url,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent("skill-sync")
            .build()
            .map_err(|e| AppError::Auth(format!("reqwest client build failed: {e}")))?;
        Ok(Self {
            client,
            public_config,
            web_base_url,
            api_base_url,
        })
    }

    pub(crate) fn public_config(&self) -> &GithubAppPublicConfig {
        &self.public_config
    }

    /// 启动 Device Flow：只发送 client_id，不发送 scope。
    pub(crate) async fn start(&self) -> Result<DeviceFlowStart> {
        let url = self
            .web_base_url
            .join("login/device/code")
            .map_err(|e| AppError::Auth(format!("invalid url: {e}")))?;
        let resp = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .form(&[("client_id", self.public_config.client_id.as_str())])
            .send()
            .await
            .map_err(map_http_err)?;
        let body: serde_json::Value = decode_json(resp).await?;
        Ok(DeviceFlowStart {
            device_code: get_str(&body, "device_code")?,
            user_code: get_str(&body, "user_code")?,
            verification_uri: get_str(&body, "verification_uri")?,
            expires_in: get_u64(&body, "expires_in").unwrap_or(900),
            interval: get_u64(&body, "interval").unwrap_or(5),
        })
    }

    /// 轮询 token：发送 client_id、device_code、device grant_type。
    pub(crate) async fn poll(&self, device_code: &str) -> Result<InternalPollResult> {
        let url = self
            .web_base_url
            .join("login/oauth/access_token")
            .map_err(|e| AppError::Auth(format!("invalid url: {e}")))?;
        let resp = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.public_config.client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", DEVICE_GRANT_TYPE),
            ])
            .send()
            .await
            .map_err(map_http_err)?;
        let body: serde_json::Value = decode_json(resp).await?;
        if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
            return Ok(match err {
                "authorization_pending" => InternalPollResult::Pending {
                    interval: get_u64(&body, "interval").unwrap_or(5),
                },
                "slow_down" => InternalPollResult::SlowDown {
                    interval: get_u64(&body, "interval").unwrap_or(5) + 5,
                },
                "expired_token"
                | "access_denied"
                | "incorrect_device_code"
                | "device_flow_disabled" => InternalPollResult::Denied,
                other => {
                    return Err(AppError::Auth(format!("device flow error: {other}")));
                }
            });
        }
        let access_token = SecretString::new(get_str(&body, "access_token")?.into());
        let refresh_token = SecretString::new(get_str(&body, "refresh_token")?.into());
        Ok(InternalPollResult::Success {
            access_token,
            refresh_token,
            access_expires_in: get_u64(&body, "expires_in").unwrap_or(28800),
            refresh_token_expires_in: get_u64(&body, "refresh_token_expires_in")
                .unwrap_or(15897600),
        })
    }

    /// 首次 token 成功后：用 access token 调用 /user 取得 login，构造完整 credential。
    pub(crate) async fn build_credential(
        &self,
        access_token: SecretString,
        refresh_token: SecretString,
        access_expires_in: u64,
        refresh_token_expires_in: u64,
    ) -> Result<GithubCredential> {
        let login = self.fetch_login(&access_token).await?;
        let now = Utc::now();
        Ok(GithubCredential {
            schema: 1,
            generation: Uuid::new_v4(),
            access_token,
            refresh_token,
            access_expires_at: now + Duration::seconds(access_expires_in as i64),
            refresh_expires_at: now + Duration::seconds(refresh_token_expires_in as i64),
            github_login: login,
            app_client_id: self.public_config.client_id.clone(),
        })
    }

    /// refresh：只发送 client_id、grant_type=refresh_token、current refresh token。
    /// 沿用已验证 login；不发送 client secret。
    pub(crate) async fn refresh(&self, current: &GithubCredential) -> Result<GithubCredential> {
        let url = self
            .web_base_url
            .join("login/oauth/access_token")
            .map_err(|e| AppError::Auth(format!("invalid url: {e}")))?;
        let resp = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.public_config.client_id.as_str()),
                ("grant_type", REFRESH_GRANT_TYPE),
                ("refresh_token", current.refresh_token.expose_secret()),
            ])
            .send()
            .await
            .map_err(map_http_err)?;
        let body: serde_json::Value = decode_json(resp).await?;
        if body.get("error").is_some() {
            return Err(AppError::ReauthorizationRequired(format!(
                "refresh rejected: {}",
                body.get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            )));
        }
        let access_token = SecretString::new(get_str(&body, "access_token")?.into());
        let refresh_token = SecretString::new(get_str(&body, "refresh_token")?.into());
        let now = Utc::now();
        Ok(GithubCredential {
            schema: 1,
            generation: Uuid::new_v4(),
            access_token,
            refresh_token,
            access_expires_at: now
                + Duration::seconds(get_u64(&body, "expires_in").unwrap_or(28800) as i64),
            refresh_expires_at: now
                + Duration::seconds(
                    get_u64(&body, "refresh_token_expires_in").unwrap_or(15897600) as i64,
                ),
            github_login: current.github_login.clone(),
            app_client_id: self.public_config.client_id.clone(),
        })
    }

    async fn fetch_login(&self, access_token: &SecretString) -> Result<String> {
        let url = self
            .api_base_url
            .join("user")
            .map_err(|e| AppError::Auth(format!("invalid url: {e}")))?;
        let resp = self
            .client
            .get(url)
            .header(
                "Authorization",
                format!("Bearer {}", access_token.expose_secret()),
            )
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(map_http_err)?;
        if !resp.status().is_success() {
            return Err(AppError::Auth(format!(
                "fetch /user failed: status {}",
                resp.status()
            )));
        }
        let body: serde_json::Value = decode_json(resp).await?;
        body.get("login")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Auth("no login in /user response".into()))
    }
}

fn map_http_err(e: reqwest::Error) -> AppError {
    AppError::Auth(format!("http error: {e}"))
}

async fn decode_json(resp: reqwest::Response) -> Result<serde_json::Value> {
    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Auth(format!("decode json failed (status {status}): {e}")))?;
    Ok(body)
}

fn get_str(body: &serde_json::Value, key: &str) -> Result<String> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Auth(format!("missing string field {key}")))
}

fn get_u64(body: &serde_json::Value, key: &str) -> Option<u64> {
    body.get(key).and_then(|v| v.as_u64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github_app_config::GithubAppPublicConfig;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn auth_client(server: &MockServer) -> GithubAuthClient {
        let web = reqwest::Url::parse(&server.uri()).unwrap();
        let api = reqwest::Url::parse(&format!("{}/api/", server.uri())).unwrap();
        GithubAuthClient::new_with_urls(
            GithubAppPublicConfig::new("Iv1.test", "skill-sync").unwrap(),
            web,
            api,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn start_sends_client_id_without_scope() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dc",
                "user_code": "uc",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 5,
            })))
            .mount(&server)
            .await;
        let client = auth_client(&server);
        let start = client.start().await.unwrap();
        assert_eq!(start.device_code, "dc");
        assert_eq!(start.user_code, "uc");
        assert_eq!(start.interval, 5);
    }

    #[tokio::test]
    async fn poll_pending_slowdown_denied_and_success() {
        let server = MockServer::start().await;
        // pending
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "authorization_pending", "interval": 5
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // success
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "tok", "refresh_token": "ref",
                "expires_in": 28800, "refresh_token_expires_in": 15897600
            })))
            .mount(&server)
            .await;
        let client = auth_client(&server);
        match client.poll("dc").await.unwrap() {
            InternalPollResult::Pending { interval } => assert_eq!(interval, 5),
            _ => panic!("expected pending"),
        }
        match client.poll("dc").await.unwrap() {
            InternalPollResult::Success { access_token, .. } => {
                assert_eq!(access_token.expose_secret(), "tok");
            }
            _ => panic!("expected success"),
        }
    }

    #[tokio::test]
    async fn poll_denied_on_access_denied() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "access_denied"
            })))
            .mount(&server)
            .await;
        let client = auth_client(&server);
        match client.poll("dc").await.unwrap() {
            InternalPollResult::Denied => {}
            _ => panic!("expected denied"),
        }
    }

    #[tokio::test]
    async fn build_credential_fetches_login() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "login": "octocat"
            })))
            .mount(&server)
            .await;
        let client = auth_client(&server);
        let cred = client
            .build_credential(
                SecretString::new("tok".into()),
                SecretString::new("ref".into()),
                28800,
                15897600,
            )
            .await
            .unwrap();
        assert_eq!(cred.github_login, "octocat");
        assert_eq!(cred.access_token.expose_secret(), "tok");
        assert_eq!(cred.app_client_id, "Iv1.test");
    }

    #[tokio::test]
    async fn refresh_keeps_login_and_returns_new_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "tok2", "refresh_token": "ref2",
                "expires_in": 28800, "refresh_token_expires_in": 15897600
            })))
            .mount(&server)
            .await;
        let client = auth_client(&server);
        let current = GithubCredential {
            schema: 1,
            generation: Uuid::nil(),
            access_token: SecretString::new("tok".into()),
            refresh_token: SecretString::new("ref".into()),
            access_expires_at: Utc::now(),
            refresh_expires_at: Utc::now() + Duration::days(1),
            github_login: "octocat".into(),
            app_client_id: "Iv1.test".into(),
        };
        let refreshed = client.refresh(&current).await.unwrap();
        assert_eq!(refreshed.access_token.expose_secret(), "tok2");
        assert_eq!(refreshed.github_login, "octocat");
        assert_ne!(refreshed.generation, current.generation);
    }

    #[tokio::test]
    async fn refresh_rejects_returns_reauthorization_required() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "bad_refresh_token"
            })))
            .mount(&server)
            .await;
        let client = auth_client(&server);
        let current = GithubCredential {
            schema: 1,
            generation: Uuid::nil(),
            access_token: SecretString::new("tok".into()),
            refresh_token: SecretString::new("ref".into()),
            access_expires_at: Utc::now(),
            refresh_expires_at: Utc::now() + Duration::days(1),
            github_login: "octocat".into(),
            app_client_id: "Iv1.test".into(),
        };
        let err = client.refresh(&current).await.unwrap_err();
        assert!(matches!(err, AppError::ReauthorizationRequired(_)));
    }
}
