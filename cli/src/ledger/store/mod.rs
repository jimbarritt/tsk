//! The storage seam for the ledger.
//!
//! Every command that reads or writes ledger content goes through [`LedgerStore`]. The
//! git-file backend in [`files`] keeps the ledger as files in a detached git worktree.
//! The Dolt backend in [`dolt`] (an external `dolt` binary and SQL tables) implements
//! the same trait. No method takes or returns a filesystem path as a way to reach
//! content: documents, threads, continuation entries, cloud session bindings and
//! external events are addressed by logical keys. The few paths in this module
//! (`RefreshOutcome::path`, `LedgerStore::path`) name where the store lives, for the
//! operator to read.

pub mod dolt;
pub mod files;

use std::path::{Component, Path, PathBuf};

use crate::thread::lookup::Lookup;

use super::manifest::ManifestInfo;

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

#[allow(dead_code)]
pub trait LedgerStore {
    // --- Remote synchronisation and location ---

    /// Fetch the remote state and bring the local store up to it, creating the local
    /// store when it is absent. Refuses to overwrite uncommitted local changes. Prints
    /// nothing: the caller reports `pending_note` and `unpublished_note`.
    fn refresh(&self) -> Result<RefreshOutcome, String>;

    /// Where the local store lives. Has no side effects. Fails when the store has no
    /// identity yet (git backend: no clone id).
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

    // --- Manifest ---

    /// Read the manifest of the local store.
    fn manifest(&self) -> Result<ManifestInfo, String>;

    /// Check the local store's manifest: a supported version and, for a nexus ledger,
    /// the expected repo ID. Returns the version.
    fn check_manifest(&self) -> Result<i64, String>;

    // --- Documents: free-form text keyed by a ledger-relative path ---

    fn doc_get(&self, path: &str) -> Result<Option<String>, String>;
    fn doc_put(&self, path: &str, body: &str) -> Result<(), String>;
    /// Every document path starting with `prefix` (a plain string prefix), sorted.
    /// Threads, external events and the manifest are not documents.
    fn doc_list(&self, prefix: &str) -> Result<Vec<String>, String>;
    /// Remove a document. Returns false when it did not exist.
    fn doc_remove(&self, path: &str) -> Result<bool, String>;
    fn doc_exists(&self, path: &str) -> Result<bool, String>;

    // --- Threads ---

    fn thread_exists(&self, id: &str) -> Result<bool, String>;
    /// Create a thread with an empty continuation log and its briefing link. Fails when
    /// the thread exists.
    fn thread_create(&self, id: &str, briefing_path: &str) -> Result<(), String>;
    /// Delete a thread and its continuation log.
    fn thread_delete(&self, id: &str) -> Result<(), String>;
    /// Every thread ID, sorted.
    fn thread_ids(&self) -> Result<Vec<String>, String>;
    /// The briefing path a thread links to, or `None` when it records none.
    fn thread_briefing(&self, id: &str) -> Result<Option<String>, String>;

    // --- Continuation entries ---

    /// Append one JSON line (no trailing newline) to a thread's continuation log.
    fn continuation_append(&self, id: &str, line: &str) -> Result<(), String>;
    /// The stored lines of a thread's continuation log, oldest first, blank lines
    /// skipped. Empty for a thread with no log.
    fn continuation_list(&self, id: &str) -> Result<Vec<String>, String>;

    // --- Cloud session bindings ---

    /// The whole cloud session lookup. Empty when nothing is bound.
    fn cloud_lookup(&self) -> Result<Lookup, String>;
    fn cloud_bind(&self, session: &str, thread_id: &str, registered_at: &str)
        -> Result<(), String>;
    fn cloud_unbind(&self, session: &str) -> Result<(), String>;
    /// Remove every session bound to a thread.
    fn cloud_unbind_thread(&self, thread_id: &str) -> Result<(), String>;

    // --- External events ---

    /// Append envelope text: one or more complete lines, each ending in `\n`.
    fn events_append(&self, lines: &str) -> Result<(), String>;
    /// Every queued line, oldest first, without terminators. `None` when no queue
    /// exists yet.
    fn events_list(&self) -> Result<Option<Vec<String>>, String>;
    /// The number of queued lines.
    fn events_count(&self) -> Result<u64, String>;
    /// The number of queued lines already processed. 0 when there is no watermark.
    fn watermark_get(&self) -> Result<u64, String>;
    fn watermark_set(&self, processed_through: u64, updated_at: &str) -> Result<(), String>;

    // --- Descriptions for error messages ---

    /// A human-readable location of a document.
    fn describe_document(&self, path: &str) -> String;
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

/// Open the ledger store for the repository containing `dir`. The backend is `dolt` by
/// default; `[ledger] backend` in the user config or `TSK_LEDGER_BACKEND` selects `git`.
pub fn open(dir: &Path) -> Result<Box<dyn LedgerStore>, String> {
    match crate::config::ledger_backend_from_env()? {
        crate::config::LedgerBackend::Dolt => {
            dolt::DoltStore::open(dir).map(|store| Box::new(store) as Box<dyn LedgerStore>)
        }
        crate::config::LedgerBackend::Git => {
            files::FilesStore::open(dir).map(|store| Box::new(store) as Box<dyn LedgerStore>)
        }
    }
}

/// Paths a caller may name as documents. Rejects an empty path, an absolute path, a
/// path with a `..` component, and the parts of the ledger that are not documents.
pub fn check_document_path(path: &str) -> Result<(), String> {
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
    let first = parts[0].as_str();
    let reserved = first == "threads"
        || first == "external-events"
        || (parts.len() == 1 && first == super::manifest::MANIFEST_FILE);
    if reserved || parts.iter().any(|part| part == ".git") {
        return Err(format!(
            "error: '{}' is not a document: threads/, external-events/, {} and .git are managed by tsk",
            path,
            super::manifest::MANIFEST_FILE
        ));
    }
    Ok(())
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
    fn document_paths_are_relative_and_exclude_managed_parts() {
        for ok in [
            "index.md",
            "missions/M-X.md",
            "./missions/M-X.md",
            "a/b/c.md",
        ] {
            assert!(check_document_path(ok).is_ok(), "{}", ok);
        }
        for bad in [
            "",
            ".",
            "/etc/passwd",
            "../x.md",
            "a/../../x.md",
            "threads/1234abcd/index.md",
            "threads",
            "external-events/queue.ndjson",
            ".tsk-ledger.toml",
            "./.tsk-ledger.toml",
            ".git",
            "a/.git/config",
        ] {
            assert!(check_document_path(bad).is_err(), "{}", bad);
        }
    }
}
