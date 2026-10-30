use std::path::{Path, PathBuf};

use crate::ledger::location::Repo;
use crate::ledger::store::{self, BindingKind, LedgerStore, RefreshOutcome};
use crate::ledger::{fetch_keeping_local_reporting, push_reporting};

pub const CLOUD_SESSION_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";
pub const MARKER_FILE: &str = "tsk-thread-id";
/// The file in the git directory that holds the URN of the session that runs in this code
/// worktree, written at session start. A cloud session needs none: its URN comes from the
/// environment. Local and never pushed, like the skip marker.
pub const SESSION_URN_FILE: &str = "tsk-session";

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
        let git_dir = repo.git_dir.clone();
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

    /// The URN of the session `tsk` runs in (ADR 0029): from the environment in a cloud
    /// session, else from the file the session start wrote. `None` when neither is known.
    pub fn session_urn(&self) -> Option<String> {
        if let Some(id) = &self.cloud_session {
            return Some(crate::sessions::session_urn(
                crate::sessions::harness_name(
                    crate::thread::session_start::HookFormat::Claude,
                    true,
                ),
                id,
            ));
        }
        let text = std::fs::read_to_string(self.git_dir.join(SESSION_URN_FILE)).ok()?;
        let urn = text.trim();
        urn.starts_with(crate::sessions::URN_PREFIX)
            .then(|| urn.to_string())
    }

    /// Record the URN of the session that runs in this code worktree, for later commands of
    /// the same session. A cloud session writes nothing.
    pub fn remember_session_urn(&self, urn: &str) -> Result<(), String> {
        if self.cloud_session.is_some() {
            return Ok(());
        }
        let path = self.git_dir.join(SESSION_URN_FILE);
        std::fs::write(&path, format!("{}\n", urn))
            .map_err(|e| format!("error: could not write {}: {}", path.display(), e))
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

    /// The value a pause entry records in `written_by`: the session URN when the session is
    /// known (ADR 0030), else the instance key (the session ID, or
    /// `<clone-id>/<worktree-name>`), which tells two clones' main worktrees apart. A
    /// worktree with no clone ID yet falls back to the instance URN, which marks the writer
    /// as unknown.
    pub fn written_by(&self) -> Result<String, String> {
        if let Some(urn) = self.session_urn() {
            return Ok(urn);
        }
        Ok(match self.actor_key()? {
            Some((actor, _)) => actor,
            None => self.actor_urn(),
        })
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

/// The URN of a harness instance: `urn:tsk:instance:<kind>:<key>`, where `<kind>` is `cloud`
/// or `worktree` and `<key>` is the instance key (ADR 0029).
pub fn instance_urn(kind: BindingKind, key: &str) -> String {
    format!("urn:tsk:instance:{}:{}", kind.as_str(), key)
}

/// The URN of an instance that has no instance key: a code worktree of a clone with no clone
/// ID. The key part is the basename of the git directory (`.git` for a main code worktree),
/// so the URN names an unknown clone. It is the fallback value of `written_by`.
pub fn actor_urn(cloud_session: Option<&str>, git_dir: &Path) -> String {
    match cloud_session {
        Some(id) => instance_urn(BindingKind::Cloud, id),
        None => instance_urn(
            BindingKind::Worktree,
            &git_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
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
    fn an_instance_urn_names_the_kind_and_the_key() {
        assert_eq!(
            instance_urn(BindingKind::Cloud, "cse_01abc"),
            "urn:tsk:instance:cloud:cse_01abc"
        );
        assert_eq!(
            instance_urn(BindingKind::Worktree, "tsk-1a2b3c4d/main"),
            "urn:tsk:instance:worktree:tsk-1a2b3c4d/main"
        );
    }

    #[test]
    fn cloud_session_urn_names_the_session() {
        assert_eq!(
            actor_urn(Some("cse_01abc"), Path::new("/r/.git")),
            "urn:tsk:instance:cloud:cse_01abc"
        );
    }

    #[test]
    fn main_worktree_urn_has_dot_git_as_its_key() {
        assert_eq!(
            actor_urn(None, Path::new("/r/.git")),
            "urn:tsk:instance:worktree:.git"
        );
    }

    #[test]
    fn linked_worktree_urn_has_the_worktree_name_as_its_key() {
        assert_eq!(
            actor_urn(None, Path::new("/r/.git/worktrees/feature-x")),
            "urn:tsk:instance:worktree:feature-x"
        );
    }
}
