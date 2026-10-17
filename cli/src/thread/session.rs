use std::path::{Path, PathBuf};

use crate::ledger::location::Repo;
use crate::ledger::store::{self, BindingKind, LedgerStore, RefreshOutcome};
use crate::ledger::{fetch_keeping_local_reporting, push_reporting};

pub const CLOUD_SESSION_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";
pub const MARKER_FILE: &str = "tsk-thread-id";

pub struct Session {
    pub repo: Repo,
    pub cloud_session: Option<String>,
    pub git_dir: PathBuf,
    pub store: Box<dyn LedgerStore>,
}

impl Session {
    pub fn discover() -> Result<Session, String> {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("error: could not read the current directory: {}", e))?;
        let repo = Repo::discover(&cwd)?;
        let git_dir = PathBuf::from(repo.git.run_line([
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
        ])?);
        let cloud_session = std::env::var(CLOUD_SESSION_VAR)
            .ok()
            .filter(|v| !v.is_empty());
        let store = store::open(&cwd)?;
        Ok(Session {
            repo,
            cloud_session,
            git_dir,
            store,
        })
    }

    pub fn refresh(&self) -> Result<RefreshOutcome, String> {
        fetch_keeping_local_reporting(self.store.as_ref())
    }

    pub fn push(&self, message: &str) -> Result<(), String> {
        push_reporting(self.store.as_ref(), message).map(|_| ())
    }

    pub fn marker_path(&self) -> PathBuf {
        self.git_dir.join(MARKER_FILE)
    }

    /// The key and kind this session's binding has in the ledger: the session ID for a
    /// cloud session, `<clone-id>/<worktree-name>` for a local code worktree. `None` for a
    /// worktree of a clone that has no clone id yet, which has no ledger store either.
    pub fn actor_key(&self) -> Result<Option<(String, BindingKind)>, String> {
        match &self.cloud_session {
            Some(cloud) => Ok(Some((cloud.clone(), BindingKind::Cloud))),
            None => Ok(self.repo.read_clone_id()?.map(|clone_id| {
                (
                    worktree_actor(&clone_id, &self.git_dir),
                    BindingKind::Worktree,
                )
            })),
        }
    }

    pub fn actor_urn(&self) -> String {
        actor_urn(self.cloud_session.as_deref(), &self.git_dir)
    }
}

/// The actor key of a local code worktree: the clone ID, a slash, and `main` for the main
/// worktree or the name of the linked worktree. `git_dir` is the worktree's git
/// directory: `<repo>/.git`, or `<repo>/.git/worktrees/<name>`.
pub fn worktree_actor(clone_id: &str, git_dir: &Path) -> String {
    let linked = git_dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|parent| parent == "worktrees");
    let name = match git_dir.file_name() {
        Some(name) if linked => name.to_string_lossy().into_owned(),
        _ => "main".to_string(),
    };
    format!("{}/{}", clone_id, name)
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
    fn the_main_worktree_actor_is_the_clone_id_and_main() {
        assert_eq!(
            worktree_actor("tsk-1a2b3c4d", Path::new("/r/.git")),
            "tsk-1a2b3c4d/main"
        );
    }

    #[test]
    fn a_linked_worktree_actor_is_the_clone_id_and_the_worktree_name() {
        assert_eq!(
            worktree_actor("tsk-1a2b3c4d", Path::new("/r/.git/worktrees/feature-x")),
            "tsk-1a2b3c4d/feature-x"
        );
    }

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
