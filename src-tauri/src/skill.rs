use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct SkillMeta {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Skill {
    /// Stable id: `<host>/<directory-name>`.
    pub id: String,
    /// From `SKILL.md` front matter `name`.
    pub name: String,
    /// From `SKILL.md` front matter `description`.
    pub description: String,
    /// Source agent: `codex` or `claude`.
    pub host: String,
    /// Local absolute path of the skill directory.
    pub source_path: String,
    /// Normalized path inside the sync repo, e.g. `skills/codex/my-skill`.
    pub repo_path: String,
    /// Content hash of the skill directory (ignoring rules applied).
    pub hash: String,
    /// Last modified time, ISO 8601.
    pub modified_at: String,
    /// Whether this skill participates in sync.
    pub enabled: bool,
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
}
