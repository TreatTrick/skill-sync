use std::collections::HashSet;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::config::{expand_path, AppConfig};
use crate::errors::Result;
use crate::manifest::hash_dir;
use crate::skill::{parse_skill_md, Skill};

/// Scan all enabled host paths (codex + claude) plus custom paths.
/// Missing paths are not fatal; invalid skills become warnings.
pub(crate) fn scan_local_skills(config: &AppConfig) -> Result<(Vec<Skill>, Vec<String>)> {
    let mut skills = Vec::new();
    let mut warnings = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for (host, host_cfg) in config.enabled_hosts() {
        for raw_path in &host_cfg.paths {
            let path = match expand_path(raw_path) {
                Ok(p) => p,
                Err(e) => {
                    warnings.push(format!("{host}: cannot expand path {raw_path}: {e}"));
                    continue;
                }
            };
            if !path.exists() {
                continue;
            }
            scan_host_dir(
                host,
                &path,
                config,
                &mut skills,
                &mut warnings,
                &mut seen_ids,
            )?;
        }
    }

    for raw_path in &config.custom_paths {
        let path = match expand_path(raw_path) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if path.exists() {
            scan_host_dir(
                "custom",
                &path,
                config,
                &mut skills,
                &mut warnings,
                &mut seen_ids,
            )?;
        }
    }

    Ok((skills, warnings))
}

#[allow(clippy::too_many_arguments)]
fn scan_host_dir(
    host: &str,
    dir: &Path,
    config: &AppConfig,
    skills: &mut Vec<Skill>,
    warnings: &mut Vec<String>,
    seen: &mut HashSet<String>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            warnings.push(format!("{host}/{name}: no SKILL.md, skipped"));
            continue;
        }
        let content = match fs::read_to_string(&skill_md) {
            Ok(c) => c,
            Err(e) => {
                warnings.push(format!("{host}/{name}: cannot read SKILL.md: {e}"));
                continue;
            }
        };
        let meta = match parse_skill_md(&content) {
            Ok(m) => m,
            Err(e) => {
                warnings.push(format!("{host}/{name}: invalid SKILL.md: {e}"));
                continue;
            }
        };
        let id = format!("{host}/{name}");
        if !seen.insert(id.clone()) {
            warnings.push(format!("duplicate skill id: {id}"));
            continue;
        }
        let hash = match hash_dir(&path, &config.ignore) {
            Ok(h) => h,
            Err(e) => {
                warnings.push(format!("{host}/{name}: hash failed: {e}"));
                continue;
            }
        };
        let modified_at = modified_time(&path).unwrap_or_default();
        let repo_path = format!("skills/{host}/{name}");
        skills.push(Skill {
            id,
            name: meta.name,
            description: meta.description,
            host: host.to_string(),
            source_path: path.to_string_lossy().to_string(),
            repo_path,
            hash,
            modified_at,
            enabled: true,
        });
    }
    Ok(())
}

fn modified_time(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let dt: DateTime<Utc> = mtime.into();
    Some(dt.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_skill(base: &Path, name: &str, desc: &str) {
        let dir = base.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\nbody"),
        )
        .unwrap();
    }

    #[test]
    fn scans_valid_skills_and_warns_on_invalid() {
        let root = tempdir().unwrap();
        make_skill(root.path(), "good", "a good skill");
        fs::create_dir_all(root.path().join("bad")).unwrap();
        fs::write(
            root.path().join("bad").join("SKILL.md"),
            "---\nname: bad\n---\n",
        )
        .unwrap();
        fs::create_dir_all(root.path().join("notes")).unwrap();

        let mut cfg = AppConfig::default_config();
        cfg.hosts.codex.paths = vec![root.path().to_string_lossy().to_string()];
        cfg.hosts.codex.enabled = true;
        cfg.hosts.claude.enabled = false;

        let (skills, warnings) = scan_local_skills(&cfg).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "codex/good");
        assert_eq!(skills[0].repo_path, "skills/codex/good");
        assert!(warnings.iter().any(|w| w.contains("bad")));
    }

    #[test]
    fn missing_path_is_not_fatal() {
        let mut cfg = AppConfig::default_config();
        cfg.hosts.codex.paths = vec!["/nonexistent/path/xyz".into()];
        cfg.hosts.codex.enabled = true;
        cfg.hosts.claude.enabled = false;
        let (skills, warnings) = scan_local_skills(&cfg).unwrap();
        assert!(skills.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn reports_duplicate_ids_across_paths() {
        let root = tempdir().unwrap();
        make_skill(root.path(), "dup", "first");
        let root2 = tempdir().unwrap();
        make_skill(root2.path(), "dup", "second");

        let mut cfg = AppConfig::default_config();
        cfg.hosts.codex.paths = vec![
            root.path().to_string_lossy().to_string(),
            root2.path().to_string_lossy().to_string(),
        ];
        cfg.hosts.codex.enabled = true;
        cfg.hosts.claude.enabled = false;

        let (skills, warnings) = scan_local_skills(&cfg).unwrap();
        assert_eq!(skills.len(), 1);
        assert!(warnings.iter().any(|w| w.contains("duplicate")));
    }
}
