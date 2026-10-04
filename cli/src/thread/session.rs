use std::path::{Path, PathBuf};

use crate::ledger::location::{state_root_from_env, worktree_path, Repo};
use crate::ledger::{fetch_reporting, push_reporting};

pub const CLOUD_SESSION_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";
pub const MARKER_FILE: &str = "tsk-thread-id";

pub struct Session {
    pub repo: Repo,
    pub state_root: PathBuf,
    pub cloud_session: Option<String>,
    pub git_dir: PathBuf,
}

impl Session {
    pub fn discover() -> Result<Session, String> {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("error: could not read the current directory: {}", e))?;
        let repo = Repo::discover(&cwd)?;
        let state_root = state_root_from_env()?;
        let git_dir = PathBuf::from(repo.git.run_line([
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
        ])?);
        let cloud_session = std::env::var(CLOUD_SESSION_VAR)
            .ok()
            .filter(|v| !v.is_empty());
        Ok(Session {
            repo,
            state_root,
            cloud_session,
            git_dir,
        })
    }

    pub fn refresh(&self) -> Result<PathBuf, String> {
        fetch_reporting(&self.repo, &self.state_root).map(|outcome| outcome.path)
    }

    pub fn existing_worktree(&self) -> Result<Option<PathBuf>, String> {
        Ok(self
            .repo
            .read_clone_id()?
            .map(|clone_id| worktree_path(&self.state_root, &clone_id))
            .filter(|path| path.is_dir()))
    }

    pub fn push(&self, message: &str) -> Result<(), String> {
        push_reporting(&self.repo, &self.state_root, message).map(|_| ())
    }

    pub fn marker_path(&self) -> PathBuf {
        self.git_dir.join(MARKER_FILE)
    }

    pub fn actor_urn(&self) -> String {
        actor_urn(self.cloud_session.as_deref(), &self.git_dir)
    }
}

pub fn actor_urn(cloud_session: Option<&str>, git_dir: &Path) -> String {
    match cloud_session {
        Some(id) => format!("urn:tsk:cloudsession:{}", id),
        None => format!(
            "urn:tsk:worktree:{}",
            git_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_session_urn_names_the_session() {
        assert_eq!(
            actor_urn(Some("cse_01abc"), Path::new("/r/.git")),
            "urn:tsk:cloudsession:cse_01abc"
        );
    }

    #[test]
    fn main_worktree_urn_is_dot_git() {
        assert_eq!(
            actor_urn(None, Path::new("/r/.git")),
            "urn:tsk:worktree:.git"
        );
    }

    #[test]
    fn linked_worktree_urn_is_the_worktree_name() {
        assert_eq!(
            actor_urn(None, Path::new("/r/.git/worktrees/feature-x")),
            "urn:tsk:worktree:feature-x"
        );
    }
}
