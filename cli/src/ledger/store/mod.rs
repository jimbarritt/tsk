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

use crate::mission::model::Mission;
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

/// The mission and briefing a thread stores. A thread made before layout version 5 has
/// no stored mission ID: [`crate::thread::ops::thread_mission`] derives it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThreadInfo {
    pub mission_id: Option<String>,
    pub briefing_path: Option<String>,
}

#[allow(dead_code)]
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
    /// The absolute path of a document's file, or `None` when the document does not exist.
    fn doc_path(&self, path: &str) -> Result<Option<PathBuf>, String>;

    // --- Threads ---

    fn thread_exists(&self, id: &str) -> Result<bool, String>;
    /// Create a thread with an empty continuation log, the mission it works and, for a
    /// mission with no record, its briefing path. Fails when the thread exists. The first
    /// call on a Dolt ledger below version 5 upgrades it.
    fn thread_create(
        &self,
        id: &str,
        mission_id: &str,
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

    // --- Missions ---

    /// Every mission, sorted by ID. A ledger that predates missions has none. A version 1
    /// ledger is refused.
    fn missions_load(&self) -> Result<Vec<Mission>, String>;
    /// Write one mission and its objective outcomes, blockers and document links in the
    /// local store, replacing what the store held for that ID. The first call on a
    /// Dolt ledger below version 5 creates the missing tables and columns and records version 5.
    fn mission_save(&self, mission: &Mission) -> Result<(), String>;

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

    /// The sentence the session start context uses to say how to read the index.
    fn read_index_hint(&self, path: &Path) -> String;
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

/// Paths a caller may name as documents in the file ledger. Rejects an empty path, an
/// absolute path, a path with a `..` component, and the parts of the ledger that are not
/// documents.
pub fn check_document_path(path: &str) -> Result<(), String> {
    let parts = relative_parts(path)?;
    let first = parts[0].as_str();
    let reserved = first == "threads"
        || first == "external-events"
        || first == files::RECORDS_DIR
        || (parts.len() == 1 && first == super::manifest::MANIFEST_FILE);
    if reserved || parts.iter().any(|part| part == ".git") {
        return Err(format!(
            "error: '{}' is not a document: threads/, external-events/, records/, {} and .git are managed by tsk",
            path,
            super::manifest::MANIFEST_FILE
        ));
    }
    Ok(())
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
            "records/missions/M-X.json",
            ".tsk-ledger.toml",
            "./.tsk-ledger.toml",
            ".git",
            "a/.git/config",
        ] {
            assert!(check_document_path(bad).is_err(), "{}", bad);
        }
    }

    #[test]
    fn docs_paths_apply_only_the_git_rules() {
        for (given, normal) in [
            ("index.md", "index.md"),
            ("./missions/M-X.md", "missions/M-X.md"),
            // Reserved for the file ledger only.
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
