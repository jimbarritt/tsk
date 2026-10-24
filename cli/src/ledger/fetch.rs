use std::path::{Path, PathBuf};

use super::git::{failure_message, split_nul, Git};
use super::location::{LedgerLocation, Repo};
use super::manifest::MANIFEST_FILE;

pub struct FetchOutcome {
    pub location: LedgerLocation,
    pub path: PathBuf,
    pub pending: Vec<String>,
    pub unpublished: bool,
}

pub const INITIAL_INDEX: &str = "# Ledger index\n";

/// The message of the one commit a new documents branch starts with.
pub const DOCS_CREATION_MESSAGE: &str = "Create the documents branch";

pub fn fetch_tip(git: &Git, location: &LedgerLocation) -> Result<Option<String>, String> {
    let (args, output) =
        git.output(["fetch", "--quiet", location.remote(), location.ref_name()])?;
    if !output.status.success() {
        if remote_ref_absent(git, location)? {
            return Ok(None);
        }
        return Err(failure_message(&args, &output));
    }
    git.run_line(["rev-parse", "--verify", "--quiet", "FETCH_HEAD^{commit}"])
        .map(Some)
        .map_err(|_| {
            format!(
                "error: no commit in FETCH_HEAD after fetching {} from {}",
                location.ref_name(),
                location.label()
            )
        })
}

fn remote_ref_absent(git: &Git, location: &LedgerLocation) -> Result<bool, String> {
    let (_, output) = git.output([
        "ls-remote",
        "--exit-code",
        location.remote(),
        location.ref_name(),
    ])?;
    Ok(output.status.code() == Some(2))
}

/// What a ledger ref holds, judged from the top level of its tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// `.tsk-ledger.toml` is present: the git-file layout.
    Files,
    /// A `manifest` file and no `.tsk-ledger.toml`: a Dolt database stored in git.
    Dolt,
    Unknown,
}

/// The kind of ledger a commit holds. The commit must be in `git`'s object database.
pub fn ref_kind(git: &Git, commit: &str) -> Result<RefKind, String> {
    let listing = git.run(["ls-tree", "--name-only", "-z", commit])?;
    let names = split_nul(&listing);
    Ok(if names.contains(&MANIFEST_FILE) {
        RefKind::Files
    } else if names.contains(&"manifest") {
        RefKind::Dolt
    } else {
        RefKind::Unknown
    })
}

pub(crate) fn add_worktree(repo: &Repo, path: &Path, commit: &str) -> Result<(), String> {
    if worktree_registered(&repo.git, path)? {
        repo.git.run(["worktree", "prune"])?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    let path_arg = path.to_string_lossy().into_owned();
    repo.git.run([
        "worktree",
        "add",
        "--detach",
        "--quiet",
        path_arg.as_str(),
        commit,
    ])?;
    Ok(())
}

/// Bring the documents worktree up to `commit` and return the commits it holds that are
/// not on it.
///
/// The second value is true when the worktree held only its creation commit and was reset
/// to `commit`: such a worktree has no document to lose.
pub(crate) fn refresh_worktree(
    repo: &Repo,
    path: &Path,
    commit: &str,
    keep_local: bool,
) -> Result<(Vec<String>, bool), String> {
    let wt = Git::new(path);
    ensure_worktree_of_repo(&wt, &repo.common_dir)?;

    let status = wt.run(["status", "--porcelain=v2", "-z"])?;
    if !status.is_empty() {
        if keep_local {
            return Ok((pending_commits(&wt, commit)?, false));
        }
        return Err(format!(
            "error: {} holds uncommitted changes; refusing to reset over them.\n       \
             Push them with tsk ledger push, or drop them deliberately with tsk ledger discard\n       \
             (tsk ledger status shows what would go). To read the path without refreshing, use tsk ledger path",
            path.display()
        ));
    }

    let pending = pending_commits(&wt, commit)?;
    if pending.is_empty() {
        wt.run(["reset", "--hard", "--quiet", commit])?;
        return Ok((pending, false));
    }
    if is_creation_only(&pending) {
        wt.run(["reset", "--hard", "--quiet", commit])?;
        return Ok((Vec::new(), true));
    }
    Ok((pending, false))
}

/// True when the one unpushed commit is the creation commit of a documents branch. The
/// worktree is clean, so it holds the two seed files and nothing else.
fn is_creation_only(pending: &[String]) -> bool {
    matches!(pending, [only] if only.split_once(' ').map(|(_, message)| message) == Some(DOCS_CREATION_MESSAGE))
}

pub fn ensure_worktree_of_repo(wt: &Git, common_dir: &Path) -> Result<(), String> {
    let wt_common = wt
        .run_line(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .ok()
        .map(PathBuf::from);
    let matches = match wt_common {
        Some(found) => canonical(&found) == canonical(common_dir),
        None => false,
    };
    if !matches {
        return Err(format!(
            "error: {} exists but is not a worktree of this clone ({}); move it aside and re-run",
            wt.dir().display(),
            common_dir.display()
        ));
    }
    Ok(())
}

pub fn pending_commits(wt: &Git, commit: &str) -> Result<Vec<String>, String> {
    let range = format!("{}..HEAD", commit);
    let log = wt.run(["log", "-z", "--format=%h %s", range.as_str()])?;
    Ok(split_nul(&log)
        .into_iter()
        .map(|s| s.trim_matches('\n').to_string())
        .collect())
}

fn worktree_registered(git: &Git, path: &Path) -> Result<bool, String> {
    let listing = git.run(["worktree", "list", "--porcelain", "-z"])?;
    Ok(worktree_paths(&listing)
        .iter()
        .any(|listed| same_location(Path::new(listed), path)))
}

pub fn worktree_paths(listing: &str) -> Vec<String> {
    split_nul(listing)
        .into_iter()
        .filter_map(|field| field.strip_prefix("worktree "))
        .map(|p| p.to_string())
        .collect()
}

fn same_location(a: &Path, b: &Path) -> bool {
    a == b || canonical(a) == canonical(b)
}

fn canonical(path: &Path) -> PathBuf {
    if let Ok(full) = std::fs::canonicalize(path) {
        return full;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => canonical(parent).join(name),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_paths_reads_porcelain_z_listing() {
        let listing = "worktree /repo\0HEAD abc\0branch refs/heads/main\0\0\
                       worktree /state/tsk/repos/x/ledger\0HEAD def\0detached\0prunable gitdir file points to non-existent location\0\0";
        assert_eq!(
            worktree_paths(listing),
            vec!["/repo".to_string(), "/state/tsk/repos/x/ledger".to_string()]
        );
    }

    #[test]
    fn worktree_paths_keeps_spaces_in_paths() {
        let listing = "worktree /a b/c\0HEAD abc\0detached\0\0";
        assert_eq!(worktree_paths(listing), vec!["/a b/c".to_string()]);
    }

    #[test]
    fn same_location_resolves_missing_leaf_through_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let real = std::fs::canonicalize(dir.path()).unwrap();
        assert!(same_location(
            &dir.path().join("missing"),
            &real.join("missing")
        ));
        assert!(!same_location(&dir.path().join("a"), &real.join("b")));
    }

    #[test]
    fn only_the_creation_commit_of_the_documents_branch_is_replaceable() {
        let creation = format!("abc1234 {}", DOCS_CREATION_MESSAGE);
        assert!(is_creation_only(std::slice::from_ref(&creation)));
        assert!(!is_creation_only(&[]));
        assert!(!is_creation_only(&["abc1234 Add a note".to_string()]));
        assert!(!is_creation_only(&[
            "def5678 Add a note".to_string(),
            creation
        ]));
    }
}
