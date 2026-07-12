use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

/// 三个固定用户级 skill namespace，供 vault manifest / sync_state / scanner / SyncPlan 共用。
/// serde 值为 `agents` / `codex` / `claude-code`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SkillNamespace {
    Agents,
    Codex,
    ClaudeCode,
}

/// namespace 的 canonical 字符串值，用于 skill id 前缀与序列化。
pub(crate) fn namespace_value(namespace: SkillNamespace) -> &'static str {
    match namespace {
        SkillNamespace::Agents => "agents",
        SkillNamespace::Codex => "codex",
        SkillNamespace::ClaudeCode => "claude-code",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct SkillMeta {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Skill {
    pub id: String,
    pub name: String,
    /// 目录名（fixed root 的直接子目录名）。
    pub folder_name: String,
    pub description: String,
    pub namespace: SkillNamespace,
    /// V1 等于经验证的单段 `folder_name`。
    pub relative_dir: String,
    pub source_path: String,
    pub hash: String,
    /// canonical zip 字节数；扫描阶段为 0，build_plan 打包后填充。
    pub zip_size: u64,
    pub modified_at: String,
}

/// 规范化 skill name：lowercase、trim、空格和 `_` 转 `-`、只保留 ASCII 字母数字和 `-`、
/// 连续 `-` 折叠、去除首尾 `-`。规范化后为空表示无法生成合法 id。
pub(crate) fn normalize_skill_name(name: &str) -> String {
    let lower: String = name
        .trim()
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let mut out = String::new();
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if c == ' ' || c == '_' || c == '-' {
            out.push('-');
        }
        // 其余非 ASCII 字符丢弃
    }
    let mut folded = String::new();
    let mut prev_dash = false;
    for c in out.chars() {
        if c == '-' {
            if !prev_dash {
                folded.push('-');
                prev_dash = true;
            }
        } else {
            folded.push(c);
            prev_dash = false;
        }
    }
    folded.trim_matches('-').to_string()
}

/// 由 namespace 与原始 name 构造 canonical skill id `<namespace>:<normalized-name>`。
#[allow(dead_code)] // Task 8 build_plan 接入非测试调用方后使用
pub(crate) fn skill_id(namespace: SkillNamespace, name: &str) -> String {
    format!(
        "{}:{}",
        namespace_value(namespace),
        normalize_skill_name(name)
    )
}

/// identity source：先取 trim 后非空的 frontmatter `name`，否则回退 `folder_name`；
/// 两者规范化后都为空则该目录 blocked（返回 `Skill` 错误）。
pub(crate) fn skill_id_from_metadata(
    namespace: SkillNamespace,
    name: Option<&str>,
    folder_name: &str,
) -> Result<String> {
    if let Some(n) = name {
        let normalized = normalize_skill_name(n);
        if !normalized.is_empty() {
            return Ok(format!("{}:{}", namespace_value(namespace), normalized));
        }
    }
    let normalized = normalize_skill_name(folder_name);
    if normalized.is_empty() {
        return Err(AppError::Skill(format!(
            "skill identity normalizes to empty (folder={folder_name:?})"
        )));
    }
    Ok(format!("{}:{}", namespace_value(namespace), normalized))
}

/// 一个固定 namespace root：`~/.agents/skills` / `~/.codex/skills` / `~/.claude/skills`。
#[derive(Debug, Clone)]
pub(crate) struct FixedSkillRoot {
    pub namespace: SkillNamespace,
    pub path: PathBuf,
}

/// 返回三个固定用户级 skill root，严格映射 `agents -> ~/.agents/skills`、
/// `codex -> ~/.codex/skills`、`claude-code -> ~/.claude/skills`。
/// 不接收 AppConfig，不扫描项目级目录或旧 hosts/custom_paths。
pub(crate) fn fixed_skill_roots(home: &Path) -> [FixedSkillRoot; 3] {
    [
        FixedSkillRoot {
            namespace: SkillNamespace::Agents,
            path: home.join(".agents").join("skills"),
        },
        FixedSkillRoot {
            namespace: SkillNamespace::Codex,
            path: home.join(".codex").join("skills"),
        },
        FixedSkillRoot {
            namespace: SkillNamespace::ClaudeCode,
            path: home.join(".claude").join("skills"),
        },
    ]
}

/// Parse a `SKILL.md` file's YAML front matter. Requires `name` and `description`.
pub(crate) fn parse_skill_md(content: &str) -> Result<SkillMeta> {
    let trimmed = content.trim_start_matches('\u{feff}');
    let rest = trimmed
        .strip_prefix("---")
        .ok_or_else(|| AppError::Skill("SKILL.md missing leading frontmatter delimiter".into()))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| AppError::Skill("SKILL.md frontmatter not closed".into()))?;
    let frontmatter = &rest[..end];

    #[derive(Deserialize)]
    struct RawMeta {
        name: Option<String>,
        description: Option<String>,
    }

    let raw: RawMeta = serde_yaml::from_str(frontmatter)
        .map_err(|e| AppError::Skill(format!("invalid SKILL.md frontmatter: {e}")))?;
    let name = raw
        .name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Skill("SKILL.md missing name".into()))?;
    let description = raw
        .description
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Skill("SKILL.md missing description".into()))?;

    Ok(SkillMeta { name, description })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_frontmatter() {
        let md = "---\nname: my-skill\ndescription: does things\n---\n# Body\n";
        let m = parse_skill_md(md).unwrap();
        assert_eq!(m.name, "my-skill");
        assert_eq!(m.description, "does things");
    }

    #[test]
    fn fails_without_frontmatter() {
        assert!(parse_skill_md("# just a body").is_err());
    }

    #[test]
    fn fails_when_not_closed() {
        assert!(parse_skill_md("---\nname: x\ndescription: y\nbody without close").is_err());
    }

    #[test]
    fn fails_when_name_missing() {
        assert!(parse_skill_md("---\ndescription: no name\n---\n").is_err());
    }

    #[test]
    fn fails_when_description_missing() {
        assert!(parse_skill_md("---\nname: x\n---\n").is_err());
    }

    #[test]
    fn trims_quoted_values() {
        let md = "---\nname: \"quoted-name\"\ndescription: 'quoted desc'\n---\n";
        let m = parse_skill_md(md).unwrap();
        assert_eq!(m.name, "quoted-name");
        assert_eq!(m.description, "quoted desc");
    }

    #[test]
    fn normalizes_skill_id_with_namespace_prefix() {
        assert_eq!(
            skill_id(SkillNamespace::Agents, "Pony Tail"),
            "agents:pony-tail"
        );
        assert_eq!(
            skill_id(SkillNamespace::Codex, "Pony Tail"),
            "codex:pony-tail"
        );
        assert_eq!(
            skill_id(SkillNamespace::ClaudeCode, "review_bot"),
            "claude-code:review-bot"
        );
    }

    #[test]
    fn identity_uses_non_empty_frontmatter_name_then_folder_fallback() {
        assert_eq!(
            skill_id_from_metadata(SkillNamespace::Agents, Some("Pony Tail"), "folder").unwrap(),
            "agents:pony-tail"
        );
        assert_eq!(
            skill_id_from_metadata(SkillNamespace::Agents, None, "Folder Name").unwrap(),
            "agents:folder-name"
        );
        assert_eq!(
            skill_id_from_metadata(SkillNamespace::Agents, Some("   "), "Folder Name").unwrap(),
            "agents:folder-name"
        );
        assert!(skill_id_from_metadata(SkillNamespace::Agents, Some("!!!"), "___").is_err());
    }

    #[test]
    fn normalize_drops_non_ascii_and_folds_dashes() {
        assert_eq!(normalize_skill_name("Póny  Tail__X"), "pny-tail-x");
        assert_eq!(normalize_skill_name("pony-tail"), "pony-tail");
        assert_eq!(normalize_skill_name("---"), "");
        assert_eq!(normalize_skill_name("  "), "");
    }

    fn root_pairs(roots: [FixedSkillRoot; 3]) -> [(SkillNamespace, String); 3] {
        roots.map(|r| {
            let name = r
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let parent_name = r
                .path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            (r.namespace, format!("{parent_name}/{name}"))
        })
    }

    #[test]
    fn fixed_roots_resolve_from_home_and_cannot_be_configured() {
        let roots = fixed_skill_roots(Path::new("/home/test"));
        assert_eq!(
            root_pairs(roots),
            [
                (SkillNamespace::Agents, ".agents/skills".into()),
                (SkillNamespace::Codex, ".codex/skills".into()),
                (SkillNamespace::ClaudeCode, ".claude/skills".into()),
            ]
        );
    }
}
