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
