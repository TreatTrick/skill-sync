use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

const CONFIG_VERSION: u32 = 1;
const CONFIG_DIR_NAME: &str = "skill-sync";
const CONFIG_FILE_NAME: &str = "config.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub repository: RepositoryConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    pub hosts: HostsConfig,
    #[serde(default)]
    pub custom_paths: Vec<String>,
    #[serde(default = "default_ignore")]
    pub ignore: Vec<String>,
}

fn default_version() -> u32 {
    CONFIG_VERSION
}

fn default_ignore() -> Vec<String> {
    vec![
        "**/.git/**".into(),
        "**/node_modules/**".into(),
        "**/.env".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepositoryConfig {
    #[serde(default)]
    pub local_path: String,
    #[serde(default)]
    pub remote: String,
    #[serde(default = "default_branch")]
    pub branch: String,
}

fn default_branch() -> String {
    "main".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DefaultsConfig {
    #[serde(default = "default_true")]
    pub backup: bool,
    #[serde(default = "default_install_mode")]
    pub install_mode: String,
}

fn default_true() -> bool {
    true
}

fn default_install_mode() -> String {
    "copy".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostsConfig {
    pub codex: HostConfig,
    pub claude: HostConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub paths: Vec<String>,
}

impl AppConfig {
    pub fn default_config() -> Self {
        Self {
            version: CONFIG_VERSION,
            repository: RepositoryConfig {
                local_path: String::new(),
                remote: String::new(),
                branch: "main".into(),
            },
            defaults: DefaultsConfig {
                backup: true,
                install_mode: "copy".into(),
            },
            hosts: HostsConfig {
                codex: HostConfig {
                    enabled: true,
                    paths: vec!["~/.codex/skills".into(), "~/.agents/skills".into()],
                },
                claude: HostConfig {
                    enabled: true,
                    paths: vec!["~/.claude/skills".into()],
                },
            },
            custom_paths: vec![],
            ignore: default_ignore(),
        }
    }

    pub fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join(CONFIG_DIR_NAME))
    }

    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join(CONFIG_FILE_NAME))
    }

    pub fn load() -> Result<Self> {
        let path =
            Self::config_path().ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
        if !path.exists() {
            return Ok(Self::default_config());
        }
        let text = fs::read_to_string(&path)?;
        let cfg: AppConfig = serde_yaml::from_str(&text)?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()
            .ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
        fs::create_dir_all(&dir)?;
        let path = Self::config_path().unwrap();
        let text = serde_yaml::to_string(self)?;
        fs::write(&path, text)?;
        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        !self.repository.local_path.trim().is_empty()
    }

    /// Returns `(host_name, host_config)` for each enabled host (codex + claude only).
    pub fn enabled_hosts(&self) -> Vec<(&'static str, &HostConfig)> {
        let mut hosts = Vec::new();
        if self.hosts.codex.enabled {
            hosts.push(("codex", &self.hosts.codex));
        }
        if self.hosts.claude.enabled {
            hosts.push(("claude", &self.hosts.claude));
        }
        hosts
    }
}

pub fn expand_path(p: &str) -> Result<PathBuf> {
    let trimmed = p.trim();
    if trimmed == "~" {
        return dirs::home_dir().ok_or_else(|| AppError::Config("cannot determine home dir".into()));
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = dirs::home_dir().ok_or_else(|| AppError::Config("cannot determine home dir".into()))?;
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_codex_and_claude() {
        let cfg = AppConfig::default_config();
        assert!(cfg.hosts.codex.enabled);
        assert!(cfg.hosts.claude.enabled);
        assert!(cfg.hosts.codex.paths.iter().any(|p| p.contains(".codex")));
        assert!(cfg.hosts.claude.paths.iter().any(|p| p.contains(".claude")));
    }

    #[test]
    fn roundtrip_serialization() {
        let cfg = AppConfig::default_config();
        let text = serde_yaml::to_string(&cfg).unwrap();
        let back: AppConfig = serde_yaml::from_str(&text).unwrap();
        assert_eq!(back.hosts.codex.paths, cfg.hosts.codex.paths);
        assert_eq!(back.hosts.claude.paths, cfg.hosts.claude.paths);
        assert!(back.is_configured() == cfg.is_configured());
    }

    #[test]
    fn expand_tilde_uses_home() {
        let expanded = expand_path("~/foo/bar").unwrap();
        assert!(!expanded.starts_with("~"));
        assert!(expanded.ends_with("foo/bar"));
    }

    #[test]
    fn expand_plain_path_unchanged() {
        let expanded = expand_path("D:/agent-skills").unwrap();
        assert_eq!(expanded.to_string_lossy(), "D:/agent-skills");
    }
}
