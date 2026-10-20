//! The lightweight up-to-date check. One `git ls-remote` names the commits origin holds for
//! the ledger ref and the documents ref, in one connection. The commits this clone last
//! fetched sit in a small file in the clone's state directory. When the two agree for a ref,
//! the fetch of that ref has nothing to bring and is skipped.
//!
//! The file records a commit only after the fetch that brought it succeeded, and the commit
//! is never newer than what that fetch brought: for the Dolt data it is the head the
//! `ls-remote` named before the fetch ran. A record that is older than the fetched state
//! costs one needless fetch. A record that is newer would skip a fetch that is due, so the
//! code never writes one. A push to a ref the clone did not fetch afterwards clears its
//! record, except for the documents, where the pushed commit is known exactly.

use std::path::{Path, PathBuf};

use super::git::Git;
use super::location::LedgerLocation;

/// The file, beside the ledger database and the documents worktree.
pub const TIPS_FILE: &str = "remote-tips";

/// What origin holds for the two refs of a ledger. `None` means the ref is absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteHeads {
    pub ledger: Option<String>,
    pub docs: Option<String>,
}

/// Ask origin for both refs in one `git ls-remote`. A failure to reach origin is an error
/// the caller may answer by fetching as before.
pub fn remote_heads(git: &Git, location: &LedgerLocation) -> Result<RemoteHeads, String> {
    let docs = location.docs();
    let (args, output) = git.output([
        "ls-remote",
        location.remote(),
        location.ref_name(),
        docs.ref_name(),
    ])?;
    if !output.status.success() {
        return Err(super::git::failure_message(&args, &output));
    }
    Ok(parse_heads(
        &String::from_utf8_lossy(&output.stdout),
        location.ref_name(),
        docs.ref_name(),
    ))
}

/// The local refs a combined fetch writes. They keep the fetched commits reachable, and
/// the next combined fetch overwrites them.
pub const FETCHED_LEDGER_REF: &str = "refs/tsk-fetch/ledger";
pub const FETCHED_DOCS_REF: &str = "refs/tsk-fetch/docs";

/// What the refusal of a fetch says when origin lacks one of the refs asked for.
const MISSING_REF_TEXT: &str = "couldn't find remote ref";

/// Why a combined fetch did not bring the commits.
#[derive(Debug, PartialEq, Eq)]
pub enum FetchFailure {
    /// Origin lacks the ledger ref or the documents ref. Asking with `ls-remote` tells which.
    RefMissing,
    /// Origin could not be reached, or the fetch failed for another reason.
    Failed(String),
}

/// Fetch the ledger ref and the documents ref in one `git fetch`, and name the commits
/// they are at. For a caller that would have to fetch both anyway: the commits are then in
/// the repository, and one connection did the work of the `ls-remote` and two fetches.
pub fn fetch_heads(git: &Git, location: &LedgerLocation) -> Result<RemoteHeads, FetchFailure> {
    let docs = location.docs();
    let ledger_spec = format!("+{}:{}", location.ref_name(), FETCHED_LEDGER_REF);
    let docs_spec = format!("+{}:{}", docs.ref_name(), FETCHED_DOCS_REF);
    let (args, output) = git
        .output([
            "fetch",
            "--quiet",
            "--no-tags",
            location.remote(),
            ledger_spec.as_str(),
            docs_spec.as_str(),
        ])
        .map_err(FetchFailure::Failed)?;
    if !output.status.success() {
        let failure = super::git::failure_message(&args, &output);
        return Err(if failure.contains(MISSING_REF_TEXT) {
            FetchFailure::RefMissing
        } else {
            FetchFailure::Failed(failure)
        });
    }
    let ledger_commit = format!("{}^{{commit}}", FETCHED_LEDGER_REF);
    let docs_commit = format!("{}^{{commit}}", FETCHED_DOCS_REF);
    let listing = git
        .run(["rev-parse", ledger_commit.as_str(), docs_commit.as_str()])
        .map_err(FetchFailure::Failed)?;
    let mut commits = listing.lines().map(str::to_string);
    match (commits.next(), commits.next()) {
        (Some(ledger), Some(docs)) => Ok(RemoteHeads {
            ledger: Some(ledger),
            docs: Some(docs),
        }),
        _ => Err(FetchFailure::Failed(
            "error: git rev-parse did not name the two fetched refs".to_string(),
        )),
    }
}

/// Read the `ls-remote` listing. A line counts only when its ref name is exactly the one
/// asked for: `ls-remote` also lists refs that end in the pattern.
pub fn parse_heads(listing: &str, ledger_ref: &str, docs_ref: &str) -> RemoteHeads {
    let mut heads = RemoteHeads {
        ledger: None,
        docs: None,
    };
    for line in listing.lines() {
        let mut fields = line.split_whitespace();
        let (Some(commit), Some(name)) = (fields.next(), fields.next()) else {
            continue;
        };
        if name == ledger_ref {
            heads.ledger = Some(commit.to_string());
        } else if name == docs_ref {
            heads.docs = Some(commit.to_string());
        }
    }
    heads
}

/// The commits this clone last fetched, one per remote and ref.
pub struct Tips {
    file: PathBuf,
}

impl Tips {
    /// The record of the clone whose state directory is `clone_dir`
    /// (`<state>/repos/<clone-id>`).
    pub fn new(clone_dir: &Path) -> Tips {
        Tips {
            file: clone_dir.join(TIPS_FILE),
        }
    }

    fn read(&self) -> Vec<(String, String, String)> {
        let Ok(text) = std::fs::read_to_string(&self.file) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| {
                let mut fields = line.splitn(3, '\t');
                Some((
                    fields.next()?.to_string(),
                    fields.next()?.to_string(),
                    fields.next()?.to_string(),
                ))
            })
            .collect()
    }

    /// The commit recorded for `ref_name` on `remote`, if any.
    pub fn get(&self, remote: &str, ref_name: &str) -> Option<String> {
        self.read()
            .into_iter()
            .find(|(r, n, _)| r == remote && n == ref_name)
            .map(|(_, _, commit)| commit)
    }

    /// Record the commit a fetch brought, or forget the ref with `None`. A failure to write
    /// is ignored: the next command then fetches.
    pub fn set(&self, remote: &str, ref_name: &str, commit: Option<&str>) {
        let mut entries: Vec<(String, String, String)> = self
            .read()
            .into_iter()
            .filter(|(r, n, _)| !(r == remote && n == ref_name))
            .collect();
        if let Some(commit) = commit {
            entries.push((remote.to_string(), ref_name.to_string(), commit.to_string()));
        }
        let text: String = entries
            .iter()
            .map(|(r, n, c)| format!("{}\t{}\t{}\n", r, n, c))
            .collect();
        if let Some(parent) = self.file.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return;
            }
        }
        let temp = self
            .file
            .with_extension(format!("tmp{}", std::process::id()));
        if std::fs::write(&temp, text).is_ok() && std::fs::rename(&temp, &self.file).is_err() {
            let _ = std::fs::remove_file(&temp);
        }
    }
}

/// True when `commit` is in `git`'s object database.
pub fn commit_is_local(git: &Git, commit: &str) -> bool {
    let spec = format!("{}^{{commit}}", commit);
    git.output(["cat-file", "-e", spec.as_str()])
        .is_ok_and(|(_, output)| output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = "refs/heads/tsk/ledger";
    const DOCS: &str = "refs/heads/tsk/docs";

    #[test]
    fn the_listing_gives_the_commit_of_each_exact_ref() {
        let listing = format!(
            "{}\t{}\n{}\t{}\n",
            "a".repeat(40),
            LEDGER,
            "b".repeat(40),
            DOCS
        );
        let heads = parse_heads(&listing, LEDGER, DOCS);
        assert_eq!(heads.ledger.as_deref(), Some("a".repeat(40).as_str()));
        assert_eq!(heads.docs.as_deref(), Some("b".repeat(40).as_str()));
    }

    #[test]
    fn a_ref_that_only_ends_in_the_pattern_is_not_the_ref() {
        let listing = format!("{}\trefs/heads/x/refs/heads/tsk/ledger\n", "a".repeat(40));
        let heads = parse_heads(&listing, LEDGER, DOCS);
        assert_eq!(heads.ledger, None);
        assert_eq!(heads.docs, None);
        assert_eq!(
            parse_heads("", LEDGER, DOCS),
            RemoteHeads {
                ledger: None,
                docs: None
            }
        );
    }

    fn run_git(dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn a_combined_fetch_names_both_commits_and_says_why_it_failed() {
        let root = tempfile::tempdir().unwrap();
        let origin = root.path().join("origin.git");
        let work = root.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        run_git(root.path(), &["init", "-q", "--bare", "origin.git"]);
        run_git(&work, &["init", "-q", "-b", "main"]);
        run_git(
            &work,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        run_git(&work, &["commit", "-q", "--allow-empty", "-m", "ledger"]);
        let ledger = run_git(&work, &["rev-parse", "HEAD"]);
        run_git(
            &work,
            &["push", "-q", "origin", "HEAD:refs/heads/tsk/ledger"],
        );
        run_git(&work, &["commit", "-q", "--allow-empty", "-m", "docs"]);
        let docs = run_git(&work, &["rev-parse", "HEAD"]);
        run_git(&work, &["push", "-q", "origin", "HEAD:refs/heads/tsk/docs"]);

        // A clone that has neither commit gets both, and they are in its repository.
        let clone = root.path().join("clone");
        std::fs::create_dir_all(&clone).unwrap();
        run_git(&clone, &["init", "-q", "-b", "main"]);
        run_git(
            &clone,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        let git = Git::new(&clone);
        let location = LedgerLocation::in_repo();
        let heads = fetch_heads(&git, &location).unwrap();
        assert_eq!(heads.ledger.as_deref(), Some(ledger.as_str()));
        assert_eq!(heads.docs.as_deref(), Some(docs.as_str()));
        assert!(commit_is_local(&git, &ledger));
        assert!(commit_is_local(&git, &docs));
        // The refs hold the commits, and no tag came with them.
        assert_eq!(run_git(&clone, &["rev-parse", FETCHED_DOCS_REF]), docs);
        assert_eq!(run_git(&clone, &["tag", "--list"]), "");

        // A second fetch with nothing new names the same commits.
        assert_eq!(fetch_heads(&git, &location).unwrap(), heads);

        // Origin without the documents ref: the failure says a ref is missing.
        run_git(&work, &["push", "-q", "origin", ":refs/heads/tsk/docs"]);
        assert_eq!(
            fetch_heads(&git, &location).unwrap_err(),
            FetchFailure::RefMissing
        );

        // An origin that cannot be reached is another failure.
        run_git(
            &clone,
            &["remote", "set-url", "origin", "/no/such/origin.git"],
        );
        assert!(matches!(
            fetch_heads(&git, &location),
            Err(FetchFailure::Failed(_))
        ));
    }

    #[test]
    fn a_tip_is_recorded_replaced_and_forgotten_per_remote_and_ref() {
        let dir = tempfile::tempdir().unwrap();
        let tips = Tips::new(&dir.path().join("repos").join("id"));
        assert_eq!(tips.get("origin", LEDGER), None);
        tips.set("origin", LEDGER, Some("1111"));
        tips.set("origin", DOCS, Some("2222"));
        tips.set("https://nexus", LEDGER, Some("3333"));
        assert_eq!(tips.get("origin", LEDGER).as_deref(), Some("1111"));
        assert_eq!(tips.get("origin", DOCS).as_deref(), Some("2222"));
        tips.set("origin", LEDGER, Some("4444"));
        assert_eq!(tips.get("origin", LEDGER).as_deref(), Some("4444"));
        assert_eq!(tips.get("https://nexus", LEDGER).as_deref(), Some("3333"));
        tips.set("origin", LEDGER, None);
        assert_eq!(tips.get("origin", LEDGER), None);
        assert_eq!(tips.get("origin", DOCS).as_deref(), Some("2222"));
    }
}
