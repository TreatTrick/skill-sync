use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{expand_path, AppConfig};
use crate::errors::{AppError, Result};
use crate::git_store::GitStore;
use crate::manifest::hash_dir;
use crate::skill::{parse_skill_md, Skill};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncPlan {
    pub uploads: Vec<SyncAction>,
    pub downloads: Vec<SyncAction>,
    pub updates: Vec<SyncAction>,
    pub deletes: Vec<SyncAction>,
    pub conflicts: Vec<Conflict>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAction {
    pub skill_id: String,
    pub name: String,
    pub host: String,
    pub source_path: String,
    pub repo_path: String,
    /// "upload" (local -> repo) or "download" (repo -> local).
    pub direction: String,
    pub local_hash: String,
    pub remote_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub skill_id: String,
    pub name: String,
    pub host: String,
    pub local_path: String,
    pub repo_path: String,
    pub local_hash: String,
    pub remote_hash: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LockFile {
    /// skill_id -> last synced hash.
    pub skills: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub applied: Vec<String>,
    pub backups: Vec<String>,
    pub warnings: Vec<String>,
}

const LOCK_FILE_NAME: &str = "skill-sync.lock.json";
const SKILLS_DIR: &str = "skills";

/// Build a sync plan by comparing local skills with the remote repo state.
/// Read-only: writes nothing. Uses the lock file to distinguish update from
/// conflict when both sides changed.
pub fn build_plan(config: &AppConfig, local_skills: &[Skill]) -> Result<SyncPlan> {
    let repo_path = expand_path(&config.repository.local_path)?;
    let git = GitStore::new(&repo_path);
    if !git.is_repo() {
        return Err(AppError::NotConfigured(
            "sync repository is not a git repo; finish onboarding first".into(),
        ));
    }
    git.pull(&config.repository.branch)?;

    let remote_skills = read_remote_skills(&repo_path, &config.ignore)?;
    let lock = read_lock(&repo_path);

    let mut plan = SyncPlan::default();
    let local_map: HashMap<&str, &Skill> = local_skills
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();
    let remote_map: HashMap<&str, &Skill> = remote_skills
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();

    let mut all_ids: Vec<&str> = local_map.keys().copied().collect();
    all_ids.extend(remote_map.keys().copied());
    all_ids.sort();
    all_ids.dedup();

    for id in all_ids {
        let local = local_map.get(id);
        let remote = remote_map.get(id);
        match (local, remote) {
            (Some(l), None) => plan.uploads.push(action(l, "upload", &l.hash, "")),
            (None, Some(r)) => plan.downloads.push(action(r, "download", "", &r.hash)),
            (Some(l), Some(r)) => {
                if l.hash == r.hash {
                    continue;
                }
                match lock.skills.get(id) {
                    Some(last) if *last == l.hash => {
                        plan.updates.push(action(r, "download", &l.hash, &r.hash));
                    }
                    Some(last) if *last == r.hash => {
                        plan.updates.push(action(l, "upload", &l.hash, &r.hash));
                    }
                    _ => plan
                        .conflicts
                        .push(conflict(l, r, "both changed since last sync")),
                }
            }
            (None, None) => {}
        }
        if local.is_none() && remote.is_none() && lock.skills.contains_key(id) {
            plan.warnings
                .push(format!("{id}: existed in last sync but missing on both sides; skipped"));
        }
    }
    Ok(plan)
}

fn action(s: &Skill, direction: &str, local_hash: &str, remote_hash: &str) -> SyncAction {
    SyncAction {
        skill_id: s.id.clone(),
        name: s.name.clone(),
        host: s.host.clone(),
        source_path: s.source_path.clone(),
        repo_path: s.repo_path.clone(),
        direction: direction.to_string(),
        local_hash: local_hash.to_string(),
        remote_hash: remote_hash.to_string(),
    }
}

fn conflict(l: &Skill, r: &Skill, reason: &str) -> Conflict {
    Conflict {
        skill_id: l.id.clone(),
        name: l.name.clone(),
        host: l.host.clone(),
        local_path: l.source_path.clone(),
        repo_path: r.repo_path.clone(),
        local_hash: l.hash.clone(),
        remote_hash: r.hash.clone(),
        reason: reason.to_string(),
    }
}

/// Read skills from the repo's `skills/<host>/<name>/` tree.
pub fn read_remote_skills(repo_path: &Path, ignore: &[String]) -> Result<Vec<Skill>> {
    let skills_dir = repo_path.join(SKILLS_DIR);
    let mut skills = Vec::new();
    if !skills_dir.exists() {
        return Ok(skills);
    }
    for host_entry in fs::read_dir(&skills_dir)? {
        let host_entry = host_entry?;
        let host_path = host_entry.path();
        if !host_path.is_dir() {
            continue;
        }
        let host = host_entry.file_name().to_string_lossy().to_string();
        for skill_entry in fs::read_dir(&host_path)? {
            let skill_entry = skill_entry?;
            let skill_path = skill_entry.path();
            if !skill_path.is_dir() {
                continue;
            }
            let name = match skill_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let skill_md = skill_path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let content = fs::read_to_string(&skill_md)?;
            let meta = match parse_skill_md(&content) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let hash = hash_dir(&skill_path, ignore)?;
            skills.push(Skill {
                id: format!("{host}/{name}"),
                name: meta.name,
                description: meta.description,
                host: host.clone(),
                source_path: skill_path.to_string_lossy().to_string(),
                repo_path: format!("skills/{host}/{name}"),
                hash,
                modified_at: String::new(),
                enabled: true,
            });
        }
    }
    Ok(skills)
}

fn lock_path(repo_path: &Path) -> PathBuf {
    repo_path.join(LOCK_FILE_NAME)
}

pub fn read_lock(repo_path: &Path) -> LockFile {
    let path = lock_path(repo_path);
    if !path.exists() {
        return LockFile::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_lock(repo_path: &Path, lock: &LockFile) -> Result<()> {
    let text = serde_json::to_string_pretty(lock)?;
    fs::write(lock_path(repo_path), text)?;
    Ok(())
}

/// Apply a sync plan. `decisions` maps a conflict skill id to `"local"` or
/// `"remote"` (anything else, including missing, means skip). Backs up local
/// skills before overwriting, updates the lock to the new repo state, then
/// commits and pushes.
pub fn apply_plan(
    config: &AppConfig,
    plan: &SyncPlan,
    decisions: &HashMap<String, String>,
) -> Result<ApplyResult> {
    let repo_path = expand_path(&config.repository.local_path)?;
    let git = GitStore::new(&repo_path);
    let mut applied = Vec::new();
    let mut backups = Vec::new();
    let mut warnings = plan.warnings.clone();

    for a in plan
        .uploads
        .iter()
        .chain(plan.updates.iter().filter(|a| a.direction == "upload"))
    {
        copy_skill_dir(Path::new(&a.source_path), &repo_path.join(&a.repo_path))?;
        applied.push(a.skill_id.clone());
    }

    for a in plan
        .downloads
        .iter()
        .chain(plan.updates.iter().filter(|a| a.direction == "download"))
    {
        let target = local_target_path(config, &a.host, &a.skill_id)?;
        if target.exists() && config.defaults.backup {
            let entry = crate::backup::create_backup(&target, &a.skill_id)?;
            backups.push(entry.id);
        }
        fs::create_dir_all(&target)?;
        copy_skill_dir(&repo_path.join(&a.repo_path), &target)?;
        applied.push(a.skill_id.clone());
    }

    for c in &plan.conflicts {
        let decision = decisions.get(&c.skill_id).map(|s| s.as_str()).unwrap_or("skip");
        match decision {
            "local" => {
                copy_skill_dir(Path::new(&c.local_path), &repo_path.join(&c.repo_path))?;
                applied.push(c.skill_id.clone());
            }
            "remote" => {
                let target = local_target_path(config, &c.host, &c.skill_id)?;
                if target.exists() && config.defaults.backup {
                    let entry = crate::backup::create_backup(&target, &c.skill_id)?;
                    backups.push(entry.id);
                }
                fs::create_dir_all(&target)?;
                copy_skill_dir(&repo_path.join(&c.repo_path), &target)?;
                applied.push(c.skill_id.clone());
            }
            _ => warnings.push(format!("{}: conflict skipped", c.skill_id)),
        }
    }

    // Lock reflects the current repo state after apply.
    let remote_after = read_remote_skills(&repo_path, &config.ignore)?;
    let mut lock = LockFile::default();
    for s in &remote_after {
        lock.skills.insert(s.id.clone(), s.hash.clone());
    }
    write_lock(&repo_path, &lock)?;

    git.commit_push(&config.repository.branch, "skill-sync: apply sync plan")?;

    Ok(ApplyResult {
        applied,
        backups,
        warnings,
    })
}

fn local_target_path(config: &AppConfig, host: &str, skill_id: &str) -> Result<PathBuf> {
    let name = skill_id.split('/').nth(1).unwrap_or(skill_id);
    let paths: &[String] = match host {
        "codex" => &config.hosts.codex.paths,
        "claude" => &config.hosts.claude.paths,
        _ => &config.custom_paths,
    };
    let base = paths
        .first()
        .ok_or_else(|| AppError::Sync(format!("no path configured for host {host}")))?;
    let expanded = expand_path(base)?;
    Ok(expanded.join(name))
}

fn copy_skill_dir(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_skill_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_local_skill(base: &Path, name: &str, desc: &str) {
        let dir = base.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\nbody"),
        )
        .unwrap();
    }

    fn config_with_repo(repo: &Path, local_root: &Path) -> AppConfig {
        let mut cfg = AppConfig::default_config();
        cfg.repository.local_path = repo.to_string_lossy().to_string();
        cfg.repository.branch = "main".into();
        cfg.hosts.codex.paths = vec![local_root.to_string_lossy().to_string()];
        cfg.hosts.codex.enabled = true;
        cfg.hosts.claude.enabled = false;
        cfg
    }

    #[test]
    fn upload_then_in_sync() {
        let repo = tempdir().unwrap();
        let local = tempdir().unwrap();
        GitStore::new(repo.path()).init().unwrap();
        make_local_skill(local.path(), "alpha", "first skill");

        let cfg = config_with_repo(repo.path(), local.path());
        let (skills, _w) = crate::detect::scan_local_skills(&cfg).unwrap();

        let plan = build_plan(&cfg, &skills).unwrap();
        assert_eq!(plan.uploads.len(), 1);
        assert!(plan.downloads.is_empty());

        let result = apply_plan(&cfg, &plan, &HashMap::new()).unwrap();
        assert_eq!(result.applied.len(), 1);

        let (skills2, _) = crate::detect::scan_local_skills(&cfg).unwrap();
        let plan2 = build_plan(&cfg, &skills2).unwrap();
        assert!(plan2.uploads.is_empty());
        assert!(plan2.downloads.is_empty());
        assert!(plan2.conflicts.is_empty());
    }

    #[test]
    fn download_from_repo_to_empty_local() {
        let repo = tempdir().unwrap();
        let local = tempdir().unwrap();
        let git = GitStore::new(repo.path());
        git.init().unwrap();
        let repo_skill = repo.path().join("skills").join("codex").join("beta");
        fs::create_dir_all(&repo_skill).unwrap();
        fs::write(
            repo_skill.join("SKILL.md"),
            "---\nname: beta\ndescription: from repo\n---\nbody",
        )
        .unwrap();
        git.commit_push("main", "seed").unwrap();

        let cfg = config_with_repo(repo.path(), local.path());
        let plan = build_plan(&cfg, &[]).unwrap();
        assert_eq!(plan.downloads.len(), 1);

        let result = apply_plan(&cfg, &plan, &HashMap::new()).unwrap();
        assert!(result.applied.contains(&"codex/beta".to_string()));
        assert!(local.path().join("beta").join("SKILL.md").exists());
    }
}
