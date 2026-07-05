use std::path::{Path, PathBuf};
use std::process::Command;

use crate::errors::{AppError, Result};

/// Thin wrapper over the system `git` CLI. Uses explicit argument lists; never
/// shells out, so remote URLs and commit messages are never interpreted by a
/// shell.
pub struct GitStore {
    pub repo_path: PathBuf,
}

impl GitStore {
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    pub fn check_git() -> Result<String> {
        let out = Command::new("git")
            .arg("--version")
            .output()
            .map_err(|e| AppError::Git(format!("git not available: {e}")))?;
        if !out.status.success() {
            return Err(AppError::Git("git --version failed".into()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub fn is_repo(&self) -> bool {
        self.repo_path.join(".git").exists()
    }

    pub fn init(&self) -> Result<()> {
        if self.is_repo() {
            return Ok(());
        }
        fs_create_dir_all(&self.repo_path)?;
        run_git(&self.repo_path, &["init"])?;
        let _ = run_git(&self.repo_path, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        // local identity so commits work without global git config
        let _ = run_git(&self.repo_path, &["config", "user.email", "skill-sync@local"]);
        let _ = run_git(&self.repo_path, &["config", "user.name", "Skill Sync"]);
        Ok(())
    }

    pub fn set_remote(&self, remote: &str) -> Result<()> {
        let _ = run_git(&self.repo_path, &["remote", "remove", "origin"]);
        run_git(&self.repo_path, &["remote", "add", "origin", remote])?;
        Ok(())
    }

    pub fn check_remote(remote: &str) -> Result<()> {
        let out = Command::new("git")
            .args(["ls-remote", remote])
            .output()
            .map_err(|e| AppError::Git(format!("cannot run git ls-remote: {e}")))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(AppError::Git(format!("remote not accessible: {err}")));
        }
        Ok(())
    }

    pub fn has_remote(&self) -> bool {
        Command::new("git")
            .args(["remote"])
            .current_dir(&self.repo_path)
            .output()
            .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false)
    }

    /// Fetch and fast-forward merge from origin. No-remote repos are skipped.
    pub fn pull(&self, branch: &str) -> Result<()> {
        if !self.has_remote() {
            return Ok(());
        }
        let _ = run_git(&self.repo_path, &["fetch", "origin", branch]);
        let verify = Command::new("git")
            .args(["rev-parse", "--verify", &format!("origin/{branch}")])
            .current_dir(&self.repo_path)
            .output()?;
        if verify.status.success() {
            let _ = run_git(
                &self.repo_path,
                &["merge", "--ff-only", &format!("origin/{branch}")],
            );
        }
        Ok(())
    }

    /// Stage all, commit if there are changes, push if a remote exists.
    pub fn commit_push(&self, branch: &str, message: &str) -> Result<()> {
        run_git(&self.repo_path, &["add", "."])?;
        let status = run_git(&self.repo_path, &["status", "--porcelain"])?;
        if status.trim().is_empty() {
            return Ok(());
        }
        run_git(
            &self.repo_path,
            &["commit", "-m", message],
        )?;
        if self.has_remote() {
            let _ = run_git(&self.repo_path, &["push", "-u", "origin", branch]);
        }
        Ok(())
    }
}

fn run_git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| AppError::Git(format!("git {} failed to start: {e}", args.join(" "))))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(AppError::Git(format!(
            "git {} failed: {stderr}{stdout}",
            args.join(" ")
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn fs_create_dir_all(p: &Path) -> Result<()> {
    std::fs::create_dir_all(p).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn check_git_returns_version() {
        // only run if git is installed
        if GitStore::check_git().is_ok() {
            let v = GitStore::check_git().unwrap();
            assert!(v.contains("git version"));
        }
    }

    #[test]
    fn init_creates_repo_and_commit_works() {
        let dir = tempdir().unwrap();
        let git = GitStore::new(dir.path());
        git.init().unwrap();
        assert!(git.is_repo());
        std::fs::write(dir.path().join("SKILL.md"), "content").unwrap();
        git.commit_push("main", "test: add skill").unwrap();
        // second commit with no changes is a no-op
        git.commit_push("main", "test: nothing").unwrap();
    }

    #[test]
    fn check_remote_rejects_invalid() {
        let res = GitStore::check_remote("not-a-valid-remote-url-xyz");
        assert!(res.is_err());
    }
}
