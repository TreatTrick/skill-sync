use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

const CONFIG_VERSION: u32 = 1;
const CONFIG_DIR_NAME: &str = "skill-sync";
const CONFIG_FILE_NAME: &str = "config.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AppConfig {
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
        "**/.DS_Store".into(),
        "**/cache/**".into(),
        "**/.cache/**".into(),
        "**/tmp/**".into(),
        "**/temp/**".into(),
        "**/*.log".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct RepositoryConfig {
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
pub(crate) struct DefaultsConfig {
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

/// GitHub vault 远端配置：绑定到固定的 installation/repository/branch。
/// 本任务只新增 DTO，不接入 AppConfig（Task 13 原子迁移）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RemoteConfig {
    pub installation_id: u64,
    pub repository_id: u64,
    pub owner: String,
    pub repo: String,
    pub branch: String,
}

/// pack/unpack 四项资源 limit 与删除护栏阈值；作为一个对象传给 packer/unpacker，
/// 下载路径不得使用硬编码的另一组值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LimitsConfig {
    #[serde(default = "default_max_skill_zip_bytes")]
    pub max_skill_zip_bytes: u64,
    #[serde(default = "default_max_skill_files")]
    pub max_skill_files: usize,
    #[serde(default = "default_max_single_file_unpacked_bytes")]
    pub max_single_file_unpacked_bytes: u64,
    #[serde(default = "default_max_skill_unpacked_bytes")]
    pub max_skill_unpacked_bytes: u64,
    #[serde(default = "default_max_auto_delete")]
    pub max_auto_delete: usize,
}

fn default_max_skill_zip_bytes() -> u64 {
    20 * 1024 * 1024
}

fn default_max_skill_files() -> usize {
    2_000
}

fn default_max_single_file_unpacked_bytes() -> u64 {
    50 * 1024 * 1024
}

fn default_max_skill_unpacked_bytes() -> u64 {
    100 * 1024 * 1024
}

fn default_max_auto_delete() -> usize {
    10
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_skill_zip_bytes: default_max_skill_zip_bytes(),
            max_skill_files: default_max_skill_files(),
            max_single_file_unpacked_bytes: default_max_single_file_unpacked_bytes(),
            max_skill_unpacked_bytes: default_max_skill_unpacked_bytes(),
            max_auto_delete: default_max_auto_delete(),
        }
    }
}

impl LimitsConfig {
    /// 拒绝任一 pack/unpack limit 为 0，并要求单文件上限不超过总上限。
    /// `max_auto_delete` 为 0 表示“任何删除都触发护栏”，是合法的严格配置，不拒绝。
    #[allow(dead_code)]
    pub(crate) fn validate(&self) -> Result<()> {
        if self.max_skill_zip_bytes == 0
            || self.max_skill_files == 0
            || self.max_single_file_unpacked_bytes == 0
            || self.max_skill_unpacked_bytes == 0
        {
            return Err(AppError::Config(
                "pack/unpack limits must be non-zero".into(),
            ));
        }
        if self.max_single_file_unpacked_bytes > self.max_skill_unpacked_bytes {
            return Err(AppError::Config(
                "max_single_file_unpacked_bytes must not exceed max_skill_unpacked_bytes".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HostsConfig {
    pub codex: HostConfig,
    pub claude: HostConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HostConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub paths: Vec<String>,
}

impl AppConfig {
    pub(crate) fn default_config() -> Self {
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

    pub(crate) fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join(CONFIG_DIR_NAME))
    }

    pub(crate) fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join(CONFIG_FILE_NAME))
    }

    pub(crate) fn load() -> Result<Self> {
        let path = Self::config_path()
            .ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
        if !path.exists() {
            return Ok(Self::default_config());
        }
        let text = fs::read_to_string(&path)?;
        let cfg: AppConfig = serde_yaml::from_str(&text)?;
        Ok(cfg)
    }

    pub(crate) fn save(&self) -> Result<()> {
        let dir = Self::config_dir()
            .ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
        fs::create_dir_all(&dir)?;
        let path = Self::config_path()
            .ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
        let text = serde_yaml::to_string(self)?;
        fs::write(&path, text)?;
        Ok(())
    }

    pub(crate) fn is_configured(&self) -> bool {
        !self.repository.local_path.trim().is_empty() || !self.repository.remote.trim().is_empty()
    }

    /// Default managed repo path used when the user does not supply a local_path.
    pub(crate) fn default_repo_path() -> Result<PathBuf> {
        let dir = Self::config_dir()
            .ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
        Ok(dir.join("sync-repo"))
    }

    /// Resolve the sync repo path: use the configured local_path, or fall back
    /// to the managed default under the config dir.
    pub(crate) fn resolve_repo_path(&self) -> Result<PathBuf> {
        if !self.repository.local_path.trim().is_empty() {
            return expand_path(&self.repository.local_path);
        }
        Self::default_repo_path()
    }

    /// Returns `(host_name, host_config)` for each enabled host (codex + claude only).
    pub(crate) fn enabled_hosts(&self) -> Vec<(&'static str, &HostConfig)> {
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

pub(crate) fn expand_path(p: &str) -> Result<PathBuf> {
    let trimmed = p.trim();
    if trimmed == "~" {
        return dirs::home_dir()
            .ok_or_else(|| AppError::Config("cannot determine home dir".into()));
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        let home =
            dirs::home_dir().ok_or_else(|| AppError::Config("cannot determine home dir".into()))?;
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

    #[test]
    fn github_remote_config_roundtrip_preserves_stable_identity() {
        let remote = RemoteConfig {
            installation_id: 123456,
            repository_id: 987654,
            owner: "example".into(),
            repo: "agent-skills".into(),
            branch: "main".into(),
        };
        let back: RemoteConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&remote).unwrap()).unwrap();
        assert_eq!(back, remote);
    }

    #[test]
    fn default_limits_include_pack_and_unpack_budgets() {
        let limits = LimitsConfig::default();
        assert_eq!(limits.max_skill_zip_bytes, 20 * 1024 * 1024);
        assert_eq!(limits.max_skill_files, 2_000);
        assert_eq!(limits.max_single_file_unpacked_bytes, 50 * 1024 * 1024);
        assert_eq!(limits.max_skill_unpacked_bytes, 100 * 1024 * 1024);
        assert!(limits.validate().is_ok());
    }
}
