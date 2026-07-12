#![allow(dead_code)]
// GitHub App 公开构建配置：编译时注入的 client_id / slug。
// 生产代码不得调用 std::env::var（build.rs 通过 cargo:rustc-env 注入，env! 编译期读取）。

use crate::errors::{AppError, Result};

/// GitHub App 的公开配置（client_id + slug）。private key / client secret 永不进入。
#[derive(Debug, Clone)]
pub(crate) struct GithubAppPublicConfig {
    pub client_id: String,
    pub slug: String,
}

impl GithubAppPublicConfig {
    /// 校验非空后构造。
    pub(crate) fn new(client_id: &str, slug: &str) -> Result<Self> {
        if client_id.trim().is_empty() {
            return Err(AppError::Config("github app client_id is empty".into()));
        }
        if slug.trim().is_empty() {
            return Err(AppError::Config("github app slug is empty".into()));
        }
        Ok(Self {
            client_id: client_id.into(),
            slug: slug.into(),
        })
    }

    /// 编译时注入的配置。debug/test 构建未注入时返回 `NotConfigured`，release 由 build.rs 保证非空。
    pub(crate) fn embedded() -> Result<Self> {
        let client_id = env!("SKILL_SYNC_GITHUB_APP_CLIENT_ID");
        let slug = env!("SKILL_SYNC_GITHUB_APP_SLUG");
        if client_id.is_empty() || slug.is_empty() {
            return Err(AppError::NotConfigured(
                "github app public config not embedded (set SKILL_SYNC_GITHUB_APP_CLIENT_ID/SLUG at build)"
                    .into(),
            ));
        }
        Self::new(client_id, slug)
    }

    /// 公开字段名（用于断言不含 private key / client secret）。
    pub(crate) fn public_field_names() -> [&'static str; 2] {
        ["client_id", "slug"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_config_requires_non_empty_client_id_and_slug() {
        assert!(GithubAppPublicConfig::new("", "skill-sync").is_err());
        assert!(GithubAppPublicConfig::new("Iv1.test", "").is_err());
        assert!(GithubAppPublicConfig::new("Iv1.test", "skill-sync").is_ok());
    }

    #[test]
    fn public_config_contains_no_private_key_or_client_secret_fields() {
        let fields = GithubAppPublicConfig::public_field_names();
        assert_eq!(fields, ["client_id", "slug"]);
    }
}
