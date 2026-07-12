use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::Result;
use crate::portable_path::{collision_key, validate_component};
use crate::skill::{
    fixed_skill_roots, parse_skill_md, skill_id_from_metadata, FixedSkillRoot, Skill,
    SkillNamespace,
};

fn modified_time(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let dt: DateTime<Utc> = mtime.into();
    Some(dt.to_rfc3339())
}

// ---- 新 vault scanner：三个固定 namespace root ----

/// 新 scanner 的扫描结果：skills（非碰撞）+ warnings + 每个 root 的状态 + 本地碰撞。
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScanResult {
    pub skills: Vec<Skill>,
    pub warnings: Vec<String>,
    pub roots: Vec<ScanRootStatus>,
    pub collisions: Vec<ScanCollision>,
}

#[allow(dead_code)]
impl ScanResult {
    pub(crate) fn skill_ids(&self) -> Vec<String> {
        // 保留扫描插入顺序（固定 root 顺序 Agents/Codex/ClaudeCode，root 内按 folder_name 排序）。
        self.skills.iter().map(|s| s.id.clone()).collect()
    }

    pub(crate) fn paths(&self) -> impl Iterator<Item = &str> {
        self.skills.iter().map(|s| s.source_path.as_str())
    }

    pub(crate) fn collision(&self, kind: ScanCollisionKind) -> Option<&ScanCollision> {
        self.collisions.iter().find(|c| c.kind == kind)
    }
}

/// 单个固定 root 的扫描状态。root 不存在或不可读时只记录状态，不推断删除。
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScanRootStatus {
    pub namespace: SkillNamespace,
    pub root_path: String,
    pub exists: bool,
    pub readable: bool,
    pub scan_complete: bool,
    pub error: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScanCollisionKind {
    NormalizedId,
    FoldedFolderName,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScanCollision {
    pub namespace: SkillNamespace,
    pub collision_key: String,
    pub kind: ScanCollisionKind,
    pub skill_ids: Vec<String>,
    pub paths: Vec<String>,
}

/// 扫描三个固定 namespace root。只看直接子目录；codex root 跳过 `.system`，所有 root
/// 跳过 `.skill-sync-staging` / `.skill-sync-rollback` / `.skill-sync-trash`；无合法
/// `SKILL.md` 的目录跳过且不递归。同 namespace 的 normalized id 或 folded folder
/// collision 返回结构化 `ScanCollision` 并把涉及项排除出 skills。
#[allow(dead_code)]
pub(crate) fn scan_fixed_roots(home: &Path) -> Result<ScanResult> {
    let roots = fixed_skill_roots(home);
    let mut skills = Vec::new();
    let mut warnings = Vec::new();
    let mut root_statuses = Vec::new();
    let mut collisions = Vec::new();

    for root in &roots {
        let status = scan_one_root(root, &mut skills, &mut warnings, &mut collisions);
        root_statuses.push(status);
    }
    Ok(ScanResult {
        skills,
        warnings,
        roots: root_statuses,
        collisions,
    })
}

struct SkillCandidate {
    id: String,
    folder_name: String,
    name: String,
    description: String,
    source_path: String,
    modified_at: String,
}

fn scan_one_root(
    root: &FixedSkillRoot,
    skills: &mut Vec<Skill>,
    warnings: &mut Vec<String>,
    collisions: &mut Vec<ScanCollision>,
) -> ScanRootStatus {
    let root_path = root.path.to_string_lossy().to_string();
    if !root.path.exists() {
        return ScanRootStatus {
            namespace: root.namespace,
            root_path,
            exists: false,
            readable: false,
            scan_complete: true,
            error: None,
        };
    }
    let read_dir = match fs::read_dir(&root.path) {
        Ok(rd) => rd,
        Err(e) => {
            return ScanRootStatus {
                namespace: root.namespace,
                root_path,
                exists: true,
                readable: false,
                scan_complete: false,
                error: Some(e.to_string()),
            };
        }
    };

    let mut candidates: Vec<SkillCandidate> = Vec::new();
    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warnings.push(format!(
                    "{}: read_dir entry error: {e}",
                    root.path.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if is_reserved_dir(&folder_name) {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue; // 非 skill 目录，跳过且不递归
        }
        let content = match fs::read_to_string(&skill_md) {
            Ok(c) => c,
            Err(e) => {
                warnings.push(format!("{folder_name}: cannot read SKILL.md: {e}"));
                continue;
            }
        };
        let meta = match parse_skill_md(&content) {
            Ok(m) => m,
            Err(e) => {
                warnings.push(format!("{folder_name}: invalid SKILL.md: {e}"));
                continue;
            }
        };
        if validate_component(&folder_name).is_err() {
            warnings.push(format!("{folder_name}: unsafe folder name, skipped"));
            continue;
        }
        let id = match skill_id_from_metadata(root.namespace, Some(&meta.name), &folder_name) {
            Ok(id) => id,
            Err(e) => {
                warnings.push(format!("{folder_name}: identity blocked: {e}"));
                continue;
            }
        };
        candidates.push(SkillCandidate {
            id,
            folder_name,
            name: meta.name,
            description: meta.description,
            source_path: path.to_string_lossy().to_string(),
            modified_at: modified_time(&path).unwrap_or_default(),
        });
    }

    collect_collisions(root.namespace, &mut candidates, collisions, skills);
    ScanRootStatus {
        namespace: root.namespace,
        root_path,
        exists: true,
        readable: true,
        scan_complete: true,
        error: None,
    }
}

/// 事务保留目录与 codex `.system`：扫描时一律跳过。
fn is_reserved_dir(name: &str) -> bool {
    matches!(
        name,
        ".system" | ".skill-sync-staging" | ".skill-sync-rollback" | ".skill-sync-trash"
    )
}

/// 检测同 namespace 内的 normalized id 与 folded folder collision，碰撞项排除出 skills。
fn collect_collisions(
    namespace: SkillNamespace,
    candidates: &mut Vec<SkillCandidate>,
    collisions: &mut Vec<ScanCollision>,
    skills: &mut Vec<Skill>,
) {
    candidates.sort_by(|a, b| a.folder_name.cmp(&b.folder_name));

    // normalized id collision（id 已含规范化 name，重复 id 即碰撞）
    let mut blocked: Vec<bool> = vec![false; candidates.len()];
    let mut id_groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        if let Some(g) = id_groups.iter_mut().find(|(k, _)| k == &c.id) {
            g.1.push(i);
        } else {
            id_groups.push((c.id.clone(), vec![i]));
        }
    }
    for (id, idxs) in &id_groups {
        if idxs.len() > 1 {
            for &i in idxs {
                blocked[i] = true;
            }
            collisions.push(ScanCollision {
                namespace,
                collision_key: id.clone(),
                kind: ScanCollisionKind::NormalizedId,
                skill_ids: idxs.iter().map(|&i| candidates[i].id.clone()).collect(),
                paths: idxs
                    .iter()
                    .map(|&i| candidates[i].source_path.clone())
                    .collect(),
            });
        }
    }

    // folded folder collision（NFC + lowercase）
    let mut folder_groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let key = collision_key(&c.folder_name);
        if let Some(g) = folder_groups.iter_mut().find(|(k, _)| k == &key) {
            g.1.push(i);
        } else {
            folder_groups.push((key, vec![i]));
        }
    }
    for (key, idxs) in &folder_groups {
        if idxs.len() > 1 {
            for &i in idxs {
                blocked[i] = true;
            }
            collisions.push(ScanCollision {
                namespace,
                collision_key: key.clone(),
                kind: ScanCollisionKind::FoldedFolderName,
                skill_ids: idxs.iter().map(|&i| candidates[i].id.clone()).collect(),
                paths: idxs
                    .iter()
                    .map(|&i| candidates[i].source_path.clone())
                    .collect(),
            });
        }
    }

    for (i, c) in candidates.drain(..).enumerate() {
        if !blocked[i] {
            skills.push(Skill {
                id: c.id,
                name: c.name,
                folder_name: c.folder_name.clone(),
                description: c.description,
                namespace,
                relative_dir: c.folder_name,
                source_path: c.source_path,
                hash: String::new(),
                zip_size: 0,
                modified_at: c.modified_at,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // ---- 新 vault scanner 测试 ----

    fn make_skill_named(base: &Path, folder: &str, name: &str, desc: &str) {
        let dir = base.join(folder);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\nbody"),
        )
        .unwrap();
    }

    fn ns_root(home: &Path, namespace: SkillNamespace) -> PathBuf {
        fixed_skill_roots(home)
            .into_iter()
            .find(|r| r.namespace == namespace)
            .map(|r| r.path)
            .unwrap()
    }

    fn fixed_root_fixture() -> tempfile::TempDir {
        let home = tempdir().unwrap();
        for root in fixed_skill_roots(home.path()) {
            let dir = root.path.join("valid");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("SKILL.md"),
                "---\nname: valid\ndescription: d\n---\nbody",
            )
            .unwrap();
        }
        // codex `.system` 与无 SKILL.md 的 `aggregate` 应被跳过；项目级目录不在固定 root 下。
        let codex_root = ns_root(home.path(), SkillNamespace::Codex);
        fs::create_dir_all(codex_root.join(".system")).unwrap();
        fs::write(
            codex_root.join(".system/SKILL.md"),
            "---\nname: sys\ndescription: d\n---\n",
        )
        .unwrap();
        fs::create_dir_all(codex_root.join("aggregate")).unwrap();
        fs::create_dir_all(home.path().join("project/skills/x")).unwrap();
        fs::write(
            home.path().join("project/skills/x/SKILL.md"),
            "---\nname: x\ndescription: d\n---\n",
        )
        .unwrap();
        home
    }

    #[test]
    fn scanner_excludes_project_roots_codex_system_and_non_skills() {
        let home = fixed_root_fixture();
        let result = scan_fixed_roots(home.path()).unwrap();
        assert_eq!(
            result.skill_ids(),
            ["agents:valid", "codex:valid", "claude-code:valid"]
        );
        assert!(!result
            .paths()
            .any(|p| p.contains("project") || p.contains(".system") || p.contains("aggregate")));
    }

    #[test]
    fn scanner_excludes_local_apply_reserved_directories() {
        let home = tempdir().unwrap();
        for root in fixed_skill_roots(home.path()) {
            let dir = root.path.join("keep");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("SKILL.md"),
                "---\nname: keep\ndescription: d\n---\n",
            )
            .unwrap();
        }
        let agents_root = ns_root(home.path(), SkillNamespace::Agents);
        for reserved in [
            ".skill-sync-staging",
            ".skill-sync-rollback",
            ".skill-sync-trash",
        ] {
            let dir = agents_root.join(reserved);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("SKILL.md"),
                "---\nname: leak\ndescription: d\n---\n",
            )
            .unwrap();
        }
        let result = scan_fixed_roots(home.path()).unwrap();
        assert!(!result.paths().any(|p| {
            p.contains(".skill-sync-staging")
                || p.contains(".skill-sync-rollback")
                || p.contains(".skill-sync-trash")
        }));
    }

    #[test]
    fn duplicate_normalized_id_or_case_folded_folder_is_blocked() {
        let home = tempdir().unwrap();
        let agents = ns_root(home.path(), SkillNamespace::Agents);
        // id collision: 两个目录都规范化为 agents:pony-tail（space 与 dash 在文件系统上不同）
        make_skill_named(&agents, "Pony Tail", "Pony Tail", "d");
        make_skill_named(&agents, "pony-tail", "pony-tail", "d");

        let result = scan_fixed_roots(home.path()).unwrap();
        let id_collision = result.collision(ScanCollisionKind::NormalizedId).unwrap();
        assert_eq!(
            id_collision.skill_ids,
            ["agents:pony-tail", "agents:pony-tail"]
        );
        assert_eq!(id_collision.paths.len(), 2);
        // 碰撞项不进入 skills
        assert!(result.skills.is_empty());
    }

    /// folded folder collision 需要两个 NFC+lowercase 折叠相等但文件系统不同的目录。
    /// Windows 文件系统对大小写和 NFC 变体都会合并，故仅在 Unix（大小写敏感）下可测。
    #[cfg(unix)]
    #[test]
    fn case_folded_folder_collision_is_blocked() {
        let home = tempdir().unwrap();
        let agents = ns_root(home.path(), SkillNamespace::Agents);
        make_skill_named(&agents, "Pónytail", "alpha", "d");
        make_skill_named(&agents, "pónytail", "beta", "d");

        let result = scan_fixed_roots(home.path()).unwrap();
        let folder_collision = result
            .collision(ScanCollisionKind::FoldedFolderName)
            .unwrap();
        assert_eq!(folder_collision.collision_key, "pónytail");
        assert_eq!(folder_collision.skill_ids.len(), 2);
        assert_eq!(folder_collision.paths.len(), 2);
    }

    #[test]
    fn same_name_in_different_namespaces_is_not_a_collision() {
        let home = tempdir().unwrap();
        for root in fixed_skill_roots(home.path()) {
            make_skill_named(&root.path, "shared", "shared", "d");
        }
        let result = scan_fixed_roots(home.path()).unwrap();
        assert!(result.collisions.is_empty());
        assert_eq!(
            result.skill_ids(),
            ["agents:shared", "codex:shared", "claude-code:shared"]
        );
    }
}
