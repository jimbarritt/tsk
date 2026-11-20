//! `tsk ledger erase`: remove a commit of the documents branch, and every file it added or
//! changed, from the whole documents history, then force-push the rewritten branch.
//!
//! A rewrite alone is not enough: a clone that still holds the old history would rebase its
//! local commits onto the new tip at its next push, and the rebase replays the erased commit.
//! So the rewritten branch carries a marker file, [`MARKER`], with one line per erase: the
//! erased commit and the documents tip before the erase. A fetch or a push of a clone whose
//! `HEAD` still contains an erased commit replaces its copy with the new tip when it holds
//! no local change, and stops with an error when it does. The marker names commits only,
//! never paths, so it repeats nothing that was erased.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::git::{failure_message, split_nul, Git};
use super::location::LedgerLocation;

/// The file on the documents branch that lists the erased commits.
pub const MARKER: &str = ".tsk-erased";

const MARKER_HEADER: &str =
    "# Commits erased from the documents history by tsk ledger erase: the erased commit, the documents tip before the erase, the time.\n";

/// A file the erased commit added or changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purged {
    pub path: String,
    /// True when the file existed before the erased commit: the whole file leaves the
    /// history, its earlier versions included.
    pub existed_before: bool,
}

/// What an erase does, worked out before anything is pushed. The rewritten commits are
/// objects in the ledger clone only; no ref names them until the push.
#[derive(Debug, Clone)]
pub struct Plan {
    pub commit: String,
    pub subject: String,
    pub old_tip: String,
    pub new_tip: String,
    pub purged: Vec<Purged>,
    /// Commits whose id changes, the marker commit not counted.
    pub rewritten: usize,
    /// Commits that are dropped because they become empty, the erased commit included.
    pub dropped: usize,
}

impl Plan {
    pub fn lines(&self, location: &LedgerLocation) -> Vec<String> {
        let mut lines = vec![format!(
            "erase {} \"{}\" from the documents history of {} ({})",
            short(&self.commit),
            self.subject,
            location.label(),
            location.ref_name()
        )];
        for purged in &self.purged {
            let note = if purged.existed_before {
                "changed by the commit: the whole file leaves the history"
            } else {
                "added by the commit"
            };
            lines.push(format!(
                "  removes from every commit: {} ({})",
                purged.path, note
            ));
        }
        lines.push(format!(
            "  rewrites {} commit(s) and drops {}; the documents tip goes from {} to {}",
            self.rewritten,
            self.dropped,
            short(&self.old_tip),
            short(&self.new_tip)
        ));
        lines
    }
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(12)]
}

/// Work out the rewrite of the history that ends at `tip` without `target` and without the
/// files `target` added or changed. `git` is the ledger clone, where the new objects go.
pub fn plan(git: &Git, tip: &str, target: &str) -> Result<Plan, String> {
    let commit = git
        .run_line(["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", target)])
        .map_err(|_| {
            format!(
                "error: {} is not a commit of the documents branch. tsk ledger erase takes a documents commit id",
                target
            )
        })?;
    if !is_ancestor(git, &commit, tip)? {
        return Err(format!(
            "error: {} is not in the history of the documents branch (tip {}). tsk ledger erase takes a documents commit id",
            short(&commit),
            short(tip)
        ));
    }
    let parents = parents_of(git, &commit)?;
    let Some(first_parent) = parents.first().cloned() else {
        return Err(format!(
            "error: {} is the first commit of the documents branch, which tsk ledger erase does not remove",
            short(&commit)
        ));
    };
    let subject = git.run_line(["log", "-1", "--format=%s", &commit])?;
    let purged = purged_files(git, &first_parent, &commit)?;
    if purged.is_empty() {
        return Err(format!(
            "error: {} adds or changes no document, so there is nothing to erase",
            short(&commit)
        ));
    }

    let index = TempIndex::new(git)?;
    let paths: Vec<&str> = purged.iter().map(|p| p.path.as_str()).collect();
    let listing = git.run(["rev-list", "--topo-order", "--reverse", "--parents", tip])?;
    let mut map: HashMap<String, String> = HashMap::new();
    let mut trees: HashMap<String, String> = HashMap::new();
    let mut rewritten = 0;
    let mut dropped = 0;
    for line in listing.lines() {
        let mut ids = line.split_whitespace().map(str::to_string);
        let Some(id) = ids.next() else { continue };
        let old_parents: Vec<String> = ids.collect();
        let mut new_parents: Vec<String> = Vec::new();
        for parent in &old_parents {
            let mapped = map.get(parent).cloned().unwrap_or_else(|| parent.clone());
            if !new_parents.contains(&mapped) {
                new_parents.push(mapped);
            }
        }
        if id == commit {
            map.insert(id, new_parents[0].clone());
            dropped += 1;
            continue;
        }
        let old_tree = tree_of(git, &id, &mut trees)?;
        let new_tree = index.without(&id, &paths)?;
        if !old_parents.is_empty() && new_parents.len() == 1 {
            let parent = new_parents[0].clone();
            let parent_tree = tree_of(git, &parent, &mut trees)?;
            let was_empty = tree_of(git, &old_parents[0], &mut trees)? == old_tree;
            if new_tree == parent_tree && !was_empty {
                map.insert(id, parent);
                dropped += 1;
                continue;
            }
        }
        if new_tree == old_tree && new_parents == old_parents {
            map.insert(id.clone(), id);
            continue;
        }
        let new_id = copy_commit(git, &id, &new_tree, &new_parents)?;
        trees.insert(new_id.clone(), new_tree);
        map.insert(id, new_id);
        rewritten += 1;
    }
    let rewritten_tip = map
        .get(tip)
        .cloned()
        .ok_or_else(|| format!("error: the rewrite did not reach the tip {}", short(tip)))?;
    let new_tip = marker_commit(git, &index, &rewritten_tip, &commit, tip)?;
    Ok(Plan {
        commit,
        subject,
        old_tip: tip.to_string(),
        new_tip,
        purged,
        rewritten,
        dropped,
    })
}

/// The files `commit` added or changed against `parent`, the marker excluded, with whether
/// each existed before `commit`.
fn purged_files(git: &Git, parent: &str, commit: &str) -> Result<Vec<Purged>, String> {
    let out = git.run([
        "diff-tree",
        "-r",
        "-z",
        "--no-renames",
        "--name-status",
        "--diff-filter=AMT",
        parent,
        commit,
    ])?;
    let fields = split_nul(&out);
    let mut purged = Vec::new();
    for pair in fields.chunks(2) {
        if let [status, path] = pair {
            let path = path.trim_matches('\n').to_string();
            if path.is_empty() || path == MARKER {
                continue;
            }
            let existed_before = !status.trim().starts_with('A');
            purged.push(Purged {
                path,
                existed_before,
            });
        }
    }
    Ok(purged)
}

fn parents_of(git: &Git, commit: &str) -> Result<Vec<String>, String> {
    let line = git.run_line(["rev-list", "--parents", "-n", "1", commit])?;
    Ok(line
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect())
}

fn tree_of(git: &Git, commit: &str, cache: &mut HashMap<String, String>) -> Result<String, String> {
    if let Some(tree) = cache.get(commit) {
        return Ok(tree.clone());
    }
    let tree = git.run_line(["rev-parse", &format!("{}^{{tree}}", commit)])?;
    cache.insert(commit.to_string(), tree.clone());
    Ok(tree)
}

/// A new commit with the author, committer, dates and message of `old`, and the given tree
/// and parents.
fn copy_commit(git: &Git, old: &str, tree: &str, parents: &[String]) -> Result<String, String> {
    let fields = git.run([
        "log",
        "-1",
        "--date=raw",
        "--format=%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd",
        old,
    ])?;
    let fields: Vec<&str> = fields.trim_end_matches('\n').split('\0').collect();
    let [an, ae, ad, cn, ce, cd] = fields.as_slice() else {
        return Err(format!(
            "error: could not read the author of {}",
            short(old)
        ));
    };
    let message = git.run(["log", "-1", "--format=%B", old])?;
    let env = vec![
        ("GIT_AUTHOR_NAME".to_string(), an.to_string()),
        ("GIT_AUTHOR_EMAIL".to_string(), ae.to_string()),
        ("GIT_AUTHOR_DATE".to_string(), ad.to_string()),
        ("GIT_COMMITTER_NAME".to_string(), cn.to_string()),
        ("GIT_COMMITTER_EMAIL".to_string(), ce.to_string()),
        ("GIT_COMMITTER_DATE".to_string(), cd.to_string()),
    ];
    let mut args = vec!["commit-tree".to_string(), tree.to_string()];
    for parent in parents {
        args.push("-p".to_string());
        args.push(parent.clone());
    }
    args.extend(["-F".to_string(), "-".to_string()]);
    git.with_env(env)
        .run_stdin(args, &message)
        .map(|id| id.trim().to_string())
}

/// The commit on top of `base` that adds a line for this erase to the marker.
fn marker_commit(
    git: &Git,
    index: &TempIndex,
    base: &str,
    erased: &str,
    old_tip: &str,
) -> Result<String, String> {
    let existing = git
        .run(["show", &format!("{}:{}", base, MARKER)])
        .unwrap_or_default();
    let mut text = if existing.is_empty() {
        MARKER_HEADER.to_string()
    } else {
        existing
    };
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let now = crate::thread::clock::utc_now();
    text.push_str(&format!("{} {} {}\n", erased, old_tip, now));
    let tree = index.with_file(base, MARKER, &text)?;
    let (git, mut args) = crate::attribution::ledger_git(git)?;
    args.extend(
        [
            "commit-tree",
            tree.as_str(),
            "-p",
            base,
            "-m",
            &format!("Erase {} from the documents history", short(erased)),
        ]
        .map(String::from),
    );
    git.run_line(args)
}

/// A git index file of its own, so the rewrite needs no working tree and touches no index
/// of a worktree.
struct TempIndex {
    git: Git,
    path: PathBuf,
}

impl TempIndex {
    fn new(git: &Git) -> Result<TempIndex, String> {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "tsk-erase-index-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        // update-index needs a work tree, and the ledger clone is bare. An empty directory
        // serves: the index is only ever filled from a tree, never from files.
        let work = path.with_extension("work");
        std::fs::create_dir_all(&work)
            .map_err(|e| format!("error: could not create {}: {}", work.display(), e))?;
        let env = vec![
            (
                "GIT_INDEX_FILE".to_string(),
                path.to_string_lossy().to_string(),
            ),
            (
                "GIT_WORK_TREE".to_string(),
                work.to_string_lossy().to_string(),
            ),
        ];
        Ok(TempIndex {
            git: git.with_env(env),
            path,
        })
    }

    /// The tree of `commit` without `paths`.
    fn without(&self, commit: &str, paths: &[&str]) -> Result<String, String> {
        self.git.run(["read-tree", commit])?;
        let mut input = String::new();
        for path in paths {
            input.push_str(path);
            input.push('\0');
        }
        self.git
            .run_stdin(["update-index", "--force-remove", "-z", "--stdin"], &input)?;
        self.git.run_line(["write-tree"])
    }

    /// The tree of `commit` with `path` set to `body`.
    fn with_file(&self, commit: &str, path: &str, body: &str) -> Result<String, String> {
        self.git.run(["read-tree", commit])?;
        let blob = self.git.run_stdin(["hash-object", "-w", "--stdin"], body)?;
        let info = format!("100644,{},{}", blob.trim(), path);
        self.git
            .run(["update-index", "--add", "--cacheinfo", info.as_str()])?;
        self.git.run_line(["write-tree"])
    }
}

impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir_all(self.path.with_extension("work"));
    }
}

fn is_ancestor(git: &Git, ancestor: &str, descendant: &str) -> Result<bool, String> {
    let (args, output) = git.output(["merge-base", "--is-ancestor", ancestor, descendant])?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(failure_message(&args, &output)),
    }
}

/// How a clone's documents worktree stands against an erase on the remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stale {
    /// `HEAD` contains no erased commit.
    No,
    /// `HEAD` contains an erased commit, and the worktree holds no local commit or change:
    /// the clone can take the new tip with nothing lost.
    ResetSafe { erased: String },
    /// `HEAD` contains an erased commit, and the worktree holds local work.
    Blocked { erased: String, local: usize },
}

/// Check the worktree `wt` against the marker on `remote_tip`.
pub fn stale(wt: &Git, remote_tip: &str) -> Result<Stale, String> {
    if is_ancestor(wt, "HEAD", remote_tip)? {
        return Ok(Stale::No);
    }
    let Ok(text) = wt.run(["show", &format!("{}:{}", remote_tip, MARKER)]) else {
        return Ok(Stale::No);
    };
    let records = parse_marker(&text);
    let present = |id: &str| {
        wt.output(["cat-file", "-e", &format!("{}^{{commit}}", id)])
            .is_ok_and(|(_, o)| o.status.success())
    };
    let mut erased = None;
    for (commit, _) in &records {
        if present(commit) && is_ancestor(wt, commit, "HEAD")? {
            erased = Some(commit.clone());
            break;
        }
    }
    let Some(erased) = erased else {
        return Ok(Stale::No);
    };
    let mut args = vec![
        "rev-list".to_string(),
        "--count".to_string(),
        "HEAD".to_string(),
        "--not".to_string(),
        remote_tip.to_string(),
    ];
    let old_tips: Vec<String> = records
        .iter()
        .map(|(_, tip)| tip.clone())
        .filter(|tip| present(tip))
        .collect();
    if old_tips.is_empty() {
        // The clone never fetched a tip the erase replaced, so its own commits cannot be
        // told from the erased history.
        return Ok(Stale::Blocked { erased, local: 1 });
    }
    args.extend(old_tips);
    let mut local: usize = wt.run_line(args)?.parse().unwrap_or(1);
    if !wt.run(["status", "--porcelain"])?.trim().is_empty() {
        local += 1;
    }
    if local == 0 {
        Ok(Stale::ResetSafe { erased })
    } else {
        Ok(Stale::Blocked { erased, local })
    }
}

/// The records of a marker: (erased commit, documents tip before the erase).
pub fn parse_marker(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(commit), Some(tip)) => Some((commit.to_string(), tip.to_string())),
                _ => None,
            }
        })
        .collect()
}

/// The error for a clone that holds an erased commit and local work.
pub fn blocked_message(label: &str, erased: &str, local: usize, path: &Path) -> String {
    format!(
        "error: the documents history on {} was rewritten by tsk ledger erase, and this clone still holds the erased commit {},\n       \
         with {} local change(s) on top in {}. tsk does not push or merge them: that would publish the erased commit again.\n       \
         Save the documents you changed (tsk ledger status lists them), run tsk ledger discard --yes, then write them again with tsk ledger doc put",
        label,
        short(erased),
        local,
        path.display()
    )
}

/// The note for a clone whose copy was replaced by the rewritten history.
pub fn replaced_note(label: &str, erased: &str) -> String {
    format!(
        "note: the documents history on {} was rewritten by tsk ledger erase (commit {} removed); this clone's copy is replaced by the new history",
        label,
        short(erased)
    )
}

/// Drop the objects nothing reaches any more from the git directory `git_dir`: the reflogs
/// first, which still name the old commits, then a pruning gc.
pub fn prune(git_dir: &Path) -> Result<(), String> {
    let git = Git::new(git_dir);
    git.run(["reflog", "expire", "--expire=now", "--all"])?;
    git.run(["gc", "--quiet", "--prune=now"])?;
    Ok(())
}

/// The directory of a remote on this machine, for a `file://` URL or a path.
pub fn local_remote_dir(remote: &str) -> Option<PathBuf> {
    let path = remote.strip_prefix("file://").unwrap_or(remote);
    if path.contains("://") || path.starts_with("git@") {
        return None;
    }
    let path = PathBuf::from(path);
    path.is_dir().then_some(path)
}

/// True when the commit is still an object in the git directory.
pub fn still_present(git_dir: &Path, commit: &str) -> bool {
    Git::new(git_dir)
        .output(["cat-file", "-e", &format!("{}^{{commit}}", commit)])
        .is_ok_and(|(_, o)| o.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_is_read_without_its_comment_lines() {
        let text = format!(
            "{}abc def 2026-11-20T10:00:00Z\n\nghi jkl 2026-11-20T11:00:00Z\n",
            MARKER_HEADER
        );
        assert_eq!(
            parse_marker(&text),
            vec![
                ("abc".to_string(), "def".to_string()),
                ("ghi".to_string(), "jkl".to_string())
            ]
        );
    }

    #[test]
    fn a_remote_on_this_machine_is_recognised_and_a_url_is_not() {
        let dir = std::env::temp_dir();
        let text = dir.to_string_lossy().to_string();
        assert_eq!(
            local_remote_dir(&format!("file://{}", text)),
            Some(dir.clone())
        );
        assert_eq!(local_remote_dir(&text), Some(dir));
        assert_eq!(local_remote_dir("https://github.com/a/b.git"), None);
        assert_eq!(local_remote_dir("git@github.com:a/b.git"), None);
    }
}
