//! How the Dolt data of a ledger travels between clones. See ADR 0017.
//!
//! - `git-remote`: the ledger ref holds Dolt's own git blobstore, and `dolt push` and
//!   `dolt fetch` talk to origin through git processes that Dolt starts. This is the layout
//!   before 0.21.0, kept as a fallback.
//! - `file-remote`: the ledger ref holds a Dolt file remote, a directory of `manifest`,
//!   table files and zero-byte `LOCK` files. tsk keeps that directory as a linked worktree
//!   of the repository beside the ledger database. The ledger store's `origin` is
//!   `file://<that directory>`, so Dolt never starts git. tsk moves the directory with one
//!   `git fetch` and one plain `git push`.
//!
//! The ref says which layout it holds: a file remote has the file [`MARKER_FILE`] at the top
//! of its tree. The clone's own store says which layout it uses through the URL of its
//! `origin` remote.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::fetch::{add_worktree, ensure_worktree_of_repo};
use super::git::{failure_message, split_nul, Git};
use super::location::{LedgerLocation, Repo};
use super::push::{commit_changes, parse_push_status};
use super::refs::{is_server_refusal, refused_push, LedgerRefs};

/// Set to `git-remote` to make a new ledger use the older layout.
pub const ENV_VAR: &str = "TSK_LEDGER_TRANSPORT";
/// The file at the top of a file remote's tree that names its transport.
pub const MARKER_FILE: &str = ".tsk-transport";
/// What a file remote directory ignores: Dolt's lock files hold no data.
pub const GITIGNORE_FILE: &str = ".gitignore";
pub const GITIGNORE_TEXT: &str = "LOCK\noldgen/LOCK\n";
/// The name of the directory, beside the ledger database, that holds the file remote.
pub const REMOTE_DIR_NAME: &str = "ledger-remote";
/// The file, beside the ledger database, that records the ledger ref of a file-remote store.
/// A git remote records it in Dolt's `repo_state.json`; a file remote has no ref there.
pub const REF_FILE: &str = "ledger-ref";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    GitRemote,
    FileRemote,
}

impl Transport {
    pub fn as_str(self) -> &'static str {
        match self {
            Transport::GitRemote => "git-remote",
            Transport::FileRemote => "file-remote",
        }
    }

    pub fn parse(text: &str) -> Option<Transport> {
        match text.trim() {
            "git-remote" => Some(Transport::GitRemote),
            "file-remote" => Some(Transport::FileRemote),
            _ => None,
        }
    }

    /// The transport a Dolt remote URL belongs to.
    pub fn of_url(url: &str) -> Transport {
        if url.starts_with("file://") {
            Transport::FileRemote
        } else {
            Transport::GitRemote
        }
    }
}

/// The transport the environment forces, or `None` when it is unset.
pub fn forced() -> Result<Option<Transport>, String> {
    forced_from(std::env::var(ENV_VAR).ok().as_deref())
}

fn forced_from(value: Option<&str>) -> Result<Option<Transport>, String> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(text) => Transport::parse(text).map(Some).ok_or_else(|| {
            format!(
                "error: {}=\"{}\" is not a ledger transport; use git-remote or file-remote",
                ENV_VAR, text
            )
        }),
    }
}

/// The transport a new ledger uses: the environment's choice, else the file remote.
pub fn for_new_ledger() -> Result<Transport, String> {
    Ok(forced()?.unwrap_or(Transport::FileRemote))
}

/// The transport the ledger ref holds at `commit`, which must be in `git`'s object database.
pub fn of_commit(git: &Git, commit: &str) -> Result<Transport, String> {
    let listing = git.run(["ls-tree", "--name-only", "-z", commit])?;
    Ok(if split_nul(&listing).contains(&MARKER_FILE) {
        Transport::FileRemote
    } else {
        Transport::GitRemote
    })
}

/// The Dolt URL of a file remote directory.
pub fn file_url(dir: &Path) -> String {
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    format!("file://{}", dir.to_string_lossy())
}

/// An orphan commit holding the two files a file remote directory starts with.
pub fn initial_commit(git: &Git) -> Result<String, String> {
    let marker = git.run_stdin(
        ["hash-object", "-w", "--stdin"],
        &format!("{}\n", Transport::FileRemote.as_str()),
    )?;
    let ignore = git.run_stdin(["hash-object", "-w", "--stdin"], GITIGNORE_TEXT)?;
    let listing = format!(
        "100644 blob {}\t{}\n100644 blob {}\t{}\n",
        ignore, GITIGNORE_FILE, marker, MARKER_FILE
    );
    let tree = git.run_stdin(["mktree"], &listing)?;
    git.run_line([
        "commit-tree",
        tree.as_str(),
        "-m",
        "Create the ledger file remote",
    ])
}

/// Make the directory at `path` a linked worktree of the repository at `commit`: created
/// when absent, else reset to the commit. Anything in it that the commit does not hold is
/// dropped; Dolt sends it again on the next push.
pub fn sync_checkout(repo: &Repo, path: &Path, commit: &str) -> Result<(), String> {
    if path.exists() {
        let wt = Git::new(path);
        ensure_worktree_of_repo(&wt, &repo.common_dir)?;
        wt.run(["reset", "--hard", "--quiet", commit])?;
        wt.run(["clean", "-fdq"])?;
        Ok(())
    } else {
        add_worktree(repo, path, commit)
    }
}

/// What a publish of the directory did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    /// `HEAD` is a commit origin already holds: nothing was pushed.
    Unchanged(String),
    Pushed(String),
    /// Origin's ref is not an ancestor of the commit: origin moved.
    Rejected(String),
}

/// Commit what the directory holds and push the commit to the ledger ref with a plain
/// `git push`: no force, no lease. `known_tip` is the commit origin is known to hold.
pub fn publish(
    location: &LedgerLocation,
    path: &Path,
    message: &str,
    known_tip: Option<&str>,
) -> Result<Published, String> {
    let wt = Git::new(path);
    let committed = commit_changes(&wt, message)?;
    let head = wt.run_line(["rev-parse", "--verify", "HEAD^{commit}"])?;
    if !committed && known_tip == Some(head.as_str()) {
        return Ok(Published::Unchanged(head));
    }
    let refspec = format!("HEAD:{}", location.ref_name());
    let (args, output) = wt.output(["push", "--porcelain", location.remote(), refspec.as_str()])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let custom = location.refs() == LedgerRefs::Custom;
    match parse_push_status(&stdout, location.ref_name()) {
        Some(status) if status.rejected() => {
            if custom && is_server_refusal(&status.summary) {
                return Err(refused_push(location.ref_name(), &status.summary));
            }
            Ok(Published::Rejected(status.summary))
        }
        Some(_) if output.status.success() => Ok(Published::Pushed(head)),
        _ => {
            let failure = failure_message(&args, &output);
            if custom && is_server_refusal(&failure) {
                return Err(refused_push(location.ref_name(), &failure));
            }
            let lower = failure.to_lowercase();
            if ["non-fast-forward", "fetch first", "rejected"]
                .iter()
                .any(|word| lower.contains(word))
            {
                return Ok(Published::Rejected(
                    "[rejected] (non-fast-forward)".to_string(),
                ));
            }
            Err(failure)
        }
    }
}

/// A short random wait before a retry, so two writers that collided do not collide again.
pub fn retry_wait() -> Duration {
    Duration::from_millis(50 + random_below(201))
}

fn random_below(bound: u64) -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    hasher.finish() % bound
}

/// The ledger ref a file-remote store was made for, from the file beside its database.
pub fn read_ref_file(clone_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(clone_dir.join(REF_FILE)).ok()?;
    let name = text.trim();
    (!name.is_empty()).then(|| name.to_string())
}

pub fn write_ref_file(clone_dir: &Path, ref_name: &str) -> Result<(), String> {
    let file = clone_dir.join(REF_FILE);
    std::fs::write(&file, format!("{}\n", ref_name))
        .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
}

/// The directory of the file remote for the clone whose database is at `db_path`.
pub fn remote_dir_beside(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(|dir| dir.join(REMOTE_DIR_NAME))
        .unwrap_or_else(|| PathBuf::from(REMOTE_DIR_NAME))
}

/// The URL of the `origin` remote that Dolt's `repo_state.json` in `db_path` records.
pub fn store_url(db_path: &Path) -> Option<String> {
    let state = std::fs::read_to_string(db_path.join(".dolt").join("repo_state.json")).ok()?;
    let state: serde_json::Value = serde_json::from_str(&state).ok()?;
    state
        .get("remotes")?
        .get(super::dolt::REMOTE)?
        .get("url")?
        .as_str()
        .map(str::to_string)
}

/// Point the store's `origin` remote at the file remote in `dir`, and record `ref_name` as
/// its ledger ref. The remote-tracking branch is gone until the next `dolt fetch`. Dolt's
/// cache of the old blobstore remote is dropped. The commits of the store stay.
pub fn point_store_at_file_remote(
    db: &super::dolt::Dolt,
    dir: &Path,
    ref_name: &str,
) -> Result<(), String> {
    db.run(["remote", "remove", super::dolt::REMOTE])?;
    db.run(["remote", "add", super::dolt::REMOTE, file_url(dir).as_str()])?;
    if let Some(clone_dir) = db.dir().parent() {
        write_ref_file(clone_dir, ref_name)?;
    }
    let _ = std::fs::remove_dir_all(db.dir().join(".dolt").join("git-remote-cache"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transports_parse_print_and_come_from_urls() {
        assert_eq!(Transport::parse("git-remote"), Some(Transport::GitRemote));
        assert_eq!(
            Transport::parse(" file-remote\n"),
            Some(Transport::FileRemote)
        );
        assert_eq!(Transport::parse("git"), None);
        assert_eq!(Transport::FileRemote.as_str(), "file-remote");
        assert_eq!(
            Transport::of_url("file:///s/tsk/repos/x/ledger-remote"),
            Transport::FileRemote
        );
        assert_eq!(Transport::of_url("git+file:///o.git"), Transport::GitRemote);
        assert_eq!(
            Transport::of_url("git+https://example.test/o"),
            Transport::GitRemote
        );
    }

    #[test]
    fn the_environment_forces_a_transport_or_is_refused() {
        assert_eq!(forced_from(None).unwrap(), None);
        assert_eq!(forced_from(Some("  ")).unwrap(), None);
        assert_eq!(
            forced_from(Some("git-remote")).unwrap(),
            Some(Transport::GitRemote)
        );
        let err = forced_from(Some("ftp")).unwrap_err();
        assert!(err.contains("TSK_LEDGER_TRANSPORT"), "{}", err);
    }

    #[test]
    fn the_retry_wait_is_between_50_and_250_ms() {
        for _ in 0..50 {
            let wait = retry_wait();
            assert!(wait >= Duration::from_millis(50), "{:?}", wait);
            assert!(wait <= Duration::from_millis(250), "{:?}", wait);
        }
    }

    #[test]
    fn the_ref_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_ref_file(dir.path()), None);
        write_ref_file(dir.path(), "refs/tsk/ledger").unwrap();
        assert_eq!(
            read_ref_file(dir.path()).as_deref(),
            Some("refs/tsk/ledger")
        );
    }

    #[test]
    fn the_remote_directory_sits_beside_the_database() {
        assert_eq!(
            remote_dir_beside(Path::new("/s/tsk/repos/x/ledger-db")),
            PathBuf::from("/s/tsk/repos/x/ledger-remote")
        );
    }
}
