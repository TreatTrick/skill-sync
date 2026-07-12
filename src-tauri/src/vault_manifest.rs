// 远端 vault manifest DTO 与校验。Task 8/10 接入前，非测试构建中为 dead code，整模块 allow。
#![allow(dead_code)]

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::errors::{AppError, Result};
use crate::portable_path::validate_component;
use crate::skill::SkillNamespace;
use unicode_normalization::UnicodeNormalization;

const SCHEMA_VERSION: u32 = 1;

/// 远端 vault manifest 中的单个 skill 条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct VaultSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub namespace: SkillNamespace,
    pub folder_name: String,
    pub hash: String,
    pub blob: String,
    pub size: u64,
    pub updated_at: String,
    pub updated_by: String,
}

/// 远端 `manifest.json` 的 DTO，使用 BTreeMap 保证稳定序列化。
/// V1 不实现 tombstone，未来依靠 `schema` 版本升级。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct VaultManifest {
    pub schema: u32,
    pub updated_at: String,
    pub updated_by: String,
    #[serde(deserialize_with = "deserialize_skills_map", default)]
    pub skills: BTreeMap<String, VaultSkill>,
}

impl VaultManifest {
    /// 构造空 manifest（schema 1，无 skill）。
    pub(crate) fn empty(device_id: &str) -> Self {
        Self {
            schema: SCHEMA_VERSION,
            updated_at: String::new(),
            updated_by: device_id.to_string(),
            skills: BTreeMap::new(),
        }
    }

    /// 解析远端 manifest JSON 的唯一入口；任何单项校验失败都使整个 manifest 无效。
    /// adapter 不得直接调用 `serde_json::from_slice::<VaultManifest>` 绕过校验。
    pub(crate) fn parse_validated(bytes: &[u8]) -> Result<Self> {
        let manifest: VaultManifest = serde_json::from_slice(bytes)
            .map_err(|e| AppError::Vault(format!("invalid manifest json: {e}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA_VERSION {
            let schema = self.schema;
            return Err(AppError::Vault(format!(
                "unsupported manifest schema: {schema}"
            )));
        }
        let mut folder_collision: HashSet<(SkillNamespace, String)> = HashSet::new();
        for (key, skill) in &self.skills {
            if key != &skill.id {
                return Err(AppError::Vault(format!(
                    "manifest skill key {key:?} != id {:?}",
                    skill.id
                )));
            }
            validate_skill_entry(skill)?;
            let collision = folder_collision_key(&skill.folder_name);
            if !folder_collision.insert((skill.namespace, collision.clone())) {
                return Err(AppError::Vault(format!(
                    "manifest folder_name collision: {collision:?}"
                )));
            }
        }
        Ok(())
    }
}

/// 校验单个 skill 条目：id 前缀与 namespace 一致、hash/blob/size/folder_name 合法。
fn validate_skill_entry(skill: &VaultSkill) -> Result<()> {
    let namespace_value = namespace_ser_value(skill.namespace);
    let (prefix, name) = skill.id.split_once(':').ok_or_else(|| {
        AppError::Vault(format!(
            "skill id missing namespace separator: {:?}",
            skill.id
        ))
    })?;
    if prefix != namespace_value {
        return Err(AppError::Vault(format!(
            "skill id namespace {prefix:?} != field {namespace_value:?} (id={:?})",
            skill.id
        )));
    }
    if name.is_empty() {
        return Err(AppError::Vault(format!(
            "skill id has empty name: {:?}",
            skill.id
        )));
    }

    let hex = skill.hash.strip_prefix("sha256:").ok_or_else(|| {
        AppError::Vault(format!(
            "invalid hash (no sha256: prefix): {:?}",
            skill.hash
        ))
    })?;
    if hex.len() != 64 || !hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(AppError::Vault(format!(
            "invalid hash (expect sha256:64 lowercase hex): {:?}",
            skill.hash
        )));
    }
    let expected_blob = format!("blobs/sha256/{hex}.skill.zip");
    if skill.blob != expected_blob {
        return Err(AppError::Vault(format!(
            "blob {:?} != expected {expected_blob:?}",
            skill.blob
        )));
    }
    if skill.size == 0 {
        return Err(AppError::Vault(format!(
            "skill size must be > 0: {:?}",
            skill.id
        )));
    }
    validate_component(&skill.folder_name)?;
    Ok(())
}

fn namespace_ser_value(ns: SkillNamespace) -> &'static str {
    match ns {
        SkillNamespace::Agents => "agents",
        SkillNamespace::Codex => "codex",
        SkillNamespace::ClaudeCode => "claude-code",
    }
}

/// folder_name 的 NFC + lowercase 折叠 collision key。
fn folder_collision_key(folder_name: &str) -> String {
    folder_name.nfc().collect::<String>().to_lowercase()
}

/// 自定义 skills map 反序列化：遇重复 skill key 返回错误。
/// BTreeMap 默认会静默保留最后一个 entry，必须显式拒绝重复 key。
fn deserialize_skills_map<'de, D>(
    deserializer: D,
) -> std::result::Result<BTreeMap<String, VaultSkill>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SkillsVisitor;

    impl<'de> Visitor<'de> for SkillsVisitor {
        type Value = BTreeMap<String, VaultSkill>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a skills map")
        }

        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut skills = BTreeMap::new();
            while let Some(key) = map.next_key::<String>()? {
                if skills.contains_key(&key) {
                    return Err(de::Error::custom(format!("duplicate skill key: {key}")));
                }
                let value: VaultSkill = map.next_value()?;
                skills.insert(key, value);
            }
            Ok(skills)
        }
    }

    deserializer.deserialize_map(SkillsVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BLOB_A: &str =
        "blobs/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.skill.zip";

    fn valid_skill() -> VaultSkill {
        VaultSkill {
            id: "codex:ponytail".into(),
            name: "ponytail".into(),
            description: "desc".into(),
            namespace: SkillNamespace::Codex,
            folder_name: "ponytail".into(),
            hash: HASH_A.into(),
            blob: BLOB_A.into(),
            size: 123,
            updated_at: "2026-07-07T13:00:00Z".into(),
            updated_by: "device-a".into(),
        }
    }

    fn valid_skill_value() -> serde_json::Value {
        serde_json::json!({
            "id": "codex:ponytail",
            "name": "ponytail",
            "description": "desc",
            "namespace": "codex",
            "folder_name": "ponytail",
            "hash": HASH_A,
            "blob": BLOB_A,
            "size": 123,
            "updated_at": "2026-07-07T13:00:00Z",
            "updated_by": "device-a",
        })
    }

    fn manifest_with(skill_key: &str, skill: serde_json::Value) -> String {
        let mut skills = serde_json::Map::new();
        skills.insert(skill_key.to_string(), skill);
        let manifest = serde_json::json!({
            "schema": 1,
            "updated_at": "",
            "updated_by": "device-a",
            "skills": serde_json::Value::Object(skills),
        });
        serde_json::to_string(&manifest).unwrap()
    }

    #[test]
    fn manifest_roundtrip_preserves_skill_entry() {
        let mut manifest = VaultManifest::empty("device-a");
        manifest
            .skills
            .insert("codex:ponytail".into(), valid_skill());
        let text = serde_json::to_string(&manifest).unwrap();
        let back: VaultManifest = serde_json::from_str(&text).unwrap();
        assert_eq!(back.skills["codex:ponytail"].hash, HASH_A);
    }

    #[test]
    fn manifest_valid_parses_successfully() {
        let json = manifest_with("codex:ponytail", valid_skill_value());
        assert!(VaultManifest::parse_validated(json.as_bytes()).is_ok());
    }

    #[test]
    fn manifest_allows_same_folder_name_in_different_namespaces() {
        let mut codex_skill = valid_skill_value();
        codex_skill["id"] = serde_json::json!("codex:shared");
        codex_skill["name"] = serde_json::json!("shared");
        codex_skill["folder_name"] = serde_json::json!("shared");

        let mut claude_skill = valid_skill_value();
        claude_skill["id"] = serde_json::json!("claude-code:shared");
        claude_skill["name"] = serde_json::json!("shared");
        claude_skill["namespace"] = serde_json::json!("claude-code");
        claude_skill["folder_name"] = serde_json::json!("shared");

        let manifest = serde_json::json!({
            "schema": 1,
            "updated_at": "",
            "updated_by": "device-a",
            "skills": {
                "codex:shared": codex_skill,
                "claude-code:shared": claude_skill,
            },
        });
        let json = serde_json::to_string(&manifest).unwrap();

        let result = VaultManifest::parse_validated(json.as_bytes());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn manifest_rejects_same_folder_name_in_one_namespace() {
        let mut first_skill = valid_skill_value();
        first_skill["id"] = serde_json::json!("codex:shared-a");
        first_skill["name"] = serde_json::json!("shared-a");
        first_skill["folder_name"] = serde_json::json!("Shared");

        let mut second_skill = valid_skill_value();
        second_skill["id"] = serde_json::json!("codex:shared-b");
        second_skill["name"] = serde_json::json!("shared-b");
        second_skill["folder_name"] = serde_json::json!("shared");

        let manifest = serde_json::json!({
            "schema": 1,
            "updated_at": "",
            "updated_by": "device-a",
            "skills": {
                "codex:shared-a": first_skill,
                "codex:shared-b": second_skill,
            },
        });
        let json = serde_json::to_string(&manifest).unwrap();

        assert!(VaultManifest::parse_validated(json.as_bytes()).is_err());
    }

    #[test]
    fn manifest_rejects_key_id_namespace_and_unsafe_folder_mismatches() {
        for invalid in manifest_validation_fixtures() {
            assert!(
                VaultManifest::parse_validated(invalid.as_bytes()).is_err(),
                "expected invalid: {invalid}"
            );
        }
    }

    #[test]
    fn manifest_rejects_schema_duplicate_keys_hash_blob_and_size_violations() {
        for invalid in [
            unsupported_schema_fixture(),
            duplicate_skill_key_fixture(),
            invalid_hash_fixture("sha256:ABC"),
            invalid_hash_fixture("sha256:abc"),
            mismatched_blob_fixture("blobs/sha256/other.skill.zip"),
            invalid_size_fixture(0),
        ] {
            assert!(
                VaultManifest::parse_validated(invalid.as_bytes()).is_err(),
                "expected invalid: {invalid}"
            );
        }
    }

    fn manifest_validation_fixtures() -> Vec<String> {
        let mut fixtures = Vec::new();
        // map key != entry id
        fixtures.push(manifest_with("codex:other", valid_skill_value()));
        // id namespace 前缀 != namespace 字段
        let mut s = valid_skill_value();
        s["id"] = serde_json::json!("agents:ponytail");
        fixtures.push(manifest_with("agents:ponytail", s));
        // 不安全 folder_name：分隔符 / Windows 设备名 / 末尾点
        for folder in ["has/slash", "CON", "ends."] {
            let mut s = valid_skill_value();
            s["folder_name"] = serde_json::json!(folder);
            fixtures.push(manifest_with("codex:ponytail", s));
        }
        fixtures
    }

    fn unsupported_schema_fixture() -> String {
        let mut skills = serde_json::Map::new();
        skills.insert("codex:ponytail".into(), valid_skill_value());
        let manifest = serde_json::json!({
            "schema": 2,
            "updated_at": "",
            "updated_by": "device-a",
            "skills": serde_json::Value::Object(skills),
        });
        serde_json::to_string(&manifest).unwrap()
    }

    fn duplicate_skill_key_fixture() -> String {
        // serde_json::Map 无法承载重复 key，手动拼出含重复 skill key 的 JSON。
        let skill = serde_json::to_string(&valid_skill_value()).unwrap();
        format!(
            r#"{{"schema":1,"updated_at":"","updated_by":"device-a","skills":{{"codex:ponytail":{skill},"codex:ponytail":{skill}}}}}"#
        )
    }

    fn invalid_hash_fixture(hash: &str) -> String {
        let mut s = valid_skill_value();
        s["hash"] = serde_json::json!(hash);
        manifest_with("codex:ponytail", s)
    }

    fn mismatched_blob_fixture(blob: &str) -> String {
        let mut s = valid_skill_value();
        s["blob"] = serde_json::json!(blob);
        manifest_with("codex:ponytail", s)
    }

    fn invalid_size_fixture(size: u64) -> String {
        let mut s = valid_skill_value();
        s["size"] = serde_json::json!(size);
        manifest_with("codex:ponytail", s)
    }
}
