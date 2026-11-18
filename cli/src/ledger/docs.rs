//! The documents branch of a Dolt ledger. Dolt owns the whole tree of the ledger ref, so
//! documents cannot live there. They are plain files on a second branch, `tsk/docs`
//! (`ledgers/<id>/docs` on a nexus), checked out as a detached linked worktree of the ledger
//! home's bare clone, beside the Dolt database. The worktree, the refresh and the
//! compare-and-swap push are in
//! [`super::fetch`] and [`super::push`].
//! There is no `.tsk-ledger.toml` on the documents branch.

use std::path::{Path, PathBuf};

use super::fetch::{
    add_worktree, ensure_worktree_of, fetch_tip, refresh_worktree, DOCS_CREATION_MESSAGE,
    INITIAL_INDEX,
};
use super::git::Git;
use super::home::Home;
use super::location::LedgerLocation;
use super::push::{self, PushOutcome, PushStatus};

/// The `README.md` a new documents branch starts with.
pub const README_TEXT: &str = "Documents of the tsk ledger. Written by tsk ledger doc put.\n";
pub const README_FILE: &str = "README.md";
const INDEX_FILE: &str = "index.md";

/// What a refresh of the documents worktree found.
pub struct DocsFetch {
    /// The documents location (the docs ref on the ledger's remote).
    pub location: LedgerLocation,
    pub path: PathBuf,
    pub pending: Vec<String>,
    /// True when the remote holds no documents branch yet.
    pub unpublished: bool,
    /// True when the worktree held only its creation commit and was replaced by the
    /// remote branch.
    pub replaced: bool,
}

/// Where the documents worktree of the ledger home is, or would be.
pub fn worktree_path(home: &Home) -> PathBuf {
    home.docs()
}

/// Fetch the documents branch and bring the worktree up to it, creating the worktree when
/// it is absent. A new branch starts from an orphan commit holding `README.md` and the
/// seed `index.md`. Refuses a dirty worktree, unless `keep_local` is set, and leaves unpushed commits as they are.
pub fn fetch(
    home: &Home,
    location: &LedgerLocation,
    keep_local: bool,
) -> Result<DocsFetch, String> {
    let tip = fetch_tip(&home.git(), &location.docs())?;
    fetch_at_tip(home, location, keep_local, tip)
}

/// [`fetch`] for a caller that already holds the documents tip: the commit origin's branch
/// is at, with its objects in the ledger clone, or `None` for a branch origin does not hold.
pub fn fetch_with_tip(
    home: &Home,
    location: &LedgerLocation,
    keep_local: bool,
    tip: Option<String>,
) -> Result<DocsFetch, String> {
    fetch_at_tip(home, location, keep_local, tip)
}

fn fetch_at_tip(
    home: &Home,
    location: &LedgerLocation,
    keep_local: bool,
    tip: Option<String>,
) -> Result<DocsFetch, String> {
    let docs = location.docs();
    let path = home.docs();
    let git = home.git();
    let Some(commit) = tip else {
        if path.exists() {
            ensure_worktree_of(&Git::new(&path), &home.git_dir())?;
        } else {
            // A new documents branch is never started while the ledger holds one under an
            // older or a backup name: that would fork its documents.
            refuse_over_older_ref(&git, location)?;
            let commit = create_initial_commit(&git)?;
            add_worktree(&git, &path, &commit)?;
        }
        return Ok(DocsFetch {
            location: docs,
            path,
            pending: Vec::new(),
            unpublished: true,
            replaced: false,
        });
    };
    let (pending, replaced) = if path.exists() {
        refresh_worktree(&home.git_dir(), &path, &commit, keep_local)?
    } else {
        add_worktree(&git, &path, &commit)?;
        (Vec::new(), false)
    };
    Ok(DocsFetch {
        location: docs,
        path,
        pending,
        unpublished: false,
        replaced,
    })
}

/// Stop a first documents commit while the remote holds the documents of this ledger under
/// the name of tsk 0.69.0 and earlier, or under a backup name (ADR 0035).
fn refuse_over_older_ref(git: &Git, location: &LedgerLocation) -> Result<(), String> {
    let (Some((old, _)), Some(backup)) = (
        location.legacy_ref_names().map(|(_, docs)| (docs, ())),
        location.backup_ref_names(),
    ) else {
        return Ok(());
    };
    for name in [old, backup.1] {
        let (_, output) =
            git.output(["ls-remote", "--exit-code", location.remote(), name.as_str()])?;
        if output.status.code() == Some(0) {
            return Err(format!(
                "error: {} on {} holds the documents of this ledger under the name {}, which a move to the new names has not finished.\n       \
                 Run tsk ledger fetch again to finish it, with write access to the nexus. tsk does not start a second documents branch",
                location.docs().ref_name(),
                location.label(),
                name
            ));
        }
    }
    Ok(())
}

/// An orphan commit holding `README.md` and the seed `index.md`, made with git plumbing
/// so no working tree is needed.
pub fn create_initial_commit(git: &Git) -> Result<String, String> {
    let readme = git.run_stdin(["hash-object", "-w", "--stdin"], README_TEXT)?;
    let index = git.run_stdin(["hash-object", "-w", "--stdin"], INITIAL_INDEX)?;
    let listing = format!(
        "100644 blob {}\t{}\n100644 blob {}\t{}\n",
        readme, README_FILE, index, INDEX_FILE
    );
    let tree = git.run_stdin(["mktree"], &listing)?;
    git.run_line(["commit-tree", tree.as_str(), "-m", DOCS_CREATION_MESSAGE])
}

/// True when `body` is one of the files a new documents branch starts with.
pub fn is_seed(path: &str, body: &str) -> bool {
    (path == README_FILE && body == README_TEXT) || (path == INDEX_FILE && body == INITIAL_INDEX)
}

/// Commit every change in the documents worktree and push it with a compare and swap,
/// rebasing onto a newer remote tip. `HEAD` already on the remote pushes nothing.
pub fn push(
    home: &Home,
    location: &LedgerLocation,
    path: &Path,
    message: &str,
    known_tip: Option<Option<String>>,
    on_retry: &mut dyn FnMut(usize, &PushStatus),
) -> Result<PushOutcome, String> {
    push::push_worktree(home, &location.docs(), path, message, known_tip, on_retry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_files_are_recognised_and_other_text_is_not() {
        assert!(is_seed("README.md", README_TEXT));
        assert!(is_seed("index.md", INITIAL_INDEX));
        assert!(!is_seed("index.md", "# Real index\n"));
        assert!(!is_seed("notes.md", README_TEXT));
    }

    #[test]
    fn the_readme_says_what_wrote_the_branch() {
        assert_eq!(
            README_TEXT,
            "Documents of the tsk ledger. Written by tsk ledger doc put.\n"
        );
    }
}
