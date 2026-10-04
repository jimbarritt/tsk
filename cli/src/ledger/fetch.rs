use std::path::{Path, PathBuf};

use super::git::{failure_message, split_nul, Git};
use super::location::{worktree_path, LedgerLocation, Repo};
use super::manifest::{self, MANIFEST_FILE};

pub struct FetchOutcome {
    pub location: LedgerLocation,
    pub path: PathBuf,
    pub commit: String,
    pub pending: Vec<String>,
    pub unpublished: bool,
}

pub fn fetch(repo: &Repo, state_root: &Path) -> Result<FetchOutcome, String> {
    let location = repo.location(state_root)?;
    let clone_id = repo.ensure_clone_id()?;
    let path = worktree_path(state_root, &clone_id);

    let Some(commit) = fetch_tip(&repo.git, &location)? else {
        return open_unpublished(repo, location, path);
    };
    check_manifest(&repo.git, &commit, &location)?;

    if path.exists() {
        let pending = refresh_worktree(repo, &path, &commit)?;
        Ok(FetchOutcome {
            location,
            path,
            commit,
            pending,
            unpublished: false,
        })
    } else {
        add_worktree(repo, &path, &commit)?;
        Ok(FetchOutcome {
            location,
            path,
            commit,
            pending: Vec::new(),
            unpublished: false,
        })
    }
}

fn open_unpublished(
    repo: &Repo,
    location: LedgerLocation,
    path: PathBuf,
) -> Result<FetchOutcome, String> {
    let commit = if path.exists() {
        let wt = Git::new(&path);
        ensure_worktree_of_repo(&wt, &repo.common_dir)?;
        let head = wt.run_line(["rev-parse", "--verify", "HEAD^{commit}"])?;
        check_manifest(&wt, &head, &location)?;
        head
    } else {
        let commit = create_initial_commit(&repo.git, &location)?;
        add_worktree(repo, &path, &commit)?;
        commit
    };
    Ok(FetchOutcome {
        location,
        path,
        commit,
        pending: Vec::new(),
        unpublished: true,
    })
}

pub const INITIAL_INDEX: &str = "# Ledger index\n";

pub fn create_initial_commit(git: &Git, location: &LedgerLocation) -> Result<String, String> {
    let manifest_text = manifest::initial_text(location.repo_id());
    let manifest_blob = git.run_stdin(["hash-object", "-w", "--stdin"], &manifest_text)?;
    let index_blob = git.run_stdin(["hash-object", "-w", "--stdin"], INITIAL_INDEX)?;
    let listing = format!(
        "100644 blob {}\t{}\n100644 blob {}\tindex.md\n",
        manifest_blob, MANIFEST_FILE, index_blob
    );
    let tree = git.run_stdin(["mktree"], &listing)?;
    git.run_line(["commit-tree", tree.as_str(), "-m", "Create the ledger"])
}

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

pub fn check_manifest(git: &Git, commit: &str, location: &LedgerLocation) -> Result<i64, String> {
    let object = format!("{}:{}", commit, MANIFEST_FILE);
    let (_, output) = git.output(["cat-file", "-e", object.as_str()])?;
    if !output.status.success() {
        return Err(format!(
            "error: {} on {} has no {}; it is not a tsk ledger",
            location.ref_name(),
            location.label(),
            MANIFEST_FILE
        ));
    }
    let text = git.run(["cat-file", "blob", object.as_str()])?;
    manifest::check(&text, location.repo_id())
}

fn add_worktree(repo: &Repo, path: &Path, commit: &str) -> Result<(), String> {
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

fn refresh_worktree(repo: &Repo, path: &Path, commit: &str) -> Result<Vec<String>, String> {
    let wt = Git::new(path);
    ensure_worktree_of_repo(&wt, &repo.common_dir)?;

    let status = wt.run(["status", "--porcelain=v2", "-z"])?;
    if !status.is_empty() {
        return Err(format!(
            "error: {} holds uncommitted changes; refusing to reset over them.\n       \
             Push them with tsk ledger push, or discard them deliberately with: git -C \"{}\" reset --hard\n       \
             To read the path without refreshing, use tsk ledger path",
            path.display(),
            path.display()
        ));
    }

    let pending = pending_commits(&wt, commit)?;
    if pending.is_empty() {
        wt.run(["reset", "--hard", "--quiet", commit])?;
    }
    Ok(pending)
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
}
