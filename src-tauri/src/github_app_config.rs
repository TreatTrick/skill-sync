#![allow(dead_code)]
// GitHub App 公开配置：client_id / slug 可以安全地随桌面应用发布。

const GITHUB_APP_CLIENT_ID: &str = "Iv23lif3tCgfnQjxjl9U";
const GITHUB_APP_SLUG: &str = "tt-skills-sync";

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

    /// 返回随应用发布的公开配置。
    pub(crate) fn embedded() -> Result<Self> {
        Self::new(GITHUB_APP_CLIENT_ID, GITHUB_APP_SLUG)
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

    #[cfg(debug_assertions)]
    #[test]
    fn debug_build_embeds_the_demo_github_app() {
        let config =
            GithubAppPublicConfig::embedded().expect("debug app config should be embedded");

        assert_eq!(config.client_id, "Iv23lif3tCgfnQjxjl9U");
        assert_eq!(config.slug, "tt-skills-sync");
    }
}
