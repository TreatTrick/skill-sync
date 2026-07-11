use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub(crate) enum AppError {
    #[error("io error: {0}")]
    Io(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("git error: {0}")]
    Git(String),
    #[error("skill error: {0}")]
    Skill(String),
    #[error("sync error: {0}")]
    Sync(String),
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
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Config(_) => "config",
            AppError::Git(_) => "git",
            AppError::Skill(_) => "skill",
            AppError::Sync(_) => "sync",
            AppError::NotConfigured(_) => "not_configured",
            AppError::Vault(_) => "vault",
            AppError::RemoteChanged(_) => "remote_changed",
            AppError::RemoteOutcomeUnknown { .. } => "remote_outcome_unknown",
            AppError::Auth(_) => "auth",
            AppError::Blocked(_) => "blocked",
            AppError::RecoveryPending(_) => "recovery_pending",
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
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

pub(crate) type Result<T> = std::result::Result<T, AppError>;
