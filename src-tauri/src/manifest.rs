use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::errors::Result;

/// Compute a stable content hash for a skill directory.
///
/// Files are sorted by their normalized relative path (separator `/`), and the
/// hash covers each relative path plus its bytes. Ignore rules are applied so
/// `.git`, `node_modules`, `.env` and user patterns are excluded.
pub(crate) fn hash_dir(dir: &Path, ignore: &[String]) -> Result<String> {
    let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    collect_files(dir, dir, ignore, &mut entries)?;
    let mut hasher = Sha256::new();
    for (rel, content) in &entries {
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        hasher.update(content);
        hasher.update([0u8]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_files(
    root: &Path,
    current: &Path,
    ignore: &[String],
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if !current.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if is_ignored(&rel, ignore) {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, ignore, out)?;
        } else {
            let content = fs::read(&path)?;
            out.insert(rel, content);
        }
    }
    Ok(())
}

/// Whether a relative path should be ignored. Always skips `.git` and
/// `node_modules` segments and `.env` basenames, then applies user globs.
pub(crate) fn is_ignored(rel_path: &str, ignore: &[String]) -> bool {
    let normalized = rel_path.replace('\\', "/");
    let segments: Vec<&str> = normalized.split('/').collect();
    for seg in &segments {
        if *seg == ".git" || *seg == "node_modules" {
            return true;
        }
    }
    if segments.last() == Some(&".env") {
        return true;
    }
    for pat in ignore {
        if glob_match(pat, &normalized) {
            return true;
        }
    }
    false
}

/// Simple glob matcher: `*` matches any run of characters (including `/`),
/// `?` matches one character. `**` collapses into `*`.
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut pi = 0;
    let mut ti = 0;
    let mut star: Option<usize> = None;
    let mut st = 0;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            st = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            st += 1;
            ti = st;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn hash_is_stable_across_calls() {
        let dir = tempdir().unwrap();
        write(dir.path(), "SKILL.md", "---\nname: a\n---\n");
        write(dir.path(), "scripts/run.sh", "echo hi");
        let h1 = hash_dir(dir.path(), &[]).unwrap();
        let h2 = hash_dir(dir.path(), &[]).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_changes_when_content_changes() {
        let dir = tempdir().unwrap();
        write(dir.path(), "SKILL.md", "v1");
        let h1 = hash_dir(dir.path(), &[]).unwrap();
        write(dir.path(), "SKILL.md", "v2");
        let h2 = hash_dir(dir.path(), &[]).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_ignores_git_and_env() {
        let dir = tempdir().unwrap();
        write(dir.path(), "SKILL.md", "content");
        write(dir.path(), ".git/config", "git");
        write(dir.path(), ".env", "SECRET=1");
        let h = hash_dir(dir.path(), &[]).unwrap();
        // changing ignored files must not change the hash
        write(dir.path(), ".git/config", "changed");
        write(dir.path(), ".env", "SECRET=2");
        let h2 = hash_dir(dir.path(), &[]).unwrap();
        assert_eq!(h, h2);
    }

    #[test]
    fn glob_matches_double_star() {
        assert!(glob_match("**/.git/**", "skills/.git/config"));
        assert!(glob_match("**/node_modules/**", "a/b/node_modules/x/y"));
        assert!(glob_match("**/.env", "deeply/nested/.env"));
        assert!(!glob_match("**/.git/**", "skills/SKILL.md"));
    }
}
