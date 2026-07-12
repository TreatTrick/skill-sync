use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{AppError, Result};
use crate::vault_binding::VaultBindingStore;

const CONFIG_VERSION: u32 = 2;
const CONFIG_DIR_NAME: &str = "skill-sync";
const CONFIG_FILE_NAME: &str = "config.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_ignore")]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub remote: Option<RemoteConfig>,
    #[serde(default)]
    pub limits: LimitsConfig,
    #[serde(default = "default_device_id")]
    pub device_id: String,
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

fn default_device_id() -> String {
    Uuid::new_v4().to_string()
}

/// GitHub vault 远端配置：绑定到固定的 installation/repository/branch。
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

impl AppConfig {
    pub(crate) fn default_config() -> Self {
        Self {
            version: CONFIG_VERSION,
            ignore: default_ignore(),
            remote: None,
            limits: LimitsConfig::default(),
            device_id: default_device_id(),
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
        let dir = path
            .parent()
            .ok_or_else(|| AppError::Config("config path has no parent".into()))?;
        VaultBindingStore::recover_if_needed(dir)?;
        if !path.exists() {
            return Ok(Self::default_config());
        }
        let text = fs::read_to_string(&path)?;
        Self::from_yaml(&text)
    }

    pub(crate) fn save(&self) -> Result<()> {
        let dir = Self::config_dir()
            .ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
        fs::create_dir_all(&dir)?;
        let path = Self::config_path()
            .ok_or_else(|| AppError::Config("cannot determine config dir".into()))?;
        let text = serde_yaml::to_string(self)?;
        durable_replace(&path, text.as_bytes())
    }

    pub(crate) fn is_configured(&self) -> bool {
        self.remote.is_some()
    }

    pub(crate) fn from_yaml(text: &str) -> Result<Self> {
        let config: Self = serde_yaml::from_str(text)?;
        if config.version != CONFIG_VERSION {
            return Err(AppError::Config(format!(
                "unsupported config version: {}",
                config.version
            )));
        }
        config.limits.validate()?;
        Ok(config)
    }

    pub(crate) fn validate_save_candidate(&self, candidate: &Self) -> Result<()> {
        if self.version != candidate.version
            || self.remote != candidate.remote
            || self.device_id != candidate.device_id
        {
            return Err(AppError::Blocked(
                "only ignore and limits may be changed by save_config".into(),
            ));
        }
        candidate.limits.validate()
    }
}

fn durable_replace(target: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::Config("config path has no parent".into()))?;
    fs::create_dir_all(parent)?;
    let temp = target.with_extension("tmp");
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, target)?;
    if let Ok(dir) = fs::File::open(parent) {
        drop(dir.sync_all());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_unvalidated_remote() {
        let cfg = AppConfig::default_config();

        assert_eq!(cfg.remote, None);
        assert!(!serde_yaml::to_string(&cfg).unwrap().contains("repository:"));
    }

    #[test]
    fn save_config_cannot_change_remote_identity() {
        let mut current = AppConfig::default_config();
        current.remote = Some(RemoteConfig {
            installation_id: 1,
            repository_id: 2,
            owner: "owner".into(),
            repo: "repo".into(),
            branch: "main".into(),
        });
        let mut candidate = current.clone();
        candidate.remote.as_mut().unwrap().branch = "other".into();

        assert!(current.validate_save_candidate(&candidate).is_err());
    }

    #[test]
    fn roundtrip_serialization() {
        let cfg = AppConfig::default_config();
        let text = serde_yaml::to_string(&cfg).unwrap();
        let back = AppConfig::from_yaml(&text).unwrap();
        assert_eq!(back.ignore, cfg.ignore);
        assert_eq!(back.limits.max_skill_files, cfg.limits.max_skill_files);
        assert_eq!(back.is_configured(), cfg.is_configured());
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
