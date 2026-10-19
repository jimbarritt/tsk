//! The documents branch of a Dolt ledger. Dolt owns the whole tree of the ledger ref, so
//! documents cannot live there. They are plain files on a second branch, `tsk/docs`
//! (`ledgers/<id>-docs` on a nexus), checked out as a detached linked worktree beside the
//! Dolt database. The worktree, the refresh and the compare-and-swap push are in
//! [`super::fetch`] and [`super::push`].
//! There is no `.tsk-ledger.toml` on the documents branch.

use std::path::{Path, PathBuf};

use super::fetch::{
    add_worktree, ensure_worktree_of_repo, fetch_tip, refresh_worktree, INITIAL_INDEX,
};
use super::git::Git;
use super::location::{docs_worktree_path, LedgerLocation, Repo};
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
}

/// Where this clone's documents worktree is, or would be.
pub fn worktree_path(repo: &Repo, state_root: &Path) -> Result<PathBuf, String> {
    Ok(docs_worktree_path(state_root, &repo.ensure_clone_id()?))
}

/// Fetch the documents branch and bring the worktree up to it, creating the worktree when
/// it is absent. A new branch starts from an orphan commit holding `README.md` and the
/// seed `index.md`. Refuses a dirty worktree, unless `keep_local` is set, and leaves unpushed commits as they are.
pub fn fetch(
    repo: &Repo,
    state_root: &Path,
    location: &LedgerLocation,
    keep_local: bool,
) -> Result<DocsFetch, String> {
    let clone_id = repo.ensure_clone_id()?;
    let path = docs_worktree_path(state_root, &clone_id);
    fetch_at(repo, location, &path, keep_local)
}

/// [`fetch`] with the worktree at `path`.
pub fn fetch_at(
    repo: &Repo,
    location: &LedgerLocation,
    path: &Path,
    keep_local: bool,
) -> Result<DocsFetch, String> {
    let tip = fetch_tip(&repo.git, &location.docs())?;
    fetch_at_tip(repo, location, path, keep_local, tip)
}

/// [`fetch`] for a caller that already holds the documents tip: the commit origin's branch
/// is at, with its objects in the repository, or `None` for a branch origin does not hold.
pub fn fetch_with_tip(
    repo: &Repo,
    state_root: &Path,
    location: &LedgerLocation,
    keep_local: bool,
    tip: Option<String>,
) -> Result<DocsFetch, String> {
    let clone_id = repo.ensure_clone_id()?;
    let path = docs_worktree_path(state_root, &clone_id);
    fetch_at_tip(repo, location, &path, keep_local, tip)
}

fn fetch_at_tip(
    repo: &Repo,
    location: &LedgerLocation,
    path: &Path,
    keep_local: bool,
    tip: Option<String>,
) -> Result<DocsFetch, String> {
    let docs = location.docs();
    let Some(commit) = tip else {
        if path.exists() {
            ensure_worktree_of_repo(&Git::new(path), &repo.common_dir)?;
        } else {
            let commit = create_initial_commit(&repo.git)?;
            add_worktree(repo, path, &commit)?;
        }
        return Ok(DocsFetch {
            location: docs,
            path: path.to_path_buf(),
            pending: Vec::new(),
            unpublished: true,
        });
    };
    let pending = if path.exists() {
        refresh_worktree(repo, path, &commit, keep_local)?
    } else {
        add_worktree(repo, path, &commit)?;
        Vec::new()
    };
    Ok(DocsFetch {
        location: docs,
        path: path.to_path_buf(),
        pending,
        unpublished: false,
    })
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
    git.run_line([
        "commit-tree",
        tree.as_str(),
        "-m",
        "Create the documents branch",
    ])
}

/// True when `body` is one of the files a new documents branch starts with.
pub fn is_seed(path: &str, body: &str) -> bool {
    (path == README_FILE && body == README_TEXT) || (path == INDEX_FILE && body == INITIAL_INDEX)
}

/// Commit every change in the documents worktree and push it with a compare and swap,
/// rebasing onto a newer remote tip. `HEAD` already on the remote pushes nothing.
pub fn push(
    repo: &Repo,
    location: &LedgerLocation,
    path: &Path,
    message: &str,
    known_tip: Option<Option<String>>,
    on_retry: &mut dyn FnMut(usize, &PushStatus),
) -> Result<PushOutcome, String> {
    push::push_worktree(repo, &location.docs(), path, message, known_tip, on_retry)
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
