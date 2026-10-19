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
