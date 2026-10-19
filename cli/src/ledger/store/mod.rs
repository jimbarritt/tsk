//! The storage seam for the ledger.
//!
//! Every command that reads or writes ledger content goes through [`LedgerStore`]. The
//! Dolt backend in [`dolt`] (an external `dolt` binary and SQL tables) is the one
//! implementation. ADR 0015 removed the files backend and kept the trait. No method takes or returns a filesystem path as a way to reach
//! content: documents, threads, continuation entries, bindings and
//! external events are addressed by logical keys. The few paths in this module
//! (`RefreshOutcome::path`, `LedgerStore::path`) name where the store lives, for the
//! operator to read.

pub mod doc_tree;
pub mod dolt;

use std::path::{Component, Path, PathBuf};

use crate::mission::model::Mission;

use super::status::LedgerStatus;

/// What a refresh found. The notes are ready to print to stderr, or `None`.
#[derive(Debug, Clone)]
pub struct RefreshOutcome {
    /// Where the store lives locally.
    pub path: PathBuf,
    /// Names the remote location, for example `origin's refs/heads/tsk/ledger`.
    pub remote_label: String,
    /// The identifier of the remote state the refresh fetched.
    pub remote_head: String,
    /// One line per local change that is not on the remote. Empty when there is none.
    pub pending: Vec<String>,
    /// True when the remote holds no ledger yet and the store is a new local one.
    #[allow(dead_code)]
    pub unpublished: bool,
    pub pending_note: Option<String>,
    pub unpublished_note: Option<String>,
}

/// The outcome of `commit_and_push`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushReport {
    /// True when local changes were committed by this call.
    pub committed: bool,
    /// The identifier of the state the remote holds afterwards.
    pub head: String,
    /// True when the remote already held the local state and nothing was pushed.
    pub already_on_remote: bool,
}

/// What kind of actor a binding names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindingKind {
    /// A Claude Code cloud session. The actor key is the session ID.
    Cloud,
    /// A local code worktree. The actor key is `<clone-id>/<worktree-name>`.
    Worktree,
}

impl BindingKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BindingKind::Cloud => "cloud",
            BindingKind::Worktree => "worktree",
        }
    }

    pub fn parse(text: &str) -> Option<BindingKind> {
        match text {
            "cloud" => Some(BindingKind::Cloud),
            "worktree" => Some(BindingKind::Worktree),
            _ => None,
        }
    }
}

/// One row of the bindings: an actor and the thread it works.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRow {
    pub actor: String,
    pub kind: BindingKind,
    pub thread_id: String,
    pub registered_at: String,
}

/// The mission and briefing a thread stores. A thread made before layout version 5 has
/// no stored mission ID: [`crate::thread::ops::thread_mission`] derives it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThreadInfo {
    pub mission_id: Option<String>,
    pub briefing_path: Option<String>,
}

pub trait LedgerStore {
    // --- Remote synchronisation and location ---

    /// Fetch the remote state and bring the local store up to it, creating the local
    /// store when it is absent. Refuses to overwrite uncommitted local changes. Prints
    /// nothing: the caller reports `pending_note` and `unpublished_note`.
    fn refresh(&self) -> Result<RefreshOutcome, String>;

    /// [`refresh`](LedgerStore::refresh) for `tsk ledger fetch`. The Dolt backend lets a
    /// version 1 ledger through, which every other command refuses until its documents
    /// are moved. Other backends refresh as usual.
    fn refresh_for_fetch(&self) -> Result<RefreshOutcome, String> {
        self.refresh()
    }

    /// [`refresh`](LedgerStore::refresh) for the thread commands. Local changes that are
    /// not committed are left as they are instead of refused, because a thread command
    /// pushes them with its own change, and the push merges the remote state into them.
    fn refresh_keeping_local(&self) -> Result<RefreshOutcome, String> {
        self.refresh()
    }

    /// True when the local store holds changes that are not committed: records or
    /// documents written since the last push.
    fn has_local_changes(&self) -> Result<bool, String> {
        Ok(false)
    }

    /// What the local store holds that origin does not, and how its committed state
    /// stands against origin. Fetches the remote refs into the store's remote-tracking
    /// state and nothing else: it does not merge, reset or touch the working set. When the
    /// fetch fails, `fetch_note` says so and the report comes from what is local.
    fn status(&self) -> Result<LedgerStatus, String>;

    /// Drop every local change not yet pushed, in the data and the documents: uncommitted
    /// changes and local commits origin lacks. Fetches first and resets both sides to
    /// origin's state; when the fetch fails nothing is discarded. Leaves thread state
    /// kept outside the ledger alone.
    fn discard(&self) -> Result<(), String>;

    /// Where the local store lives. Has no side effects. Fails when the store has no
    /// identity yet (no clone id).
    fn path(&self) -> Result<PathBuf, String>;

    /// True when a local store has already been materialised. Has no side effects.
    fn exists_locally(&self) -> Result<bool, String>;

    /// Commit every local change, publish it to the remote with a bounded retry, and
    /// report the outcome. Each human-readable note (retry, nothing to commit, already
    /// on remote) goes to `notes` as it arises. Needs a refreshed local store.
    fn commit_and_push(
        &self,
        message: &str,
        notes: &mut dyn FnMut(&str),
    ) -> Result<PushReport, String>;

    /// The identifier of the local state, written into continuation entries as
    /// `git.ledger.commit`.
    fn head_id(&self) -> Result<String, String>;

    // --- Documents: free-form text keyed by a ledger-relative path ---

    fn doc_get(&self, path: &str) -> Result<Option<String>, String>;
    fn doc_put(&self, path: &str, body: &str) -> Result<(), String>;
    /// Every document path starting with `prefix` (a plain string prefix), sorted.
    /// Threads, external events and the manifest are not documents.
    fn doc_list(&self, prefix: &str) -> Result<Vec<String>, String>;
    /// Remove a document. Returns false when it did not exist.
    fn doc_remove(&self, path: &str) -> Result<bool, String>;
    fn doc_exists(&self, path: &str) -> Result<bool, String>;
    /// The absolute path of a document's file, or `None` when the document does not exist.
    fn doc_path(&self, path: &str) -> Result<Option<PathBuf>, String>;

    // --- Threads ---

    fn thread_exists(&self, id: &str) -> Result<bool, String>;
    /// Create a thread with an empty continuation log, the mission it works (none for a
    /// thread with no mission) and, for a mission with no record, its briefing path.
    /// Fails when the thread exists. The first call on a Dolt ledger below version 5
    /// upgrades it.
    fn thread_create(
        &self,
        id: &str,
        mission_id: Option<&str>,
        briefing_path: Option<&str>,
    ) -> Result<(), String>;
    /// Replace the mission and briefing path of an existing thread. Both `None` detaches
    /// the thread from its mission. Earlier continuation entries are not touched. The
    /// first call on a Dolt ledger below version 5 upgrades it.
    fn thread_set_mission(
        &self,
        id: &str,
        mission_id: Option<&str>,
        briefing_path: Option<&str>,
    ) -> Result<(), String>;
    /// Delete a thread and its continuation log.
    fn thread_delete(&self, id: &str) -> Result<(), String>;
    /// Every thread ID, sorted.
    fn thread_ids(&self) -> Result<Vec<String>, String>;
    /// What a thread stores: the mission it works and the briefing path it links to, each
    /// `None` when the thread records none. An unknown thread reads as neither.
    fn thread_info(&self, id: &str) -> Result<ThreadInfo, String>;

    // --- Continuation entries ---

    /// Append one JSON line (no trailing newline) to a thread's continuation log.
    fn continuation_append(&self, id: &str, line: &str) -> Result<(), String>;
    /// The stored lines of a thread's continuation log, oldest first, blank lines
    /// skipped. Empty for a thread with no log.
    fn continuation_list(&self, id: &str) -> Result<Vec<String>, String>;

    // --- Bindings ---

    /// Every binding, sorted by registration time and then actor. Empty when nothing is
    /// bound. A Dolt ledger below version 8 reads its `cloud_session_bindings` table as
    /// cloud rows and upgrades nothing.
    fn bindings_list(&self) -> Result<Vec<BindingRow>, String>;
    /// Bind an actor to a thread, replacing the actor's earlier binding. The first call on
    /// a Dolt ledger below version 8 upgrades it. The change is local until the caller
    /// pushes it.
    fn binding_set(&self, row: &BindingRow) -> Result<(), String>;
    /// Remove an actor's binding, if it has one.
    fn binding_remove(&self, actor: &str) -> Result<(), String>;
    /// Remove the bindings of several actors in one write. An actor with no binding is
    /// skipped.
    fn bindings_remove(&self, actors: &[String]) -> Result<(), String>;
    /// Remove every binding to a thread, of either kind.
    fn binding_remove_thread(&self, thread_id: &str) -> Result<(), String>;

    // --- External events ---

    /// Append envelope text: one or more complete lines, each ending in `\n`.
    fn events_append(&self, lines: &str) -> Result<(), String>;
    /// Every queued line, oldest first, without terminators. `None` when no queue
    /// exists yet.
    fn events_list(&self) -> Result<Option<Vec<String>>, String>;
    /// The number of queued lines already processed. 0 when there is no watermark.
    fn watermark_get(&self) -> Result<u64, String>;
    fn watermark_set(&self, processed_through: u64, updated_at: &str) -> Result<(), String>;

    // --- Missions ---

    /// Every mission, sorted by ID. A ledger that predates missions has none. A version 1
    /// ledger is refused.
    fn missions_load(&self) -> Result<Vec<Mission>, String>;
    /// Write one mission and its objective outcomes, blockers and document links in the
    /// local store, replacing what the store held for that ID. The first call on a
    /// Dolt ledger below version 5 creates the missing tables and columns and records version 5.
    fn mission_save(&self, mission: &Mission) -> Result<(), String>;

    // --- Descriptions for error messages ---

    /// A human-readable location of a thread's continuation log.
    fn describe_thread(&self, id: &str) -> String;
    /// A human-readable location of the thread collection. Also seeds ID minting.
    fn describe_threads(&self) -> String;

    // --- Session start ---

    /// The command an operator runs to read local changes that are not on the remote,
    /// for the session start warning. `remote_head` is the remote state the refresh
    /// fetched.
    fn pending_inspect_hint(&self, path: &Path, remote_head: &str) -> String;
}

/// Open the ledger store for the repository containing `dir`. A ledger is a Dolt
/// database. `TSK_LEDGER_BACKEND=git` or `[ledger] backend = "git"` in the user config
/// names the removed files backend and stops with an error.
pub fn open(dir: &Path) -> Result<Box<dyn LedgerStore>, String> {
    crate::config::refuse_files_backend_from_env()?;
    dolt::DoltStore::open(dir).map(|store| Box::new(store) as Box<dyn LedgerStore>)
}

/// The components of a relative document path, with `.` dropped. Rejects an empty path,
/// an absolute path and a path with a `..` component.
fn relative_parts(path: &str) -> Result<Vec<String>, String> {
    let p = Path::new(path);
    let mut parts = Vec::new();
    for component in p.components() {
        match component {
            Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return Err(not_relative(path)),
        }
    }
    if path.is_empty() || parts.is_empty() {
        return Err(not_relative(path));
    }
    Ok(parts)
}

/// Paths a caller may name as documents on the `tsk/docs` branch, as the normalised
/// relative path. Only the rules of a file in git apply: no absolute path, no `..`, no
/// `.git` component. A path that is not valid UTF-8 cannot reach here as a `&str`.
pub fn check_docs_path(path: &str) -> Result<String, String> {
    let parts = relative_parts(path)?;
    if parts.iter().any(|part| part == ".git") {
        return Err(format!(
            "error: '{}' is not a document: .git is managed by git",
            path
        ));
    }
    Ok(parts.join("/"))
}

fn not_relative(path: &str) -> String {
    format!(
        "error: document path '{}' must be relative to the ledger root, with no '..'",
        path
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_paths_apply_only_the_git_rules() {
        for (given, normal) in [
            ("index.md", "index.md"),
            ("./missions/M-X.md", "missions/M-X.md"),
            ("threads/1234abcd/index.md", "threads/1234abcd/index.md"),
            (
                "external-events/queue.ndjson",
                "external-events/queue.ndjson",
            ),
            (".tsk-ledger.toml", ".tsk-ledger.toml"),
        ] {
            assert_eq!(check_docs_path(given).unwrap(), normal, "{}", given);
        }
        for bad in [
            "",
            ".",
            "/etc/passwd",
            "../x.md",
            "a/../../x.md",
            ".git",
            "a/.git/config",
        ] {
            assert!(check_docs_path(bad).is_err(), "{}", bad);
        }
    }
}
