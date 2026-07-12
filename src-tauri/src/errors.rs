use serde::Serialize;
use serde::Serializer;

#[derive(Debug, Serialize)]
pub(crate) struct AppErrorPayload {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_check: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AppError {
    #[error("io error: {0}")]
    Io(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("skill error: {0}")]
    Skill(String),
    #[error("not configured: {0}")]
    NotConfigured(String),
    #[error("vault error: {0}")]
    Vault(String),
    // RemoteChanged / RemoteOutcomeUnknown / Auth / RecoveryPending 在 Task 6/9/10 才被构造，
    // Task 3 阶段暂未使用，故标注 allow(dead_code)；Vault / Blocked 本任务即使用。
    #[allow(dead_code)]
    #[error("remote changed: {0}")]
    RemoteChanged(String),
    #[allow(dead_code)]
    #[error("remote outcome unknown (base={base_commit_sha}, candidate={candidate_commit_sha})")]
    RemoteOutcomeUnknown {
        base_commit_sha: String,
        candidate_commit_sha: String,
    },
    #[allow(dead_code)]
    #[error("auth error: {0}")]
    Auth(String),
    #[error("blocked: {0}")]
    Blocked(String),
    #[allow(dead_code)]
    #[error("recovery pending: {0}")]
    RecoveryPending(String),
    #[allow(dead_code)]
    #[error("credential persistence failed: {0}")]
    CredentialPersistenceFailed(String),
    #[allow(dead_code)]
    #[error("reauthorization required: {0}")]
    ReauthorizationRequired(String),
    #[allow(dead_code)]
    #[error("rate limited: retry after {retry_after:?}")]
    RateLimited { retry_after: Option<String> },
    #[allow(dead_code)]
    #[error("vault state changed: {0}")]
    VaultStateChanged(String),
    #[allow(dead_code)]
    #[error("vault state changed: {message}")]
    VaultStateChangedWithCheck {
        message: String,
        latest_check: serde_json::Value,
    },
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Config(_) => "config",
            AppError::Skill(_) => "skill",
            AppError::NotConfigured(_) => "not_configured",
            AppError::Vault(_) => "vault",
            AppError::RemoteChanged(_) => "remote_changed",
            AppError::RemoteOutcomeUnknown { .. } => "remote_outcome_unknown",
            AppError::Auth(_) => "auth",
            AppError::Blocked(_) => "blocked",
            AppError::RecoveryPending(_) => "recovery_pending",
            AppError::CredentialPersistenceFailed(_) => "credential_persistence_failed",
            AppError::ReauthorizationRequired(_) => "reauthorization_required",
            AppError::RateLimited { .. } => "rate_limited",
            AppError::VaultStateChanged(_) => "vault_state_changed",
            AppError::VaultStateChangedWithCheck { .. } => "vault_state_changed",
            AppError::Other(_) => "other",
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(e: serde_yaml::Error) -> Self {
        AppError::Config(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let payload = AppErrorPayload {
            kind: self.kind(),
            message: redact_sensitive(&self.to_string()),
            retry_after: match self {
                AppError::RateLimited { retry_after } => retry_after.clone(),
                _ => None,
            },
            latest_check: match self {
                AppError::VaultStateChangedWithCheck { latest_check, .. } => {
                    Some(latest_check.clone())
                }
                _ => None,
            },
        };
        payload.serialize(s)
    }
}

pub(crate) fn redact_sensitive(message: &str) -> String {
    let mut redacted = message.to_string();
    for marker in [
        "access_token=",
        "refresh_token=",
        "access_token:",
        "refresh_token:",
        "device_code=",
        "user_code=",
        "device_code:",
        "user_code:",
        "\"access_token\":\"",
        "\"refresh_token\":\"",
        "\"device_code\":\"",
        "\"user_code\":\"",
        "token=",
        "token:",
        "client_secret=",
        "private_key=",
        "Bearer ",
    ] {
        let mut offset = 0;
        while let Some(found) = redacted[offset..].find(marker) {
            let start = offset + found + marker.len();
            let end = redacted[start..]
                .find(|c: char| c.is_whitespace() || c == ',' || c == '}' || c == '"')
                .map(|value| start + value)
                .unwrap_or(redacted.len());
            redacted.replace_range(start..end, "[REDACTED]");
            offset = start + "[REDACTED]".len();
        }
    }
    redacted
}

pub(crate) type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_error_redacts_token_values_and_preserves_conditional_fields() {
        let error = AppError::Auth("access_token=secret refresh_token=refresh".into());
        let value = serde_json::to_value(error).unwrap();
        assert!(!value.to_string().contains("secret"));
        assert!(!value.to_string().contains("refresh refresh"));
        assert!(value.get("retry_after").is_none());

        let rate_limited = AppError::RateLimited {
            retry_after: Some("12".into()),
        };
        let value = serde_json::to_value(rate_limited).unwrap();
        assert_eq!(value["retry_after"], "12");
        assert!(value.get("latest_check").is_none());

        let changed = AppError::VaultStateChangedWithCheck {
            message: "stale".into(),
            latest_check: serde_json::json!({ "status": "ready" }),
        };
        let value = serde_json::to_value(changed).unwrap();
        assert_eq!(value["latest_check"]["status"], "ready");
        assert!(value.get("retry_after").is_none());
    }
}
