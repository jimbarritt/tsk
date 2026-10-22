//! The Dolt backend: the ledger is a Dolt database in a directory beside the managed
//! repo's other state, published to the same git ref the file backend uses through
//! Dolt's `git+` remotes. Every operation runs the external `dolt` binary. Threads,
//! continuation entries, bindings and events are rows in the tables defined in
//! [`crate::ledger::dolt`]. Documents are not rows: they are Markdown files on a second
//! branch, `tsk/docs`, checked out as a worktree beside the database (see
//! [`crate::ledger::docs`]). A version 1 ledger still keeps them in a `documents` table
//! until `tsk ledger migrate-docs` moves them.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use crate::ledger::docs;
use crate::ledger::dolt::{
    self, flag, integer, lit, text, Dolt, BRANCH, LEGACY_LAYOUT_VERSION, REMOTE, REMOTE_BRANCH_NAME,
};
use crate::ledger::fetch::{fetch_tip, ref_kind, FetchOutcome, RefKind};
use crate::ledger::git::Git;
use crate::ledger::location::{
    docs_worktree_path, dolt_db_path, state_root_from_env, LedgerLocation, Repo,
};
use crate::ledger::manifest::{self, ManifestInfo, DOLT_SUPPORTED_VERSIONS};
use crate::ledger::push::{PushOutcome, PushResult, PushStatus, MAX_ATTEMPTS};
use crate::ledger::status::{self, Change, ChangeKind, LedgerStatus, RemoteState, SideStatus};
use crate::ledger::tips::{self, RemoteHeads, Tips};
use crate::ledger::transport::{self, Published, Transport};
use crate::mission::model::{
    self, Category, Decision, Document, Item, Mission, Outcome, Question, QuestionStatus, Role,
    Status, Task,
};

use super::doc_tree::DocTree;
use super::renumber::{self, Renumbered};
use super::{BindingKind, BindingRow, LedgerStore, PushReport, RefreshOutcome, ThreadInfo};

const MANIFEST_SOURCE: &str = "the manifest table";

/// What every command except `tsk ledger fetch` and `tsk ledger migrate-docs` says about
/// a version 1 ledger.
pub const VERSION_1_REFUSAL: &str = "error: this ledger stores its documents in Dolt (version 1). Run tsk ledger migrate-docs to move them to the tsk/docs branch.";

pub struct DoltStore {
    repo: Repo,
    state_root: PathBuf,
    /// The database and layout version of the last manifest check that found the current
    /// layout. Later checks in the same process reuse it, which saves a `dolt` process
    /// each. Anything that can move the database to another state clears it.
    checked: RefCell<Option<(PathBuf, i64)>>,
    synced: RefCell<Synced>,
    /// What a thread command reads, loaded in one `dolt` process by its refresh. Any
    /// write drops it, and the next read loads it again.
    snapshot: RefCell<Option<Snapshot>>,
    /// Set by the refresh of a thread command: its reads come from the snapshot.
    read_ahead: Cell<bool>,
    /// A snapshot could not be loaded, so the reads go one by one until the next write.
    no_snapshot: Cell<bool>,
    /// Set by a command that asked not to question origin first: the record of the last
    /// fetch stands in for what origin holds. See [`LedgerStore::trust_last_fetch`].
    trust_tips: Cell<bool>,
    /// Writes held back for the next commit, which makes them in its own process. See
    /// [`LedgerStore::continuation_append_deferred`].
    queued: RefCell<Vec<QueuedWrite>>,
}

/// A write held back for the next commit: the statement, and a query whose first count is
/// above 0 once the statement has applied, so a retry after a failed batch never writes it twice.
struct QueuedWrite {
    sql: String,
    applied_query: String,
}

/// What a push of the Dolt data needs to know from `commit_and_push`: whether it committed
/// local changes first, whether the documents already moved in this push, and the message
/// for the commit of a file remote directory.
#[derive(Clone, Copy)]
struct PushRequest<'a> {
    committed: bool,
    docs_changed: bool,
    message: &'a str,
    /// The four reads that open an attempt of the push (see [`push_reads`]), when the
    /// batch of the commit made them already.
    prefetched: Option<&'a [Vec<serde_json::Value>]>,
}

/// A thread of the `threads` table.
#[derive(Clone)]
struct ThreadRow {
    id: String,
    briefing_path: Option<String>,
    mission_id: Option<String>,
}

/// The reads of a thread command, taken in one process after the refresh: the manifest
/// version, every mission, the bindings, the threads, the commit `HEAD` is at, and
/// whether the working set holds changes.
struct Snapshot {
    version: i64,
    head: String,
    dirty: bool,
    bindings: Vec<BindingRow>,
    threads: Vec<ThreadRow>,
    missions: Vec<Mission>,
}

/// What the refresh found in the Dolt database.
struct DataState {
    remote_head: String,
    pending: Vec<String>,
    version: i64,
    /// Whether the working set held changes when it was read.
    dirty: bool,
}

const DIRTY_QUERY: &str = "select count(*) as c from dolt_status";
const BINDINGS_QUERY: &str =
    "select actor, kind, thread_id, registered_at from bindings order by registered_at, actor";
const THREADS_QUERY: &str = "select thread_id, briefing_path, mission_id from threads";

/// What this process has found out about origin, so a command fetches each ref at most once.
#[derive(Default)]
struct Synced {
    /// The heads origin held when this process asked, once: `Some(None)` when the question
    /// failed, so the fetches run as they did before the question existed.
    heads: Option<Option<RemoteHeads>>,
    /// The local Dolt store holds origin's data as of the heads: it was fetched, or its
    /// record of the last fetch matched. Cleared by a push of the data.
    data: bool,
    /// The documents tip this process fetched, checked or pushed: `Some(None)` for a
    /// branch origin does not hold.
    docs: Option<Option<String>>,
    /// This process ran a fetch of the data.
    fetched_here: bool,
    /// The commits a combined fetch brought in place of the question to origin: the ledger
    /// ref's, then the documents ref's. The sync that would have fetched a ref takes its
    /// commit once. A retry after a rejected push fetches again, so it clears them.
    brought: Option<(Option<String>, Option<String>)>,
    /// The heads are those of the record of the last fetch, not an answer from origin.
    trusted: bool,
}

impl DoltStore {
    pub fn open(dir: &Path) -> Result<DoltStore, String> {
        let repo = Repo::discover(dir)?;
        let state_root = state_root_from_env()?;
        // A failed background publish shows once, on the next command that opens the store.
        if let Some(id) = repo.read_clone_id().ok().flatten() {
            crate::ledger::background::warn_once(&dolt_db_path(&state_root, &id));
        }
        Ok(DoltStore {
            repo,
            state_root,
            checked: RefCell::new(None),
            synced: RefCell::new(Synced::default()),
            snapshot: RefCell::new(None),
            read_ahead: Cell::new(false),
            no_snapshot: Cell::new(false),
            trust_tips: Cell::new(false),
            queued: RefCell::new(Vec::new()),
        })
    }

    /// The record of the commits this clone last fetched, once the clone has an id.
    fn tips(&self) -> Option<Tips> {
        let id = self.repo.read_clone_id().ok().flatten()?;
        dolt_db_path(&self.state_root, &id).parent().map(Tips::new)
    }

    /// What origin holds for the ledger ref and the documents ref, from one `git
    /// ls-remote` per process. `None` when origin cannot be asked: the caller then fetches
    /// as before, and the fetch reports the failure.
    fn heads(&self, location: &LedgerLocation) -> Option<RemoteHeads> {
        if let Some(known) = &self.synced.borrow().heads {
            return known.clone();
        }
        let from_tips = if self.trust_tips.get() {
            self.heads_from_tips(location)
        } else {
            None
        };
        let trusted = from_tips.is_some();
        let asked = from_tips.or_else(|| {
            // A clone with no record of a fetch has to fetch both refs. One fetch of both
            // does that, and names their commits, in place of a question and two fetches.
            if self.has_no_record(location) {
                match tips::fetch_heads(&self.repo.git, location) {
                    Ok(heads) => {
                        self.synced.borrow_mut().brought =
                            Some((heads.ledger.clone(), heads.docs.clone()));
                        return Some(heads);
                    }
                    // Origin is out of reach: the question fails the same way, so skip it.
                    Err(tips::FetchFailure::Failed(_)) => return None,
                    // A ref is missing: the question says which.
                    Err(tips::FetchFailure::RefMissing) => {}
                }
            }
            tips::remote_heads(&self.repo.git, location).ok()
        });
        self.synced.borrow_mut().heads = Some(asked.clone());
        self.synced.borrow_mut().trusted = trusted;
        asked
    }

    /// True for the first rejection of a push that went on the record of the last fetch.
    /// It says only that the record was out of date, which the rejected push has found
    /// out, and the retry fetches. Said aloud it would be a note that an ask to origin
    /// first would not have printed.
    fn rejection_is_expected(&self, attempt: usize) -> bool {
        attempt == 1 && self.synced.borrow().trusted
    }

    /// True when this clone has no ledger database or no record of a fetch of the ledger
    /// ref, so the data has to be fetched whatever origin holds.
    fn has_no_record(&self, location: &LedgerLocation) -> bool {
        let Some(path) = self.hint() else {
            return true;
        };
        if !is_database(&path) {
            return true;
        }
        self.tips()
            .and_then(|tips| tips.get(location.remote(), location.ref_name()))
            .is_none()
    }

    /// The commit of the ledger ref in the repository: the one a combined fetch brought,
    /// else the one `git fetch` brings now. `None` when origin lacks the ref.
    fn ledger_tip_fetched(&self, location: &LedgerLocation) -> Result<Option<String>, String> {
        match self.take_brought_ledger() {
            Some(commit) => Ok(Some(commit)),
            None => fetch_tip(&self.repo.git, location),
        }
    }

    /// [`Self::ledger_tip_fetched`] for the documents ref, whose location is `docs`.
    fn docs_tip_fetched(&self, docs: &LedgerLocation) -> Result<Option<String>, String> {
        match self.take_brought_docs() {
            Some(commit) => Ok(Some(commit)),
            None => fetch_tip(&self.repo.git, docs),
        }
    }

    /// The commit the combined fetch brought for the ledger ref, once.
    fn take_brought_ledger(&self) -> Option<String> {
        self.synced
            .borrow_mut()
            .brought
            .as_mut()
            .and_then(|(ledger, _)| ledger.take())
    }

    /// The commit the combined fetch brought for the documents ref, once.
    fn take_brought_docs(&self) -> Option<String> {
        self.synced
            .borrow_mut()
            .brought
            .as_mut()
            .and_then(|(_, docs)| docs.take())
    }

    /// The heads the record of the last fetch names, as if origin held them still. `None`
    /// when the record lacks either ref, or the store's file remote directory is not a
    /// checkout: origin is asked then. A push of a clone whose record is out of date is
    /// rejected, and the retry fetches.
    fn heads_from_tips(&self, location: &LedgerLocation) -> Option<RemoteHeads> {
        // Only a store on a file remote trusts its record. A store on the older git remote
        // layout meets a ledger ref that another clone has migrated, and only a question to
        // origin shows that.
        let path = self.hint()?;
        let url = transport::store_url(&path)?;
        if Transport::of_url(&url) != Transport::FileRemote {
            return None;
        }
        let tips = self.tips()?;
        let docs = location.docs();
        let ledger = tips.get(location.remote(), location.ref_name())?;
        let documents = tips.get(docs.remote(), docs.ref_name())?;
        if !self.file_checkout_present() {
            return None;
        }
        Some(RemoteHeads {
            ledger: Some(ledger),
            docs: Some(documents),
        })
    }

    /// Bring the local Dolt store's copy of origin up to date: nothing when this process
    /// already did or origin's head is the one last fetched, else `dolt fetch`.
    fn sync_data(&self, db: &Dolt, location: &LedgerLocation) -> Result<(), String> {
        if self.data_is_current(location) {
            self.synced.borrow_mut().data = true;
            return Ok(());
        }
        self.fetch_data(db, location)
    }

    /// True when a fetch of the Dolt data would bring nothing: this process fetched it, or
    /// origin's head is the one the last fetch recorded.
    fn data_is_current(&self, location: &LedgerLocation) -> bool {
        if self.synced.borrow().data {
            return true;
        }
        if !self.file_checkout_present() {
            return false;
        }
        let Some(wanted) = self.heads(location).and_then(|heads| heads.ledger) else {
            return false;
        };
        self.tips()
            .and_then(|tips| tips.get(location.remote(), location.ref_name()))
            .as_deref()
            == Some(wanted.as_str())
    }

    /// Bring the store's copy of origin up to date, and record what that brought. A store
    /// on a file remote runs `git fetch` and resets the directory first, then `dolt
    /// fetch` reads the directory. A store on a git remote runs `dolt fetch`, which starts
    /// git itself, unless origin now holds a file remote: the store then moves to it.
    fn fetch_data(&self, db: &Dolt, location: &LedgerLocation) -> Result<(), String> {
        if self.store_transport(db) == Transport::FileRemote {
            return self.fetch_file_remote(db, location);
        }
        if self.reroot(db, location)? {
            return Ok(());
        }
        let wanted = self.heads(location).and_then(|heads| heads.ledger);
        db.run(["fetch", REMOTE])?;
        self.note_data_fetched(location, wanted.as_deref());
        Ok(())
    }

    /// The transport this clone's store uses, from the URL of its `origin` remote.
    fn store_transport(&self, db: &Dolt) -> Transport {
        transport::store_url(db.dir()).map_or(Transport::GitRemote, |url| Transport::of_url(&url))
    }

    /// False when the store uses a file remote and its directory is not a checkout. The
    /// record of the last fetch cannot be trusted then.
    fn file_checkout_present(&self) -> bool {
        let Some(path) = self.hint() else {
            return true;
        };
        match transport::store_url(&path) {
            Some(url) if Transport::of_url(&url) == Transport::FileRemote => {
                transport::remote_dir_beside(&path).join(".git").exists()
            }
            _ => true,
        }
    }

    /// `git fetch` of the ledger ref, the directory reset to it, then `dolt fetch` from the
    /// directory.
    fn fetch_file_remote(&self, db: &Dolt, location: &LedgerLocation) -> Result<(), String> {
        let tip = self.ledger_tip_fetched(location)?;
        if let Some(commit) = &tip {
            if transport::of_commit(&self.repo.git, commit)? != Transport::FileRemote {
                return Err(format!(
                    "error: {} on {} holds the git-remote layout, and this ledger store uses a file remote.\n       \
                     Move the store aside and run tsk ledger fetch again",
                    location.ref_name(),
                    location.label()
                ));
            }
            transport::sync_checkout(&self.repo, &transport::remote_dir_beside(db.dir()), commit)?;
            db.run(["fetch", REMOTE])?;
        }
        self.note_data_fetched(location, tip.as_deref());
        Ok(())
    }

    /// A store on a git remote meets a ledger ref that now holds a file remote (another
    /// clone ran `tsk ledger migrate-transport`). The store keeps its commits, local ones
    /// included, and points its `origin` at the directory. Returns whether it did, and
    /// then the data is fetched. `TSK_LEDGER_TRANSPORT=git-remote` turns the check off.
    fn reroot(&self, db: &Dolt, location: &LedgerLocation) -> Result<bool, String> {
        if transport::forced()? == Some(Transport::GitRemote) {
            return Ok(false);
        }
        let Some(commit) = self.ledger_tip_fetched(location)? else {
            return Ok(false);
        };
        if transport::of_commit(&self.repo.git, &commit)? != Transport::FileRemote {
            return Ok(false);
        }
        let dir = transport::remote_dir_beside(db.dir());
        transport::sync_checkout(&self.repo, &dir, &commit)?;
        transport::point_store_at_file_remote(db, &dir, location.ref_name())?;
        db.run(["fetch", REMOTE])?;
        self.note_data_fetched(location, Some(&commit));
        eprintln!(
            "note: {} on {} holds a Dolt file remote now; this ledger store moved to it",
            location.ref_name(),
            location.label()
        );
        Ok(true)
    }

    /// Record that the local Dolt store holds origin's data as of `head`, the commit the
    /// ledger ref was at before the fetch ran (or `None` when it is not known).
    fn note_data_fetched(&self, location: &LedgerLocation, head: Option<&str>) {
        if let Some(tips) = self.tips() {
            tips.set(location.remote(), location.ref_name(), head);
        }
        let mut synced = self.synced.borrow_mut();
        synced.data = true;
        synced.fetched_here = true;
    }

    /// The documents tip: nothing is fetched when this process already holds it, when origin
    /// has no documents branch, or when the commit origin names is the one last fetched
    /// and is still in the repository. Else `git fetch`.
    fn sync_docs_tip(&self, location: &LedgerLocation) -> Result<Option<String>, String> {
        if let Some(known) = self.synced.borrow().docs.clone() {
            return Ok(known);
        }
        let docs = location.docs();
        let tips = self.tips();
        if let Some(heads) = self.heads(location) {
            match (&heads.docs, &tips) {
                (None, _) => {
                    if let Some(tips) = &tips {
                        tips.set(docs.remote(), docs.ref_name(), None);
                    }
                    self.synced.borrow_mut().docs = Some(None);
                    return Ok(None);
                }
                (Some(head), Some(tips)) => {
                    let recorded = tips.get(docs.remote(), docs.ref_name());
                    if recorded.as_deref() == Some(head.as_str())
                        && tips::commit_is_local(&self.repo.git, head)
                    {
                        self.synced.borrow_mut().docs = Some(Some(head.clone()));
                        return Ok(Some(head.clone()));
                    }
                }
                _ => {}
            }
        }
        let tip = self.docs_tip_fetched(&docs)?;
        self.note_docs_tip(location, tip.as_deref());
        Ok(tip)
    }

    /// Record the documents tip this process fetched or pushed.
    fn note_docs_tip(&self, location: &LedgerLocation, tip: Option<&str>) {
        let docs = location.docs();
        if let Some(tips) = self.tips() {
            tips.set(docs.remote(), docs.ref_name(), tip);
        }
        self.synced.borrow_mut().docs = Some(tip.map(str::to_string));
    }

    /// After a push of the data: the local copy of origin is no longer the one fetched.
    /// Dolt's cache of the remote holds the commit the push made, under `.../last`. When
    /// it can be read, it is the head origin has now, and the next command finds the
    /// store current.
    fn note_data_pushed(&self, db: &Dolt, location: &LedgerLocation) {
        self.synced.borrow_mut().data = false;
        let pushed = pushed_commit(db, location);
        if let Some(tips) = self.tips() {
            tips.set(location.remote(), location.ref_name(), pushed.as_deref());
        }
    }

    /// After a publish of the file remote: origin holds `commit`, which the directory and
    /// the store both have.
    fn note_file_published(&self, location: &LedgerLocation, commit: &str) {
        self.synced.borrow_mut().data = false;
        if let Some(tips) = self.tips() {
            tips.set(location.remote(), location.ref_name(), Some(commit));
        }
    }

    /// Forget the commit the last fetch recorded, before a push changes the directory.
    fn forget_data_tip(&self, location: &LedgerLocation) {
        self.synced.borrow_mut().data = false;
        if let Some(tips) = self.tips() {
            tips.set(location.remote(), location.ref_name(), None);
        }
    }

    fn hint(&self) -> Option<PathBuf> {
        self.repo
            .read_clone_id()
            .ok()
            .flatten()
            .map(|id| dolt_db_path(&self.state_root, &id))
    }

    /// The database of an already materialised ledger.
    fn db(&self) -> Result<Dolt, String> {
        let missing = |what: String| {
            format!(
                "error: {}. Run tsk ledger fetch first, which creates the ledger database",
                what
            )
        };
        let clone_id = self.repo.read_clone_id()?.ok_or_else(|| {
            missing(format!(
                "this clone has no clone id yet ({} is absent or empty)",
                self.repo.clone_id_file().display()
            ))
        })?;
        let path = dolt_db_path(&self.state_root, &clone_id);
        if !is_database(&path) {
            return Err(missing(format!("{} does not exist", path.display())));
        }
        Ok(Dolt::store(path))
    }

    fn describe(&self, what: &str) -> String {
        match self.hint() {
            Some(root) => format!("{} in {}", what, root.display()),
            None => what.to_string(),
        }
    }

    fn manifest_rows(&self, db: &Dolt) -> Result<ManifestInfo, String> {
        manifest_from_rows(&db.rows(MANIFEST_QUERY)?)
    }
}

const MANIFEST_QUERY: &str = "select k, v from manifest";

fn manifest_from_rows(rows: &[serde_json::Value]) -> Result<ManifestInfo, String> {
    let value = |key: &str| {
        rows.iter()
            .find(|row| text(row, "k").as_deref() == Some(key))
            .and_then(|row| text(row, "v"))
    };
    let version = value("version").ok_or_else(|| {
        "error: the ledger's manifest table has no version row; it is not a tsk ledger".to_string()
    })?;
    let version = version.trim().parse::<i64>().map_err(|_| {
        format!(
            "error: the ledger's manifest version '{}' is not a number",
            version
        )
    })?;
    Ok(ManifestInfo {
        version,
        repo_id: value("repo_id"),
    })
}

impl DoltStore {
    /// The manifest check every Dolt command makes. Versions 1 and 2 pass: it returns the
    /// version, and the caller decides what a version 1 ledger may do.
    fn check_manifest_of(&self, db: &Dolt, location: &LedgerLocation) -> Result<i64, String> {
        let info = self.manifest_rows(db)?;
        manifest::check_values(
            info.version,
            info.repo_id.as_deref(),
            location.repo_id(),
            MANIFEST_SOURCE,
            DOLT_SUPPORTED_VERSIONS,
        )
    }

    /// The manifest check of a command that needs the documents on the branch: a version
    /// 1 ledger is refused with the way out.
    fn check_current(&self, db: &Dolt, location: &LedgerLocation) -> Result<i64, String> {
        if let Some((dir, version)) = &*self.checked.borrow() {
            if dir == db.dir() {
                return Ok(*version);
            }
        }
        let version = self.check_manifest_of(db, location)?;
        if version == LEGACY_LAYOUT_VERSION {
            return Err(VERSION_1_REFUSAL.to_string());
        }
        self.remember_checked(db, version);
        Ok(version)
    }

    fn remember_checked(&self, db: &Dolt, version: i64) {
        *self.checked.borrow_mut() =
            (version == dolt::LAYOUT_VERSION).then(|| (db.dir().to_path_buf(), version));
    }

    fn forget_checked(&self) {
        *self.checked.borrow_mut() = None;
    }

    /// Drop the snapshot: something changed the database, or is about to.
    fn forget_snapshot(&self) {
        *self.snapshot.borrow_mut() = None;
        self.no_snapshot.set(false);
    }

    /// A write in one transaction. The snapshot no longer holds.
    fn write(&self, db: &Dolt, sql: &str) -> Result<(), String> {
        self.forget_snapshot();
        self.flush_queued(db)?;
        db.exec(sql)
    }

    /// Make the writes held back for a commit, in the order they were held. A write that
    /// the database already holds is not made again.
    fn flush_queued(&self, db: &Dolt) -> Result<(), String> {
        let queued = std::mem::take(&mut *self.queued.borrow_mut());
        for write in queued {
            if db.count(&write.applied_query)? == 0 {
                db.exec(&write.sql)?;
            }
        }
        Ok(())
    }

    /// True when the database is known to be at the current layout, from the last check
    /// of this process.
    fn layout_is_current(&self, db: &Dolt) -> Result<bool, String> {
        Ok(self
            .checked
            .borrow()
            .as_ref()
            .is_some_and(|(dir, version)| dir == db.dir() && *version == dolt::LAYOUT_VERSION))
    }

    /// The queries of a snapshot, and how many of them read missions.
    fn snapshot_queries() -> Result<(Vec<String>, usize), String> {
        let mut recorded = Vec::new();
        read_missions_with(dolt::LAYOUT_VERSION, &mut |sql| {
            recorded.push(sql.to_string());
            Ok(Vec::new())
        })?;
        let missions = recorded.len();
        let mut queries = vec![MANIFEST_QUERY.to_string()];
        queries.extend(recorded);
        queries.push(BINDINGS_QUERY.to_string());
        queries.push(THREADS_QUERY.to_string());
        queries.push(hash_query("HEAD"));
        queries.push(DIRTY_QUERY.to_string());
        Ok((queries, missions))
    }

    /// The snapshot in the results of [`Self::snapshot_queries`]. The manifest check is
    /// the one every command makes, so a ledger it refuses fails the same way. `None`
    /// when the ledger is not at the current layout.
    fn parse_snapshot(
        &self,
        location: &LedgerLocation,
        sets: Vec<Vec<serde_json::Value>>,
    ) -> Result<Option<Snapshot>, String> {
        let mut sets = sets.into_iter();
        let mut next = || {
            sets.next()
                .ok_or_else(|| "error: a batched read ran out of results".to_string())
        };
        let info = manifest_from_rows(&next()?)?;
        manifest::check_values(
            info.version,
            info.repo_id.as_deref(),
            location.repo_id(),
            MANIFEST_SOURCE,
            DOLT_SUPPORTED_VERSIONS,
        )?;
        if info.version != dolt::LAYOUT_VERSION {
            return Ok(None);
        }
        let missions = read_missions_with(dolt::LAYOUT_VERSION, &mut |_| next())?;
        let bindings = binding_rows(&next()?, false);
        let threads = next()?
            .iter()
            .map(|row| ThreadRow {
                id: text(row, "thread_id").unwrap_or_default(),
                briefing_path: text(row, "briefing_path").filter(|p| !p.is_empty()),
                mission_id: text(row, "mission_id").filter(|m| !m.is_empty()),
            })
            .collect();
        let head = next()?
            .first()
            .and_then(|row| text(row, "h"))
            .ok_or_else(|| "error: dolt could not resolve HEAD".to_string())?;
        let dirty = dolt::first_count(&next()?) > 0;
        Ok(Some(Snapshot {
            version: info.version,
            head,
            dirty,
            bindings,
            threads,
            missions,
        }))
    }

    /// Load the snapshot in one process. `Ok(None)` when it cannot be loaded (batching is
    /// off, a query fails, the layout is older): the reads then go one by one.
    fn read_ahead(&self, db: &Dolt, location: &LedgerLocation) -> Result<Option<i64>, String> {
        self.forget_snapshot();
        let (queries, _) = Self::snapshot_queries()?;
        let Ok(sets) = db.rows_each(&queries) else {
            self.no_snapshot.set(true);
            return Ok(None);
        };
        self.install_snapshot(db, location, sets)
    }

    fn install_snapshot(
        &self,
        db: &Dolt,
        location: &LedgerLocation,
        sets: Vec<Vec<serde_json::Value>>,
    ) -> Result<Option<i64>, String> {
        match self.parse_snapshot(location, sets)? {
            Some(snapshot) => {
                let version = snapshot.version;
                self.remember_checked(db, version);
                *self.snapshot.borrow_mut() = Some(snapshot);
                Ok(Some(version))
            }
            None => {
                self.no_snapshot.set(true);
                Ok(None)
            }
        }
    }

    /// Read from the snapshot of a thread command, loading it when a write dropped it.
    /// `None` when this command keeps no snapshot or none can be loaded.
    fn read_snapshot<T>(
        &self,
        db: &Dolt,
        pick: impl FnOnce(&Snapshot) -> T,
    ) -> Result<Option<T>, String> {
        if !self.read_ahead.get() || self.no_snapshot.get() {
            return Ok(None);
        }
        if self.snapshot.borrow().is_none() {
            let location = self.repo.location(&self.state_root)?;
            self.read_ahead(db, &location)?;
        }
        Ok(self.snapshot.borrow().as_ref().map(pick))
    }

    /// The manifest check at the end of a refresh, reading the snapshot of a thread
    /// command with it.
    fn read_tail(
        &self,
        db: &Dolt,
        location: &LedgerLocation,
        thread_command: bool,
    ) -> Result<i64, String> {
        if thread_command {
            if let Some(version) = self.read_ahead(db, location)? {
                return Ok(version);
            }
        }
        let version = self.check_manifest_of(db, location)?;
        self.remember_checked(db, version);
        Ok(version)
    }

    /// [`Self::read_tail`] for results the refresh's batch already holds.
    fn finish_tail(
        &self,
        db: &Dolt,
        location: &LedgerLocation,
        thread_command: bool,
        sets: Vec<Vec<serde_json::Value>>,
    ) -> Result<i64, String> {
        let info = manifest_from_rows(sets.first().map(Vec::as_slice).unwrap_or(&[]))?;
        if thread_command {
            if let Some(version) = self.install_snapshot(db, location, sets)? {
                return Ok(version);
            }
        }
        let version = manifest::check_values(
            info.version,
            info.repo_id.as_deref(),
            location.repo_id(),
            MANIFEST_SOURCE,
            DOLT_SUPPORTED_VERSIONS,
        )?;
        self.remember_checked(db, version);
        Ok(version)
    }

    /// Everything the refresh reads after the fetch decision, in one `dolt` process: the
    /// remote check and the dirty check (when `first` holds the URL and path), the remote
    /// branch, its head, the commits not pushed, the local head and the tail (the
    /// manifest, or the snapshot of a thread command). Brings the database up to origin
    /// when it is behind, and reads the tail again. `Ok(None)` when the batch fails or
    /// the remote branch is missing, and the caller reads one by one, which gives the same
    /// result or the same error.
    fn read_state(
        &self,
        db: &Dolt,
        location: &LedgerLocation,
        present: bool,
        first: Option<(&str, &Path)>,
        dirty_before: bool,
        thread_command: bool,
    ) -> Result<Option<DataState>, String> {
        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        let mut queries = Vec::new();
        if first.is_some() {
            queries.push(remote_query());
            queries.push(DIRTY_QUERY.to_string());
        }
        let branch_at = queries.len();
        queries.push(remote_branch_query());
        queries.push(hash_query(&remote_ref));
        if present {
            queries.push(pending_query());
            queries.push(hash_query("HEAD"));
        }
        let tail_at = queries.len();
        if thread_command {
            queries.extend(Self::snapshot_queries()?.0);
        } else {
            queries.push(MANIFEST_QUERY.to_string());
        }
        let Ok(mut sets) = db.rows_each(&queries) else {
            return Ok(None);
        };
        let mut dirty = dirty_before;
        if let Some((url, path)) = first {
            verify_remote_rows(db, &sets[0], url, location)?;
            dirty = dolt::first_count(&sets[1]) > 0;
            if dirty && !thread_command {
                return Err(dirty_error(path));
            }
        }
        let remote_head = sets[branch_at + 1].first().and_then(|r| text(r, "h"));
        let Some(remote_head) = remote_head.filter(|_| dolt::first_count(&sets[branch_at]) > 0)
        else {
            return Ok(None);
        };
        let mut pending = Vec::new();
        let mut behind = false;
        if present {
            pending = pending_from_rows(&sets[branch_at + 2]);
            let local = sets[branch_at + 3].first().and_then(|r| text(r, "h"));
            behind = pending.is_empty() && !dirty && local.as_deref() != Some(remote_head.as_str());
        }
        let version = if behind {
            self.reset_and_read_tail(db, location, thread_command, &remote_ref)?
        } else {
            let tail = sets.split_off(tail_at);
            self.finish_tail(db, location, thread_command, tail)?
        };
        Ok(Some(DataState {
            remote_head,
            pending,
            version,
            dirty,
        }))
    }

    /// Reset the database to `remote_ref`, then read the tail of the refresh. One process
    /// does both. When the batch fails, the reset and the reads run one by one, which gives
    /// the same result or the same error.
    fn reset_and_read_tail(
        &self,
        db: &Dolt,
        location: &LedgerLocation,
        thread_command: bool,
        remote_ref: &str,
    ) -> Result<i64, String> {
        let mut queries = vec![format!("call dolt_reset('--hard', {})", lit(remote_ref))];
        if thread_command {
            queries.extend(Self::snapshot_queries()?.0);
        } else {
            queries.push(MANIFEST_QUERY.to_string());
        }
        match db.rows_each(&queries) {
            Ok(mut sets) => {
                let tail = sets.split_off(1);
                self.finish_tail(db, location, thread_command, tail)
            }
            Err(_) => {
                db.run(["reset", "--hard", remote_ref])?;
                self.read_tail(db, location, thread_command)
            }
        }
    }

    /// [`Self::read_state`] one query at a time.
    fn read_state_one_by_one(
        &self,
        db: &Dolt,
        location: &LedgerLocation,
        present: bool,
        dirty: bool,
        thread_command: bool,
    ) -> Result<DataState, String> {
        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        let mut has_remote_branch = remote_branch_exists(db)?;
        if !has_remote_branch && !self.synced.borrow().fetched_here {
            // The record of the last fetch said the store was current, and it lacks the
            // branch. Fetch for real.
            self.fetch_data(db, location)?;
            has_remote_branch = remote_branch_exists(db)?;
        }
        if !has_remote_branch {
            return Err(format!(
                "error: {} on {} holds no Dolt branch '{}'; it is not a Dolt ledger",
                location.ref_name(),
                location.label(),
                BRANCH
            ));
        }
        let remote_head = db.hash_of(&remote_ref)?;
        let pending = if present {
            pending_commits(db)?
        } else {
            Vec::new()
        };
        if present && pending.is_empty() && !dirty {
            db.run(["reset", "--hard", &remote_ref])?;
        }
        let version = self.read_tail(db, location, thread_command)?;
        Ok(DataState {
            remote_head,
            pending,
            version,
            dirty,
        })
    }

    /// The worktree of the documents branch, when this clone has an id.
    fn docs_hint(&self) -> Option<PathBuf> {
        self.repo
            .read_clone_id()
            .ok()
            .flatten()
            .map(|id| docs_worktree_path(&self.state_root, &id))
    }

    /// The documents of an already materialised ledger.
    fn docs_tree(&self) -> Result<DocTree, String> {
        let missing = |what: String| {
            // A version 1 ledger has no documents worktree. Say so, not "fetch first".
            if let Ok(db) = self.db() {
                if self
                    .manifest_rows(&db)
                    .is_ok_and(|info| info.version == LEGACY_LAYOUT_VERSION)
                {
                    return VERSION_1_REFUSAL.to_string();
                }
            }
            format!(
                "error: {}. Run tsk ledger fetch first, which creates the documents worktree",
                what
            )
        };
        let clone_id = self.repo.read_clone_id()?.ok_or_else(|| {
            missing(format!(
                "this clone has no clone id yet ({} is absent or empty)",
                self.repo.clone_id_file().display()
            ))
        })?;
        let root = docs_worktree_path(&self.state_root, &clone_id);
        if !root.is_dir() {
            return Err(missing(format!("{} does not exist", root.display())));
        }
        Ok(DocTree { root })
    }

    /// [`LedgerStore::refresh`], or the lenient refresh of `tsk ledger fetch` and
    /// `tsk ledger migrate-docs` when `lenient` is set: a version 1 ledger refreshes its
    /// Dolt database only and gets a note instead of the refusal.
    fn refresh_with(&self, lenient: bool, keep_local: bool) -> Result<RefreshOutcome, String> {
        let (mut outcome, version) = self.refresh_data(keep_local)?;
        if version == LEGACY_LAYOUT_VERSION {
            if !lenient {
                return Err(VERSION_1_REFUSAL.to_string());
            }
            let note = "note: this ledger stores its documents in Dolt (version 1). Run tsk ledger migrate-docs to move them to the tsk/docs branch.".to_string();
            outcome.pending_note = Some(join_notes(outcome.pending_note.take(), note));
            return Ok(outcome);
        }
        let location = self.repo.location(&self.state_root)?;
        let tip = self.sync_docs_tip(&location)?;
        let fetched =
            docs::fetch_with_tip(&self.repo, &self.state_root, &location, keep_local, tip)?;
        let dolt_unpublished = outcome.unpublished;
        for line in &fetched.pending {
            outcome.pending.push(format!("documents: {}", line));
        }
        let docs_outcome = FetchOutcome {
            location: fetched.location,
            path: fetched.path,
            pending: fetched.pending,
            unpublished: fetched.unpublished,
        };
        if !docs_outcome.pending.is_empty() {
            let note = crate::ledger::pending_note(&docs_outcome);
            outcome.pending_note = Some(join_notes(outcome.pending_note.take(), note));
        }
        // A new ledger says once that nothing is published. The documents branch is
        // named separately only when the ledger is published and its documents are not.
        if docs_outcome.unpublished && !dolt_unpublished {
            let note = crate::ledger::unpublished_note(&docs_outcome);
            outcome.unpublished_note = Some(join_notes(outcome.unpublished_note.take(), note));
            outcome.unpublished = true;
        }
        Ok(outcome)
    }

    /// Commit nothing more and push the Dolt data: fetch, merge a newer remote tip, push,
    /// with a bounded retry. `committed` says whether the caller committed local changes
    /// to the database first. `docs_changed` says the documents already moved in this
    /// push, which silences the notes that say nothing happened.
    fn push_data(
        &self,
        location: &LedgerLocation,
        db: &Dolt,
        request: PushRequest<'_>,
        notes: &mut dyn FnMut(&str),
    ) -> Result<PushReport, String> {
        let mut renumbered = Vec::new();
        let result = self.push_data_renumbering(location, db, request, notes, &mut renumbered);
        let lines: Vec<String> = renumbered.iter().map(Renumbered::line).collect();
        match result {
            Ok(mut report) => {
                report.renumbered = lines;
                Ok(report)
            }
            // A failure after a renumbering still tells which rows moved, as the lines
            // would have been printed on success.
            Err(error) if !lines.is_empty() => Err(format!(
                "{}\n       Rows renumbered before the failure:\n       {}",
                error,
                lines.join("\n       ")
            )),
            Err(error) => Err(error),
        }
    }

    /// [`Self::push_data`], with the rows it renumbered collected in `renumbered`.
    fn push_data_renumbering(
        &self,
        location: &LedgerLocation,
        db: &Dolt,
        request: PushRequest<'_>,
        notes: &mut dyn FnMut(&str),
        renumbered: &mut Vec<Renumbered>,
    ) -> Result<PushReport, String> {
        let PushRequest {
            committed,
            docs_changed,
            message,
            prefetched,
        } = request;
        self.forget_checked();
        self.forget_snapshot();
        let path = db.dir().to_path_buf();
        let outcome = |result: PushResult, commit: String| PushOutcome {
            location: location.clone(),
            path: path.clone(),
            committed,
            commit,
            result,
        };
        let finish = |outcome: PushOutcome, notes: &mut dyn FnMut(&str)| {
            // The "nothing to commit" and "nothing to push" notes say the whole push did
            // nothing, which is untrue when the documents moved.
            if !docs_changed {
                for note in crate::ledger::push_notes(&outcome) {
                    notes(&note);
                }
            }
            PushReport {
                committed: outcome.committed,
                head: outcome.commit.clone(),
                already_on_remote: outcome.result == PushResult::AlreadyOnRemote,
                renumbered: Vec::new(),
            }
        };

        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        for attempt in 1..=MAX_ATTEMPTS {
            // The first attempt goes on what the refresh of this command fetched, or what
            // the record of the last fetch says is current. If origin moved since, the push
            // is rejected and the next attempt fetches, after a short random wait so two
            // writers that collided do not collide again.
            let mut reads_hold = false;
            if attempt == 1 {
                let fetches = self.synced.borrow().fetched_here;
                self.sync_data(db, location)?;
                // The reads the commit's batch made hold when the sync fetched nothing.
                reads_hold = prefetched.is_some() && self.synced.borrow().fetched_here == fetches;
            } else {
                // The wait keeps two writers that collided from colliding again. A push
                // that went on the record of the last fetch and was rejected found the
                // record out of date, which no other writer is doing at that moment.
                if !self.rejection_is_expected(attempt - 1) {
                    std::thread::sleep(transport::retry_wait());
                }
                // Origin moved since: what an earlier fetch brought is out of date.
                self.synced.borrow_mut().brought = None;
                if self.store_transport(db) == Transport::FileRemote {
                    self.fetch_file_remote(db, location)?;
                } else {
                    db.run(["fetch", REMOTE])?;
                    self.note_data_fetched(location, None);
                }
            }
            // After the sync: a store on a git remote moves to a file remote there when
            // origin holds one now.
            let file_remote = self.store_transport(db) == Transport::FileRemote;
            // The branch check, the merge base and the remote tip read in one process.
            // When the batch fails (no remote branch yet, say), they read one by one.
            let flags_query = flags_query(&remote_ref);
            let batched = match prefetched.filter(|_| reads_hold) {
                Some(sets) => Some(sets.to_vec()),
                None => db
                    .rows_each(&[
                        remote_branch_query(),
                        flags_query.clone(),
                        hash_query(&remote_ref),
                        hash_query("HEAD"),
                    ])
                    .ok(),
            }
            .filter(|sets| dolt::first_count(&sets[0]) > 0);
            // The commit `HEAD` is at, while nothing has moved it.
            let mut head_before = batched
                .as_ref()
                .and_then(|sets| sets[3].first())
                .and_then(|row| text(row, "h"));
            let known = batched.map(|sets| (sets[1].first().cloned(), sets[2].first().cloned()));
            if let Some((flags, tip)) = known {
                let (head_in_remote, remote_in_head) = flags
                    .map(|row| (flag(&row, "head_in_remote"), flag(&row, "remote_in_head")))
                    .unwrap_or((false, false));
                if head_in_remote {
                    let tip = tip
                        .and_then(|row| text(&row, "h"))
                        .ok_or_else(|| format!("error: dolt could not resolve {}", remote_ref))?;
                    return Ok(finish(outcome(PushResult::AlreadyOnRemote, tip), notes));
                }
                if !remote_in_head {
                    head_before = merge_renumbering(db, &remote_ref, renumbered)?;
                }
            } else if remote_branch_exists(db)? {
                let rows = db.rows(&flags_query);
                let (head_in_remote, remote_in_head) = match rows {
                    Ok(rows) => rows
                        .first()
                        .map(|row| (flag(row, "head_in_remote"), flag(row, "remote_in_head")))
                        .unwrap_or((false, false)),
                    Err(_) => (false, false),
                };
                if head_in_remote {
                    let tip = db.hash_of(&remote_ref)?;
                    return Ok(finish(outcome(PushResult::AlreadyOnRemote, tip), notes));
                }
                if !remote_in_head {
                    head_before = merge_renumbering(db, &remote_ref, renumbered)?;
                }
            }

            // The commit origin is known to hold. A file remote forgets it now: from the `dolt
            // push` until the `git push` has gone through, the directory and the store are
            // ahead of origin, and a process that dies in between must fetch next time.
            let known_tip = self
                .tips()
                .and_then(|tips| tips.get(location.remote(), location.ref_name()));
            if file_remote {
                self.forget_data_tip(location);
            }
            let (args, pushed) = db.output(["push", REMOTE, BRANCH])?;
            if pushed.status.success() && file_remote {
                match transport::publish(
                    location,
                    &transport::remote_dir_beside(db.dir()),
                    message,
                    known_tip.as_deref(),
                )? {
                    Published::Unchanged(commit) | Published::Pushed(commit) => {
                        self.note_file_published(location, &commit);
                        let head = match head_before {
                            Some(head) => head,
                            None => db.head_hash()?,
                        };
                        return Ok(finish(
                            outcome(PushResult::Pushed { attempts: attempt }, head),
                            notes,
                        ));
                    }
                    Published::Rejected(summary) => {
                        if attempt < MAX_ATTEMPTS && !self.rejection_is_expected(attempt) {
                            notes(&crate::ledger::retry_note(
                                attempt,
                                &PushStatus { flag: '!', summary },
                            ));
                        }
                        continue;
                    }
                }
            }
            if pushed.status.success() {
                self.note_data_pushed(db, location);
                let head = match head_before {
                    Some(head) => head,
                    None => db.head_hash()?,
                };
                return Ok(finish(
                    outcome(PushResult::Pushed { attempts: attempt }, head),
                    notes,
                ));
            }
            let stderr = String::from_utf8_lossy(&pushed.stderr).to_lowercase();
            if location.refs() == crate::ledger::refs::LedgerRefs::Custom
                && crate::ledger::refs::is_server_refusal(&stderr)
            {
                return Err(crate::ledger::refs::refused_push(
                    location.ref_name(),
                    &dolt::failure_message(&args, &pushed),
                ));
            }
            let stale = ["rejected", "behind", "non-fast-forward"]
                .iter()
                .any(|word| stderr.contains(word));
            if !stale {
                return Err(dolt::failure_message(&args, &pushed));
            }
            if attempt < MAX_ATTEMPTS && !self.rejection_is_expected(attempt) {
                notes(&crate::ledger::retry_note(
                    attempt,
                    &PushStatus {
                        flag: '!',
                        summary: "[rejected] (non-fast-forward)".to_string(),
                    },
                ));
            }
        }
        Err(format!(
            "error: {} on {} rejected the push {} times; it moved between each fetch and push.\n       \
             The commit stays in {}. Re-run tsk ledger push to try again",
            location.ref_name(),
            location.label(),
            MAX_ATTEMPTS,
            path.display()
        ))
    }
}

/// The statement that appends a line to a thread's continuation log. The next number is
/// worked out inside the write, so the read and the write cannot be separated by another
/// writer.
fn continuation_insert(id: &str, line: &str) -> String {
    format!(
        "insert into continuation_entries (thread_id, seq, entry) \
         select {t}, coalesce(max(seq), 0) + 1, {e} \
         from continuation_entries where thread_id = {t};",
        t = lit(id),
        e = lit(line)
    )
}

/// The bare repository Dolt keeps for the database's `origin` remote: the one under
/// `.dolt/git-remote-cache` whose `origin` URL is the one `repo_state.json` records. A
/// database that moved to a new URL has an older repository beside it, which is not it.
/// When several match, the one whose anchor ref was written last is.
fn cache_repo(db: &Dolt, anchor: &str) -> Option<PathBuf> {
    let state = std::fs::read_to_string(db.dir().join(".dolt").join("repo_state.json")).ok()?;
    let state: serde_json::Value = serde_json::from_str(&state).ok()?;
    let url = state.get("remotes")?.get(REMOTE)?.get("url")?.as_str()?;
    let url = url.strip_prefix("git+").unwrap_or(url);
    let wanted = format!("url = {}", url);
    let cache = db.dir().join(".dolt").join("git-remote-cache");
    let written = |repo: &Path| {
        [repo.join("packed-refs"), repo.join(anchor)]
            .iter()
            .filter_map(|file| std::fs::metadata(file).ok()?.modified().ok())
            .max()
    };
    std::fs::read_dir(&cache)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("repo.git"))
        .filter(|repo| {
            std::fs::read_to_string(repo.join("config"))
                .is_ok_and(|config| config.lines().any(|line| line.trim() == wanted))
        })
        .max_by_key(|repo| written(repo))
}

/// The commit Dolt's cache of the remote records as the last one it wrote or read for the
/// ledger ref, when the cache can be found and holds one. Called after a successful push,
/// where it is the commit the push made.
fn pushed_commit(db: &Dolt, location: &LedgerLocation) -> Option<String> {
    let name = location.ref_name().strip_prefix("refs/")?;
    let reference = format!("refs/dolt/remotes/{}/{}/last", REMOTE, name);
    let repo = cache_repo(db, &reference)?;
    let git_dir = repo.to_string_lossy().into_owned();
    let spec = format!("{}^{{commit}}", reference);
    Git::new(db.dir())
        .run_line([
            "--git-dir",
            git_dir.as_str(),
            "rev-parse",
            "--verify",
            "--quiet",
            spec.as_str(),
        ])
        .ok()
        .filter(|commit| !commit.is_empty())
}

/// The merge base of the local and remote branches, and the commits on each side.
fn merge_base_queries(remote_ref: &str) -> [String; 3] {
    [
        format!("select dolt_merge_base('HEAD', {}) as b", lit(remote_ref)),
        format!(
            "select count(*) as c from dolt_log({})",
            lit(&format!("{}..{}", remote_ref, BRANCH))
        ),
        format!(
            "select count(*) as c from dolt_log({})",
            lit(&format!("{}..{}", BRANCH, remote_ref))
        ),
    ]
}

/// The base, the commits ahead and the commits behind, in the results of
/// [`merge_base_queries`].
fn merge_base_from(sets: &[Vec<serde_json::Value>]) -> (String, usize, usize) {
    (
        sets[0]
            .first()
            .and_then(|row| text(row, "b"))
            .unwrap_or_else(|| "HEAD".to_string()),
        dolt::first_count(&sets[1]) as usize,
        dolt::first_count(&sets[2]) as usize,
    )
}

/// What the first two reads of the data side of `status` found, when they came with the
/// manifest check in one process.
struct FirstReads {
    local_version: Option<i64>,
    base: String,
    ahead: usize,
    behind: usize,
}

/// How far `status` asked origin: it fetched, it could not, or it was told not to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Fetched,
    Unreachable,
    NotAsked,
}

fn join_notes(existing: Option<String>, note: String) -> String {
    match existing {
        Some(first) => format!("{}\n{}", first, note),
        None => note,
    }
}

fn is_database(path: &Path) -> bool {
    path.join(".dolt").is_dir()
}

/// Dolt's URL for a git remote: `git+` and the git URL, with local paths made absolute
/// and the scp-like SSH form turned into a `ssh://` URL.
pub fn dolt_url(raw: &str, base: &Path) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("error: the ledger remote URL is empty".to_string());
    }
    for scheme in ["https://", "http://", "ssh://"] {
        if raw.starts_with(scheme) {
            return Ok(format!("git+{}", raw));
        }
    }
    if let Some(path) = raw.strip_prefix("file://") {
        return Ok(format!("git+file://{}", absolute(path, base)));
    }
    if raw.contains("://") {
        return Err(format!(
            "error: the ledger remote {} uses a scheme Dolt cannot reach; use https, ssh or a local path",
            raw
        ));
    }
    if let Some((host, path)) = raw.split_once(':') {
        if !host.is_empty() && !host.contains('/') && !host.starts_with('.') {
            return Ok(format!(
                "git+ssh://{}/{}",
                host,
                path.trim_start_matches('/')
            ));
        }
    }
    Ok(format!("git+file://{}", absolute(raw, base)))
}

fn absolute(path: &str, base: &Path) -> String {
    let joined = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        base.join(path)
    };
    std::fs::canonicalize(&joined)
        .unwrap_or(joined)
        .to_string_lossy()
        .into_owned()
}

/// The URL of the ledger's remote, as Dolt reaches it.
pub(crate) fn remote_url(repo: &Repo, location: &LedgerLocation) -> Result<String, String> {
    let base = repo
        .git
        .run_line(["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo.common_dir.clone());
    let raw = if location.repo_id().is_some() {
        location.remote().to_string()
    } else {
        repo.git
            .run_line(["remote", "get-url", location.remote()])?
    };
    dolt_url(&raw, &base)
}

/// The commit the remote ref points at, or `None` when the ref is absent.
pub(crate) fn remote_tip(git: &Git, location: &LedgerLocation) -> Result<Option<String>, String> {
    let (args, output) = git.output([
        "ls-remote",
        "--exit-code",
        location.remote(),
        location.ref_name(),
    ])?;
    match output.status.code() {
        Some(0) => Ok(Some(
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string(),
        )),
        Some(2) => Ok(None),
        _ => Err(crate::ledger::git::failure_message(&args, &output)),
    }
}

/// Create a new ledger database at `path`: the schema and the manifest rows. The result
/// is committed and has the remote configured, so the first push creates the branch. The
/// directory is removed again when any step fails.
pub fn create_database(
    path: &Path,
    url: &str,
    ref_name: Option<&str>,
    repo_id: Option<&str>,
) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("error: could not create {}: {}", path.display(), e))?;
    let build = || -> Result<(), String> {
        let db = Dolt::new(path);
        db.run(["init", "--initial-branch", BRANCH])?;
        db.exec(&dolt::init_sql(repo_id))?;
        db.run(["add", "-A"])?;
        db.run(["commit", "-m", "Create the ledger"])?;
        match ref_name {
            Some(ref_name) => db.run(["remote", "add", "--ref", ref_name, REMOTE, url])?,
            None => db.run(["remote", "add", REMOTE, url])?,
        };
        Ok(())
    };
    build().inspect_err(|_| {
        let _ = std::fs::remove_dir_all(path);
    })
}

fn remote_query() -> String {
    format!(
        "select url, params from dolt_remotes where name = {}",
        lit(REMOTE)
    )
}

fn verify_remote(db: &Dolt, url: &str, location: &LedgerLocation) -> Result<(), String> {
    verify_remote_rows(db, &db.rows(&remote_query())?, url, location)
}

fn verify_remote_rows(
    db: &Dolt,
    rows: &[serde_json::Value],
    url: &str,
    location: &LedgerLocation,
) -> Result<(), String> {
    let row = rows.first().ok_or_else(|| {
        format!(
            "error: {} is a Dolt database with no '{}' remote; move it aside and re-run",
            db.dir().display(),
            REMOTE
        )
    })?;
    let found_url = text(row, "url").unwrap_or_default();
    let found_ref = row
        .get("params")
        .and_then(|p| p.get("git_ref"))
        .and_then(|r| r.as_str())
        .unwrap_or("");
    let file_remote = Transport::of_url(url) == Transport::FileRemote;
    if found_url != url || (!file_remote && found_ref != location.ref_name()) {
        return Err(format!(
            "error: {} points at {} ({}), not {} ({}); move it aside and re-run",
            db.dir().display(),
            found_url,
            found_ref,
            url,
            location.ref_name()
        ));
    }
    Ok(())
}

fn pending_query() -> String {
    format!(
        "select commit_hash, message from dolt_log({})",
        lit(&format!("{}/{}..{}", REMOTE, BRANCH, BRANCH))
    )
}

fn pending_commits(db: &Dolt) -> Result<Vec<String>, String> {
    Ok(pending_from_rows(&db.rows(&pending_query())?))
}

fn pending_from_rows(rows: &[serde_json::Value]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let hash = text(row, "commit_hash").unwrap_or_default();
            let message = text(row, "message").unwrap_or_default();
            format!(
                "{} {}",
                hash.chars().take(7).collect::<String>(),
                message.lines().next().unwrap_or("")
            )
        })
        .collect()
}

/// Whether `HEAD` is in the remote branch and the remote branch is in `HEAD`.
fn flags_query(remote_ref: &str) -> String {
    format!(
        "select dolt_merge_base('HEAD', {r}) = hashof('HEAD') as head_in_remote, \
         dolt_merge_base('HEAD', {r}) = hashof({r}) as remote_in_head",
        r = lit(remote_ref)
    )
}

/// The four reads that open an attempt of the push: whether the remote branch exists, how
/// it stands against `HEAD`, the commit it is at and the commit `HEAD` is at. The two about
/// the remote branch name it in a `where` clause on `dolt_remote_branches`, so a database
/// with no remote branch answers them with no rows instead of failing.
fn push_reads() -> [String; 4] {
    let remote_ref = format!("{}/{}", REMOTE, BRANCH);
    let guard = format!(
        " from dolt_remote_branches where name = {}",
        lit(REMOTE_BRANCH_NAME)
    );
    [
        remote_branch_query(),
        format!("{}{}", flags_query(&remote_ref), guard),
        format!("select hash as h{}", guard),
        hash_query("HEAD"),
    ]
}

fn remote_branch_query() -> String {
    format!(
        "select count(*) as c from dolt_remote_branches where name = {}",
        lit(REMOTE_BRANCH_NAME)
    )
}

fn hash_query(revision: &str) -> String {
    format!("select hashof({}) as h", lit(revision))
}

fn remote_branch_exists(db: &Dolt) -> Result<bool, String> {
    Ok(db.count(&remote_branch_query())? > 0)
}

fn dirty_error(path: &Path) -> String {
    format!(
        "error: {} holds uncommitted changes; refusing to reset over them.\n       \
         Push them with tsk ledger push, or drop them deliberately with tsk ledger discard\n       \
         (tsk ledger status shows what would go). To read the path without refreshing, use tsk ledger path",
        path.display()
    )
}

impl DoltStore {
    /// The data side of `status`: the tables that differ from the merge base of the local
    /// and remote branches, with row counts, and the commits on each side. Changes
    /// nothing. `online` says the fetch worked, so the commit counts can be trusted.
    fn data_side(
        &self,
        db: &Dolt,
        online: Origin,
        first: Option<FirstReads>,
    ) -> Result<SideStatus, String> {
        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        // Each step reads in one `dolt sql` process. When a batch fails, the step reads
        // query by query instead, which gives the same result or the same error.
        let (has_remote, local_version, preread) = match first {
            Some(first) => (
                true,
                first.local_version,
                Some((first.base, first.ahead, first.behind)),
            ),
            None => match db.rows_each(&[remote_branch_query(), manifest_version_query(None)]) {
                Ok(sets) => (
                    dolt::first_count(&sets[0]) > 0,
                    version_of_rows(&sets[1]),
                    None,
                ),
                Err(_) => (
                    remote_branch_exists(db)?,
                    manifest_version_of(db, None),
                    None,
                ),
            },
        };
        let (base, remote) = if has_remote {
            let queries = merge_base_queries(&remote_ref);
            let (base, ahead, behind) = match preread {
                Some(read) => read,
                None => match db.rows_each(&queries) {
                    Ok(sets) => merge_base_from(&sets),
                    Err(_) => {
                        let base = db
                            .rows(&queries[0])
                            .ok()
                            .and_then(|rows| rows.first().and_then(|row| text(row, "b")))
                            .unwrap_or_else(|| "HEAD".to_string());
                        let ahead = db.count(&queries[1])? as usize;
                        let behind = db.count(&queries[2])? as usize;
                        (base, ahead, behind)
                    }
                },
            };
            let state = match online {
                Origin::Fetched | Origin::NotAsked => RemoteState::from_counts(ahead, behind),
                Origin::Unreachable => RemoteState::Unknown,
            };
            (base, state)
        } else if online == Origin::Fetched {
            ("HEAD".to_string(), RemoteState::Unpublished)
        } else {
            ("HEAD".to_string(), RemoteState::Unknown)
        };
        let summary_query = format!("select from_table_name, to_table_name, data_change, schema_change from dolt_diff_summary({}, 'WORKING')", lit(&base));
        let stats_query = format!("select table_name, rows_added, rows_modified, rows_deleted from dolt_diff_stat({}, 'WORKING')", lit(&base));
        let mut queries = vec![summary_query, stats_query];
        if has_remote {
            queries.push(manifest_version_query(Some(&remote_ref)));
        }
        let (summary, stats, origin_version) = match db.rows_each(&queries) {
            Ok(mut sets) => {
                let origin = sets.get(2).and_then(|rows| version_of_rows(rows));
                sets.truncate(2);
                let stats = sets.pop().unwrap_or_default();
                let summary = sets.pop().unwrap_or_default();
                (summary, stats, origin)
            }
            Err(_) => (
                db.rows(&queries[0])?,
                db.rows(&queries[1])?,
                if has_remote {
                    manifest_version_of(db, Some(&remote_ref))
                } else {
                    None
                },
            ),
        };
        let mut changes = Vec::new();
        for row in &summary {
            let name = text(row, "to_table_name")
                .filter(|n| !n.is_empty())
                .or_else(|| text(row, "from_table_name"))
                .unwrap_or_default();
            let stat = stats
                .iter()
                .find(|stat| text(stat, "table_name").as_deref() == Some(name.as_str()));
            let number = |column: &str| stat.and_then(|stat| integer(stat, column)).unwrap_or(0);
            let schema = flag(row, "schema_change");
            changes.push(Change {
                name,
                kind: ChangeKind::Table {
                    added: number("rows_added"),
                    modified: number("rows_modified"),
                    deleted: number("rows_deleted"),
                    schema,
                },
            });
        }
        changes.sort_by(|a, b| a.name.cmp(&b.name));
        // A schema upgrade is the one change a write makes that the person did not ask
        // for: label it, and drop the lines it caused.
        if has_remote {
            if let (Some(local), Some(origin)) = (local_version, origin_version) {
                if local != origin {
                    changes = status::label_upgrade(changes, local, origin, &mut |table| {
                        edited_rows(db, &base, table)
                    })?;
                    // The version 8 upgrade copies the cloud rows into `bindings` and drops
                    // the old table: the copies are not changes the person made.
                    if origin < dolt::BINDINGS_LAYOUT_VERSION
                        && local >= dolt::BINDINGS_LAYOUT_VERSION
                    {
                        let copied = db
                            .count(&format!(
                                "select count(*) as c from cloud_session_bindings as of {}",
                                plain(&base)
                            ))
                            .unwrap_or(0);
                        changes = status::hide_copied_rows(
                            changes,
                            "cloud_session_bindings",
                            "bindings",
                            copied,
                        );
                    }
                }
            }
        }
        Ok(SideStatus { changes, remote })
    }

    /// The manifest check and the first reads of the data side of `status` in one process,
    /// for a status that fetches nothing. `Ok(None)` when the batch fails or the database
    /// has no remote branch: the check and the reads then run on their own, which gives
    /// the same result or the same error. A ledger the manifest check refuses fails here
    /// as it does there.
    fn status_first_reads(
        &self,
        db: &Dolt,
        location: &LedgerLocation,
    ) -> Result<Option<FirstReads>, String> {
        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        let mut queries = vec![
            MANIFEST_QUERY.to_string(),
            remote_branch_query(),
            manifest_version_query(None),
        ];
        queries.extend(merge_base_queries(&remote_ref));
        let Ok(sets) = db.rows_each(&queries) else {
            return Ok(None);
        };
        let info = manifest_from_rows(&sets[0])?;
        let version = manifest::check_values(
            info.version,
            info.repo_id.as_deref(),
            location.repo_id(),
            MANIFEST_SOURCE,
            DOLT_SUPPORTED_VERSIONS,
        )?;
        if version == LEGACY_LAYOUT_VERSION {
            return Err(VERSION_1_REFUSAL.to_string());
        }
        self.remember_checked(db, version);
        if dolt::first_count(&sets[1]) == 0 {
            return Ok(None);
        }
        let (base, ahead, behind) = merge_base_from(&sets[3..]);
        Ok(Some(FirstReads {
            local_version: version_of_rows(&sets[2]),
            base,
            ahead,
            behind,
        }))
    }

    /// Refresh the Dolt database, and return the manifest version it holds.
    pub(crate) fn refresh_data(&self, keep_local: bool) -> Result<(RefreshOutcome, i64), String> {
        self.forget_checked();
        self.forget_snapshot();
        self.read_ahead.set(keep_local);
        dolt::ensure_identity()?;
        let location = self.repo.location(&self.state_root)?;
        let clone_id = self.repo.ensure_clone_id()?;
        let path = dolt_db_path(&self.state_root, &clone_id);
        let url = self.expected_url(&path, &location)?;
        let remote_label = format!("{}'s {}", location.label(), location.ref_name());
        let present = is_database(&path);
        if path.exists() && !present {
            return Err(format!(
                "error: {} exists but is not a Dolt database; move it aside and re-run",
                path.display()
            ));
        }

        // A clone with a record of the last fetch asks origin, and reads its database in
        // the same breath: the question waits for the network, and the reads wait for
        // nothing, so they run meanwhile. The reads hold when origin has nothing new. When
        // it has, they still say whether the working set holds changes.
        let mut speculated: Option<DataState> = None;
        let speculate = present
            && !self.trust_tips.get()
            && self.synced.borrow().heads.is_none()
            && !self.has_no_record(&location)
            && self.file_checkout_present();
        if speculate {
            let git = self.repo.git.clone();
            let asking = location.clone();
            let (asked, read) = std::thread::scope(|scope| {
                let ask = scope.spawn(|| tips::remote_heads(&git, &asking).ok());
                let read = self.read_state(
                    &Dolt::store(&path),
                    &location,
                    true,
                    Some((url.as_str(), path.as_path())),
                    false,
                    keep_local,
                );
                (ask.join().ok().flatten(), read)
            });
            self.synced.borrow_mut().heads = Some(asked);
            speculated = read.ok().flatten();
        }

        // One question to origin names the ledger ref and the documents ref. When it cannot
        // be asked, the ledger ref is asked alone, which reports the failure as before.
        let ledger_tip = match self.heads(&location) {
            Some(heads) => heads.ledger,
            None => remote_tip(&self.repo.git, &location)?,
        };
        if ledger_tip.is_none() {
            crate::ledger::refs::check_origin_has_no_other(&self.repo, &location)?;
            let version = if present {
                let db = Dolt::store(&path);
                verify_remote(&db, &url, &location)?;
                self.check_manifest_of(&db, &location)?
            } else {
                self.create_new_database(&path, &location)?;
                dolt::LAYOUT_VERSION
            };
            let head = Dolt::store(&path).head_hash()?;
            return Ok((
                RefreshOutcome {
                    unpublished_note: Some(crate::ledger::unpublished_text(
                        location.label(),
                        location.ref_name(),
                        &path,
                    )),
                    path,
                    remote_label,
                    remote_head: head,
                    pending: Vec::new(),
                    unpublished: true,
                    pending_note: None,
                },
                version,
            ));
        }

        let db;
        let state;
        if present {
            db = Dolt::store(&path);
            // When origin has nothing new, one process reads everything the refresh needs.
            // Otherwise the remote check and the dirty check come first, then the fetch,
            // then the reads. When a batch fails, the reads go one by one, which gives the
            // same result or error.
            let mut found = None;
            let speculated_dirty = speculated.as_ref().map(|state| state.dirty);
            if self.data_is_current(&location) {
                found = match speculated.take() {
                    Some(state) => Some(state),
                    None => self.read_state(
                        &db,
                        &location,
                        true,
                        Some((url.as_str(), path.as_path())),
                        false,
                        keep_local,
                    )?,
                };
            }
            if found.is_none() {
                let dirty = match speculated_dirty {
                    Some(dirty) => dirty,
                    None => match db.rows_each(&[remote_query(), DIRTY_QUERY.to_string()]) {
                        Ok(sets) => {
                            verify_remote_rows(&db, &sets[0], &url, &location)?;
                            dolt::first_count(&sets[1]) > 0
                        }
                        Err(_) => {
                            verify_remote(&db, &url, &location)?;
                            db.count(DIRTY_QUERY)? > 0
                        }
                    },
                };
                if dirty && !keep_local {
                    return Err(dirty_error(&path));
                }
                self.sync_data(&db, &location)?;
                found = self.read_state(&db, &location, true, None, dirty, keep_local)?;
                if found.is_none() {
                    found =
                        Some(self.read_state_one_by_one(&db, &location, true, dirty, keep_local)?);
                }
            }
            state = found.expect("the state was read");
        } else {
            let (kind, commit) = self.remote_kind(&location)?;
            let parent = path
                .parent()
                .ok_or_else(|| format!("error: {} has no parent directory", path.display()))?;
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match kind {
                Transport::FileRemote => {
                    let dir = transport::remote_dir_beside(&path);
                    transport::sync_checkout(&self.repo, &dir, &commit)?;
                    Dolt::new(parent).run(["clone", transport::file_url(&dir).as_str(), &name])?;
                    transport::write_ref_file(parent, location.ref_name())?;
                    self.note_data_fetched(&location, Some(&commit));
                }
                Transport::GitRemote => {
                    Dolt::new(parent).run([
                        "clone",
                        "--ref",
                        location.ref_name(),
                        url.as_str(),
                        &name,
                    ])?;
                    self.note_data_fetched(&location, ledger_tip.as_deref());
                }
            }
            db = Dolt::store(&path);
            let mut found = self.read_state(&db, &location, false, None, false, keep_local)?;
            if found.is_none() {
                found = Some(self.read_state_one_by_one(&db, &location, false, false, keep_local)?);
            }
            state = found.expect("the state was read");
        }
        let DataState {
            remote_head,
            pending,
            version,
            ..
        } = state;
        let pending_note = (!pending.is_empty()).then(|| {
            let mut note = format!(
                "note: {} holds a commit that is not on {}:\n",
                path.display(),
                remote_label
            );
            for line in &pending {
                note.push_str(&format!("        {}\n", line));
            }
            note.push_str("      Leaving the database as it is rather than resetting over it.\n");
            note.push_str(
                "      Push it with tsk ledger push. To see what would be lost, run tsk ledger status.\n",
            );
            note.push_str("      To drop it deliberately, run tsk ledger discard.");
            note
        });
        Ok((
            RefreshOutcome {
                path,
                remote_label,
                remote_head,
                pending,
                unpublished: false,
                pending_note,
                unpublished_note: None,
            },
            version,
        ))
    }
}

impl LedgerStore for DoltStore {
    fn refresh(&self) -> Result<RefreshOutcome, String> {
        self.refresh_with(false, false)
    }

    fn refresh_for_fetch(&self) -> Result<RefreshOutcome, String> {
        self.refresh_with(true, false)
    }

    fn refresh_keeping_local(&self) -> Result<RefreshOutcome, String> {
        self.refresh_with(false, true)
    }

    fn has_local_changes(&self) -> Result<bool, String> {
        let db = self.db()?;
        let dirty = match self.read_snapshot(&db, |snapshot| snapshot.dirty)? {
            Some(dirty) => dirty,
            None => db.count(DIRTY_QUERY)? > 0,
        };
        if dirty {
            return Ok(true);
        }
        let docs = Git::new(&self.docs_tree()?.root);
        Ok(!docs.run(["status", "--porcelain=v2", "-z"])?.is_empty())
    }

    fn status(&self) -> Result<LedgerStatus, String> {
        let db = self.db()?;
        let location = self.repo.location(&self.state_root)?;
        // When origin has nothing new, the manifest check and the first reads are one process.
        let first = if self.data_is_current(&location) {
            self.status_first_reads(&db, &location)?
        } else {
            None
        };
        if first.is_none() {
            self.check_current(&db, &location)?;
        }
        let docs = Git::new(&self.docs_tree()?.root);

        let data_fetch = self.sync_data(&db, &location);
        let docs_fetch = self.sync_docs_tip(&location);
        let data = self.data_side(
            &db,
            if data_fetch.is_ok() {
                Origin::Fetched
            } else {
                Origin::Unreachable
            },
            first,
        )?;
        let side = status::worktree_side(&docs, &docs_fetch)?;
        let docs_ref = location.docs();
        let fetch_note = match (&data_fetch, &docs_fetch) {
            (Err(e), _) => Some(status::fetch_failed_note(
                &format!("{}'s {}", location.label(), location.ref_name()),
                e,
            )),
            (_, Err(e)) => Some(status::fetch_failed_note(
                &format!("{}'s {}", docs_ref.label(), docs_ref.ref_name()),
                e,
            )),
            _ => None,
        };
        Ok(LedgerStatus {
            data,
            docs: SideStatus {
                changes: side
                    .files
                    .iter()
                    .map(|(letter, path)| Change::file(*letter, path))
                    .collect(),
                remote: side.remote,
            },
            fetch_note,
            origin_checked: true,
        })
    }

    fn status_local(&self) -> Result<LedgerStatus, String> {
        let db = self.db()?;
        let location = self.repo.location(&self.state_root)?;
        let first = self.status_first_reads(&db, &location)?;
        if first.is_none() {
            self.check_current(&db, &location)?;
        }
        let docs = Git::new(&self.docs_tree()?.root);

        let data = self.data_side(&db, Origin::NotAsked, first)?;
        // The documents tip last fetched, when this clone recorded it and still has the commit.
        let docs_ref = location.docs();
        let tip = self
            .tips()
            .and_then(|tips| tips.get(docs_ref.remote(), docs_ref.ref_name()))
            .filter(|tip| tips::commit_is_local(&self.repo.git, tip));
        let fetched = tip
            .map(Some)
            .ok_or_else(|| "origin was not asked".to_string());
        let side = status::worktree_side(&docs, &fetched)?;
        Ok(LedgerStatus {
            data,
            docs: SideStatus {
                changes: side
                    .files
                    .iter()
                    .map(|(letter, path)| Change::file(*letter, path))
                    .collect(),
                remote: side.remote,
            },
            fetch_note: None,
            origin_checked: false,
        })
    }

    fn discard(&self) -> Result<(), String> {
        self.forget_checked();
        self.forget_snapshot();
        let db = self.db()?;
        let location = self.repo.location(&self.state_root)?;
        self.check_current(&db, &location)?;
        let docs = Git::new(&self.docs_tree()?.root);
        // Both remote states are fetched before either side changes, so a fetch that
        // fails leaves everything as it was.
        let refused = |e: String| {
            format!(
                "{}\n       Nothing was discarded: the ledger cannot be reset to a state that was not fetched",
                e
            )
        };
        self.sync_data(&db, &location).map_err(refused)?;
        let docs_tip = self.sync_docs_tip(&location).map_err(refused)?;

        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        let target = if remote_branch_exists(&db)? {
            remote_ref.as_str()
        } else {
            "HEAD"
        };
        db.run(["reset", "--hard", target])?;
        // A reset leaves a table that was never committed. Clean drops it.
        db.run(["clean"])?;
        status::discard_worktree(&docs, docs_tip.as_deref())
    }

    fn path(&self) -> Result<PathBuf, String> {
        let clone_id = self.repo.read_clone_id()?.ok_or_else(|| {
            format!(
                "error: this clone has no clone id yet ({} is absent or empty).\n       Run tsk ledger fetch, which mints it.",
                self.repo.clone_id_file().display()
            )
        })?;
        Ok(dolt_db_path(&self.state_root, &clone_id))
    }

    fn exists_locally(&self) -> Result<bool, String> {
        Ok(self.hint().is_some_and(|path| is_database(&path)))
    }

    fn commit_and_push(
        &self,
        message: &str,
        notes: &mut dyn FnMut(&str),
    ) -> Result<PushReport, String> {
        dolt::ensure_identity()?;
        // One push at a time for this clone: a background publish may be running.
        let _push_lock = match self.path() {
            Ok(path) => Some(crate::ledger::background::lock_for_push(&path)?),
            Err(_) => None,
        };
        let location = self.repo.location(&self.state_root)?;
        let db = self.db()?;
        self.check_current(&db, &location)?;
        let docs_path = self.docs_tree()?.root;

        // Stage everything, count the staged tables, list the document links and commit
        // what is staged, in one process. When that fails (a ledger below the layout that
        // has the link table, say), the steps run in two processes, and when that fails
        // they run one by one. Each gives the same result or the same error.
        //
        // Writes a command held back (see `continuation_append_deferred`) go first in the
        // same process. When the push would not fetch first, the reads that open its first
        // attempt follow the commit, so the push does not start a process for them.
        self.forget_snapshot();
        let queued = std::mem::take(&mut *self.queued.borrow_mut());
        let writes: Vec<String> = queued.iter().map(|write| write.sql.clone()).collect();
        let staged_query = "select count(*) as c from dolt_status where staged = 1";
        let links_query = "select path from mission_documents";
        let mut queries = vec![
            "call dolt_add('-A')".to_string(),
            staged_query.to_string(),
            links_query.to_string(),
            format!("call dolt_commit('--skip-empty', '-m', {})", lit(message)),
        ];
        let reads_ride = self.data_is_current(&location);
        if reads_ride {
            queries.extend(push_reads());
        }
        let all_in_one = db.rows_each_after(&writes, &queries);
        let mut prefetched = None;
        let (committed, links) = match all_in_one {
            Ok(mut sets) => {
                if reads_ride {
                    prefetched = Some(sets.split_off(4));
                }
                (
                    dolt::first_count(&sets[1]) > 0,
                    Some(
                        sets[2]
                            .iter()
                            .filter_map(|row| text(row, "path"))
                            .collect::<Vec<_>>(),
                    ),
                )
            }
            Err(_) => {
                // The batch stopped somewhere. The held back writes may have been made.
                for write in &queued {
                    if db.count(&write.applied_query)? == 0 {
                        db.exec(&write.sql)?;
                    }
                }
                let staged = match db
                    .rows_each(&["call dolt_add('-A')".to_string(), staged_query.to_string()])
                {
                    Ok(sets) => dolt::first_count(&sets[1]),
                    Err(_) => {
                        db.run(["add", "-A"])?;
                        db.count(staged_query)?
                    }
                };
                let committed = staged > 0;
                if committed {
                    db.run(["commit", "-m", message])?;
                }
                (committed, None)
            }
        };

        // The documents go first, then the Dolt data. A record must never name a document
        // the remote lacks: a reader who finds the record can then always open the
        // document. The reverse, a document the remote holds and no record names, is
        // harmless. So when the documents push fails (rejected after the retries, or a
        // rebase conflict), this stops here and the Dolt data is not pushed. Both sides
        // keep their local commits, and the next tsk ledger push resumes. Each side
        // pushes only when it has a change or an unpushed commit: a side that is already
        // on the remote sends nothing.
        let docs_outcome = {
            let mut on_retry = |attempt: usize, status: &PushStatus| {
                if !self.rejection_is_expected(attempt) {
                    notes(&crate::ledger::retry_note(attempt, status))
                }
            };
            // The tip the refresh of this command fetched, else the one the record of the
            // last fetch names. The push carries it as its lease.
            let known = self.sync_docs_tip(&location)?;
            let outcome = docs::push(
                &self.repo,
                &location,
                &docs_path,
                message,
                Some(known),
                &mut on_retry,
            )?;
            self.note_docs_tip(&location, Some(outcome.commit.as_str()));
            outcome
        };
        let docs_changed =
            docs_outcome.committed || matches!(docs_outcome.result, PushResult::Pushed { .. });

        // Every document a mission links must still exist. The documents are already
        // pushed, so this stops before the Dolt data goes and keeps both local commits.
        // The linked paths came with the commit, so when each of them exists the missions
        // are not read. A path that is missing, or no list, sends the check through the
        // missions, which names the mission and the first path.
        let docs_tree = self.docs_tree()?;
        let all_exist = links
            .as_ref()
            .is_some_and(|paths| paths.iter().all(|path| docs_tree.doc_path(path).is_file()));
        if !all_exist {
            model::check_links(
                &self.missions_in(&db, &location)?,
                &|path| docs_tree.doc_path(path).is_file(),
                "The documents are pushed. The Dolt data stays committed locally and is not pushed yet",
            )?;
        }

        let request = PushRequest {
            committed,
            docs_changed,
            message,
            prefetched: prefetched.as_deref(),
        };
        let mut report = self.push_data(&location, &db, request, notes)?;
        if docs_changed {
            report.committed = report.committed || docs_outcome.committed;
            if report.already_on_remote {
                // Only the documents changed: the commit the remote now holds is theirs.
                report.head = docs_outcome.commit;
                report.already_on_remote = false;
            }
        }
        Ok(report)
    }

    fn head_id(&self) -> Result<String, String> {
        let db = self.db()?;
        match self.read_snapshot(&db, |snapshot| snapshot.head.clone())? {
            Some(head) => Ok(head),
            None => db.head_hash(),
        }
    }

    fn doc_get(&self, path: &str) -> Result<Option<String>, String> {
        let path = super::check_docs_path(path)?;
        self.docs_tree()?.doc_get(&path)
    }

    fn doc_put(&self, path: &str, body: &str) -> Result<(), String> {
        let path = super::check_docs_path(path)?;
        self.docs_tree()?.doc_put(&path, body)
    }

    fn doc_list(&self, prefix: &str) -> Result<Vec<String>, String> {
        self.docs_tree()?.doc_list(prefix)
    }

    fn doc_remove(&self, path: &str) -> Result<bool, String> {
        let path = super::check_docs_path(path)?;
        self.docs_tree()?.doc_remove(&path)
    }

    fn doc_exists(&self, path: &str) -> Result<bool, String> {
        match super::check_docs_path(path) {
            Ok(path) => Ok(self.docs_tree()?.doc_path(&path).is_file()),
            Err(_) => Ok(false),
        }
    }

    fn doc_path(&self, path: &str) -> Result<Option<PathBuf>, String> {
        let path = super::check_docs_path(path)?;
        let file = self.docs_tree()?.doc_path(&path);
        Ok(file.is_file().then_some(file))
    }

    fn thread_exists(&self, id: &str) -> Result<bool, String> {
        let db = self.db()?;
        if let Some(found) = self.read_snapshot(&db, |snapshot| {
            snapshot.threads.iter().any(|thread| thread.id == id)
        })? {
            return Ok(found);
        }
        Ok(db.count(&format!(
            "select count(*) as c from threads where thread_id = {}",
            lit(id)
        ))? > 0)
    }

    fn thread_create(
        &self,
        id: &str,
        mission_id: Option<&str>,
        briefing_path: Option<&str>,
    ) -> Result<(), String> {
        let db = self.db()?;
        let location = self.repo.location(&self.state_root)?;
        // A refused write upgrades nothing: the checks come first.
        let version = self.check_current(&db, &location)?;
        if self.thread_exists(id)? {
            return Err(format!("error: threads/{} already exists", id));
        }
        let mut sql = String::new();
        if version < dolt::LAYOUT_VERSION {
            sql.push_str(&self.upgrade_script(&db, version)?);
        }
        sql.push_str(&format!(
            "insert into threads (thread_id, briefing_path, mission_id) values ({}, {}, {});",
            lit(id),
            briefing_path.map_or("NULL".to_string(), lit),
            mission_id.map_or("NULL".to_string(), lit)
        ));
        self.write(&db, &sql)
    }

    fn thread_set_mission(
        &self,
        id: &str,
        mission_id: Option<&str>,
        briefing_path: Option<&str>,
    ) -> Result<(), String> {
        let db = self.db()?;
        let location = self.repo.location(&self.state_root)?;
        let version = self.check_current(&db, &location)?;
        if !self.thread_exists(id)? {
            return Err(format!("error: no such thread: {}", id));
        }
        let mut sql = String::new();
        if version < dolt::LAYOUT_VERSION {
            sql.push_str(&self.upgrade_script(&db, version)?);
        }
        sql.push_str(&format!(
            "update threads set briefing_path = {}, mission_id = {} where thread_id = {};",
            briefing_path.map_or("NULL".to_string(), lit),
            mission_id.map_or("NULL".to_string(), lit),
            lit(id)
        ));
        self.write(&db, &sql)
    }

    fn thread_delete(&self, id: &str) -> Result<(), String> {
        let db = self.db()?;
        self.write(
            &db,
            &format!(
                "delete from continuation_entries where thread_id = {t};\n\
                 delete from threads where thread_id = {t};",
                t = lit(id)
            ),
        )
    }

    fn thread_ids(&self) -> Result<Vec<String>, String> {
        let db = self.db()?;
        if let Some(mut ids) = self.read_snapshot(&db, |snapshot| {
            snapshot
                .threads
                .iter()
                .map(|thread| thread.id.clone())
                .collect::<Vec<_>>()
        })? {
            ids.sort();
            return Ok(ids);
        }
        let rows = db.rows("select thread_id from threads")?;
        let mut ids: Vec<String> = rows
            .iter()
            .filter_map(|row| text(row, "thread_id"))
            .collect();
        ids.sort();
        Ok(ids)
    }

    fn thread_info(&self, id: &str) -> Result<ThreadInfo, String> {
        let db = self.db()?;
        if let Some(info) = self.read_snapshot(&db, |snapshot| {
            snapshot
                .threads
                .iter()
                .find(|thread| thread.id == id)
                .map(|thread| ThreadInfo {
                    mission_id: thread.mission_id.clone(),
                    briefing_path: thread.briefing_path.clone(),
                })
                .unwrap_or_default()
        })? {
            return Ok(info);
        }
        // A ledger below version 5 has no `mission_id` column.
        let version = match &*self.checked.borrow() {
            Some((dir, version)) if dir == db.dir() => *version,
            _ => self.manifest_rows(&db)?.version,
        };
        let column = if version >= dolt::THREAD_MISSION_LAYOUT_VERSION {
            "mission_id"
        } else {
            "null as mission_id"
        };
        let rows = db.rows(&format!(
            "select briefing_path, {} from threads where thread_id = {}",
            column,
            lit(id)
        ))?;
        Ok(rows
            .first()
            .map(|row| ThreadInfo {
                mission_id: text(row, "mission_id").filter(|m| !m.is_empty()),
                briefing_path: text(row, "briefing_path").filter(|p| !p.is_empty()),
            })
            .unwrap_or_default())
    }

    fn continuation_append(&self, id: &str, line: &str) -> Result<(), String> {
        let db = self.db()?;
        if !self.thread_exists(id)? {
            return Err(format!(
                "error: no such thread: {} (expected {})",
                id,
                self.describe_thread(id)
            ));
        }
        self.write(&db, &continuation_insert(id, line))
    }

    fn continuation_append_deferred(&self, id: &str, line: &str) -> Result<(), String> {
        let db = self.db()?;
        if !self.thread_exists(id)? {
            return Err(format!(
                "error: no such thread: {} (expected {})",
                id,
                self.describe_thread(id)
            ));
        }
        // Held back only where the batch of the commit can carry it: a ledger at the
        // current layout, whose tables the batch reads. Any other ledger writes now.
        if self.layout_is_current(&db)? {
            self.forget_snapshot();
            self.queued.borrow_mut().push(QueuedWrite {
                sql: continuation_insert(id, line),
                applied_query: format!(
                    "select count(*) as c from continuation_entries where thread_id = {} and entry = {}",
                    lit(id),
                    lit(line)
                ),
            });
            return Ok(());
        }
        self.write(&db, &continuation_insert(id, line))
    }

    fn trust_last_fetch(&self) {
        self.trust_tips.set(true);
    }

    fn distrust_last_fetch(&self) {
        self.trust_tips.set(false);
        *self.synced.borrow_mut() = Synced::default();
    }

    fn continuation_list(&self, id: &str) -> Result<Vec<String>, String> {
        let rows = self.db()?.rows(&format!(
            "select entry from continuation_entries where thread_id = {} order by seq",
            lit(id)
        ))?;
        Ok(rows
            .iter()
            .filter_map(|row| text(row, "entry"))
            .filter(|entry| !entry.trim().is_empty())
            .collect())
    }

    fn continuation_lists(&self, ids: &[String]) -> Result<Vec<Vec<String>>, String> {
        let db = self.db()?;
        // Every thread's lines in one process. When the batch fails, they are read one
        // thread at a time, which gives the same lines or the same error.
        if let Ok(mut sets) = db.rows_each(&[
            "select thread_id, entry from continuation_entries order by thread_id, seq".to_string(),
        ]) {
            let mut by_thread: std::collections::BTreeMap<String, Vec<String>> =
                std::collections::BTreeMap::new();
            for row in sets.remove(0) {
                if let (Some(thread), Some(entry)) = (text(&row, "thread_id"), text(&row, "entry"))
                {
                    if !entry.trim().is_empty() {
                        by_thread.entry(thread).or_default().push(entry);
                    }
                }
            }
            return Ok(ids
                .iter()
                .map(|id| by_thread.remove(id).unwrap_or_default())
                .collect());
        }
        ids.iter().map(|id| self.continuation_list(id)).collect()
    }

    fn bindings_list(&self) -> Result<Vec<BindingRow>, String> {
        let db = self.db()?;
        if let Some(rows) = self.read_snapshot(&db, |snapshot| snapshot.bindings.clone())? {
            return Ok(rows);
        }
        // One query on a current ledger. A ledger below version 8 has no `bindings` table:
        // its cloud rows are read from the old table, and nothing is upgraded.
        let (rows, legacy) = match db.rows(BINDINGS_QUERY) {
            Ok(rows) => (rows, false),
            Err(e) if e.contains("table not found: bindings") => (
                db.rows(
                    "select session_id as actor, 'cloud' as kind, thread_id, registered_at \
                     from cloud_session_bindings order by registered_at, session_id",
                )?,
                true,
            ),
            Err(e) => return Err(e),
        };
        Ok(binding_rows(&rows, legacy))
    }

    fn binding_set(&self, row: &BindingRow) -> Result<(), String> {
        self.write_bindings(&format!(
            "replace into bindings (actor, kind, thread_id, registered_at) \
             values ({}, {}, {}, {});",
            lit(&row.actor),
            lit(row.kind.as_str()),
            lit(&row.thread_id),
            lit(&row.registered_at)
        ))
    }

    fn binding_remove(&self, actor: &str) -> Result<(), String> {
        self.write_bindings(&format!(
            "delete from bindings where actor = {};",
            lit(actor)
        ))
    }

    fn bindings_remove(&self, actors: &[String]) -> Result<(), String> {
        if actors.is_empty() {
            return Ok(());
        }
        let list: Vec<String> = actors.iter().map(|actor| lit(actor)).collect();
        self.write_bindings(&format!(
            "delete from bindings where actor in ({});",
            list.join(", ")
        ))
    }

    fn binding_remove_thread(&self, thread_id: &str) -> Result<(), String> {
        self.write_bindings(&format!(
            "delete from bindings where thread_id = {};",
            lit(thread_id)
        ))
    }

    fn events_append(&self, lines: &str) -> Result<(), String> {
        let items: Vec<&str> = lines
            .split_inclusive('\n')
            .map(|line| line.strip_suffix('\n').unwrap_or(line))
            .collect();
        if items.is_empty() {
            return Ok(());
        }
        let db = self.db()?;
        let last = db.count("select coalesce(max(position), 0) as m from external_events")?;
        let values: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(index, line)| format!("({}, {})", last + 1 + index as i64, lit(line)))
            .collect();
        self.write(
            &db,
            &format!(
                "insert into external_events (position, envelope) values {};",
                values.join(", ")
            ),
        )
    }

    fn events_list(&self) -> Result<Option<Vec<String>>, String> {
        let rows = self
            .db()?
            .rows("select envelope from external_events order by position")?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            rows.iter()
                .map(|row| text(row, "envelope").unwrap_or_default())
                .collect(),
        ))
    }

    fn watermark_get(&self) -> Result<u64, String> {
        let rows = self
            .db()?
            .rows("select processed_through from watermark where id = 1")?;
        Ok(rows
            .first()
            .and_then(|row| integer(row, "processed_through"))
            .unwrap_or(0)
            .max(0) as u64)
    }

    fn watermark_set(&self, processed_through: u64, updated_at: &str) -> Result<(), String> {
        let value = i64::try_from(processed_through)
            .map_err(|_| format!("error: watermark {} is too large", processed_through))?;
        let db = self.db()?;
        self.write(
            &db,
            &format!(
                "replace into watermark (id, processed_through, updated_at) values (1, {}, {});",
                value,
                lit(updated_at)
            ),
        )
    }

    fn missions_load(&self) -> Result<Vec<Mission>, String> {
        let location = self.repo.location(&self.state_root)?;
        self.missions_in(&self.db()?, &location)
    }

    fn mission_save(&self, mission: &Mission) -> Result<(), String> {
        let location = self.repo.location(&self.state_root)?;
        let db = self.db()?;
        let version = self.check_current(&db, &location)?;
        let mut sql = String::new();
        if version < dolt::LAYOUT_VERSION {
            // The first write on an older ledger: the missing tables and columns and the
            // new version land in the same local change as the mission.
            sql.push_str(&self.upgrade_script(&db, version)?);
        }
        sql.push_str(&save_sql(mission));
        self.write(&db, &sql)
    }

    fn describe_thread(&self, id: &str) -> String {
        self.describe(&format!("thread {}", id))
    }

    fn describe_threads(&self) -> String {
        self.describe("the threads table")
    }

    fn pending_inspect_hint(&self, path: &Path, _remote_head: &str) -> String {
        let data = format!(
            "cd '{}' && dolt log {r}/{b}..{b} && dolt diff {r}/{b} {b}",
            path.display(),
            r = REMOTE,
            b = BRANCH
        );
        match self.docs_hint() {
            Some(docs) => format!(
                "{}; for the documents: git -C '{}' log -p",
                data,
                docs.display()
            ),
            None => data,
        }
    }
}

impl DoltStore {
    /// Run a write on `bindings` in one transaction. A ledger below version 8 is upgraded
    /// first, in the same transaction, so the write lands on the new table. A refused
    /// write upgrades nothing: the checks come first.
    fn write_bindings(&self, sql: &str) -> Result<(), String> {
        let db = self.db()?;
        let location = self.repo.location(&self.state_root)?;
        let version = self.check_current(&db, &location)?;
        let mut script = String::new();
        if version < dolt::LAYOUT_VERSION {
            script.push_str(&self.upgrade_script(&db, version)?);
        }
        script.push_str(sql);
        self.write(&db, &script)
    }

    /// The statements that take a ledger at `version` to the newest layout. From versions 3
    /// to 6 the old text of every mission's constraints and out-of-scope columns is read
    /// here and split into items, which the statements insert before they drop the columns.
    fn upgrade_script(&self, db: &Dolt, version: i64) -> Result<String, String> {
        let moved = if (dolt::MISSIONS_LAYOUT_VERSION..dolt::LIST_LAYOUT_VERSION).contains(&version)
        {
            let rows: Vec<(String, String, String)> = db
                .rows("select id, constraints_text, out_of_scope from missions order by id")?
                .iter()
                .map(|row| {
                    (
                        text(row, "id").unwrap_or_default(),
                        text(row, "constraints_text").unwrap_or_default(),
                        text(row, "out_of_scope").unwrap_or_default(),
                    )
                })
                .collect();
            dolt::move_lists_sql(&rows)
        } else {
            String::new()
        };
        Ok(dolt::upgrade_sql_moving(version, &moved))
    }

    /// Every mission in `db`. A version 2 ledger has no mission tables: it has no
    /// missions. A version 1 ledger is refused.
    fn missions_in(&self, db: &Dolt, location: &LedgerLocation) -> Result<Vec<Mission>, String> {
        if let Some(missions) = self.read_snapshot(db, |snapshot| snapshot.missions.clone())? {
            return Ok(missions);
        }
        if let Some(missions) = self.missions_in_one_call(db, location)? {
            return Ok(missions);
        }
        let version = self.check_current(db, location)?;
        if version < dolt::MISSIONS_LAYOUT_VERSION {
            return Ok(Vec::new());
        }
        read_missions(db, version)
    }

    /// The common read, in one `dolt sql` process: the manifest and every query of the
    /// current layout. `None` when the ledger is not at the current layout or the batch
    /// fails, and the caller then reads it query by query, which gives the same result
    /// or the same error.
    fn missions_in_one_call(
        &self,
        db: &Dolt,
        location: &LedgerLocation,
    ) -> Result<Option<Vec<Mission>>, String> {
        let mut queries = vec![MANIFEST_QUERY.to_string()];
        let mut recorded = Vec::new();
        read_missions_with(dolt::LAYOUT_VERSION, &mut |sql| {
            recorded.push(sql.to_string());
            Ok(Vec::new())
        })?;
        queries.extend(recorded);
        let Ok(mut results) = db.rows_each(&queries) else {
            return Ok(None);
        };
        let info = manifest_from_rows(&results.remove(0))?;
        if info.version != dolt::LAYOUT_VERSION {
            return Ok(None);
        }
        manifest::check_values(
            info.version,
            info.repo_id.as_deref(),
            location.repo_id(),
            MANIFEST_SOURCE,
            DOLT_SUPPORTED_VERSIONS,
        )?;
        self.remember_checked(db, info.version);
        let mut results = results.into_iter();
        let missions = read_missions_with(dolt::LAYOUT_VERSION, &mut |_| {
            results
                .next()
                .ok_or_else(|| "error: a batched read ran out of results".to_string())
        })?;
        Ok(Some(missions))
    }

    /// The URL the store's `origin` remote should have: the directory of the file remote
    /// for a store that uses one, else the git URL of the ledger ref.
    fn expected_url(&self, path: &Path, location: &LedgerLocation) -> Result<String, String> {
        if is_database(path) {
            if let Some(url) = transport::store_url(path) {
                if Transport::of_url(&url) == Transport::FileRemote {
                    return Ok(transport::file_url(&transport::remote_dir_beside(path)));
                }
            }
        }
        remote_url(&self.repo, location)
    }

    /// Make the database of a ledger origin does not hold yet. A file remote starts as a
    /// directory holding the marker and the ignore file, a linked worktree of this
    /// repository; the first push fills it and publishes it.
    fn create_new_database(&self, path: &Path, location: &LedgerLocation) -> Result<(), String> {
        match transport::for_new_ledger()? {
            Transport::GitRemote => {
                let url = remote_url(&self.repo, location)?;
                create_database(path, &url, Some(location.ref_name()), location.repo_id())
            }
            Transport::FileRemote => {
                let dir = transport::remote_dir_beside(path);
                let initial = transport::initial_commit(&self.repo.git)?;
                transport::sync_checkout(&self.repo, &dir, &initial)?;
                create_database(path, &transport::file_url(&dir), None, location.repo_id())?;
                if let Some(parent) = path.parent() {
                    transport::write_ref_file(parent, location.ref_name())?;
                }
                Ok(())
            }
        }
    }

    /// A fresh clone meets the ledger ref: which transport it holds, and its commit. A ref
    /// that holds a file ledger stops with the way out.
    fn remote_kind(&self, location: &LedgerLocation) -> Result<(Transport, String), String> {
        let Some(commit) = self.ledger_tip_fetched(location)? else {
            return Err(format!(
                "error: {} on {} has no commit",
                location.ref_name(),
                location.label()
            ));
        };
        match ref_kind(&self.repo.git, &commit)? {
            RefKind::Files => Err(format!(
                "error: {} on {} holds a file ledger, not a Dolt ledger.\n       \
                 The files ledger backend was removed in 0.16.0. Use tsk 0.15.1 to convert it with tsk ledger migrate-to-dolt",
                location.ref_name(),
                location.label()
            )),
            RefKind::Dolt => Ok((transport::of_commit(&self.repo.git, &commit)?, commit)),
            RefKind::Unknown => Err(format!(
                "error: {} on {} has no Dolt manifest and no .tsk-ledger.toml; it is not a tsk ledger",
                location.ref_name(),
                location.label()
            )),
        }
    }
}

/// The bindings in the rows of [`BINDINGS_QUERY`], or of the older table when `legacy` is
/// set: every row of that one is a cloud row.
fn binding_rows(rows: &[serde_json::Value], legacy: bool) -> Vec<BindingRow> {
    rows.iter()
        .filter_map(|row| {
            let kind = if legacy {
                BindingKind::Cloud
            } else {
                BindingKind::parse(&text(row, "kind")?)?
            };
            Some(BindingRow {
                actor: text(row, "actor")?,
                kind,
                thread_id: text(row, "thread_id").unwrap_or_default(),
                registered_at: text(row, "registered_at").unwrap_or_default(),
            })
        })
        .collect()
}

/// A nullable text column: NULL for an empty value.
fn nullable(value: &str) -> String {
    if value.is_empty() {
        "NULL".to_string()
    } else {
        lit(value)
    }
}

/// The statements that make a list table hold `items` for the mission `id` (already a
/// SQL literal), with the rows it no longer has removed.
fn items_sql(id: &str, table: &str, items: &[Item]) -> String {
    let seqs: Vec<String> = items.iter().map(|i| i.seq.to_string()).collect();
    let mut sql = format!(
        "delete from {} where mission_id = {}{};\n",
        table,
        id,
        if seqs.is_empty() {
            String::new()
        } else {
            format!(" and seq not in ({})", seqs.join(", "))
        }
    );
    if !items.is_empty() {
        let rows: Vec<String> = items
            .iter()
            .map(|i| format!("({}, {}, {})", id, i.seq, lit(&i.text)))
            .collect();
        sql.push_str(&format!(
            "replace into {} (mission_id, seq, text) values {};\n",
            table,
            rows.join(", ")
        ));
    }
    sql
}

/// The statements that make the tables hold `mission`: its row, and its outcomes, blockers
/// and links, with the rows it no longer has removed. A value that did not change writes
/// the same cell, so Dolt merges two clones that changed different fields.
fn save_sql(mission: &Mission) -> String {
    let id = lit(&mission.id);
    let mut sql = format!(
        "insert into missions (id, name, category, status, parent, purpose, decision_authority, \
         next_objective, next_task, next_question, next_constraint, next_out_of_scope) values \
         ({id}, {name}, '{category}', '{status}', {parent}, {purpose}, {authority}, \
         {next}, {next_task}, {next_question}, {next_constraint}, {next_out_of_scope}) \
         on duplicate key update name = values(name), category = values(category), \
         status = values(status), parent = values(parent), purpose = values(purpose), \
         decision_authority = values(decision_authority), \
         next_objective = values(next_objective), next_task = values(next_task), \
         next_question = values(next_question), next_constraint = values(next_constraint), \
         next_out_of_scope = values(next_out_of_scope);\n",
        id = id,
        name = lit(&mission.name),
        category = mission.category.as_str(),
        status = mission.status.as_str(),
        parent = mission
            .parent
            .as_deref()
            .map(lit)
            .unwrap_or_else(|| "NULL".to_string()),
        purpose = nullable(&mission.purpose),
        authority = nullable(&mission.decision_authority),
        next = mission.next_objective,
        next_task = mission.next_task,
        next_question = mission.next_question,
        next_constraint = mission.next_constraint,
        next_out_of_scope = mission.next_out_of_scope,
    );
    sql.push_str(&items_sql(&id, "mission_constraints", &mission.constraints));
    sql.push_str(&items_sql(
        &id,
        "mission_out_of_scope",
        &mission.out_of_scope,
    ));

    let seqs: Vec<String> = mission
        .objective
        .iter()
        .map(|o| o.seq.to_string())
        .collect();
    sql.push_str(&format!(
        "delete from objective_outcomes where mission_id = {}{};\n",
        id,
        if seqs.is_empty() {
            String::new()
        } else {
            format!(" and seq not in ({})", seqs.join(", "))
        }
    ));
    if !mission.objective.is_empty() {
        let rows: Vec<String> = mission
            .objective
            .iter()
            .map(|o| {
                format!(
                    "({}, {}, {}, {})",
                    id,
                    o.seq,
                    lit(&o.outcome),
                    if o.met { "true" } else { "false" }
                )
            })
            .collect();
        sql.push_str(&format!(
            "replace into objective_outcomes (mission_id, seq, outcome, met) values {};\n",
            rows.join(", ")
        ));
    }

    let kept: Vec<String> = mission.blocked_by.iter().map(|b| lit(b)).collect();
    sql.push_str(&format!(
        "delete from mission_blocks where mission_id = {}{};\n",
        id,
        if kept.is_empty() {
            String::new()
        } else {
            format!(" and blocked_by_id not in ({})", kept.join(", "))
        }
    ));
    if !kept.is_empty() {
        let rows: Vec<String> = kept.iter().map(|b| format!("({}, {})", id, b)).collect();
        sql.push_str(&format!(
            "replace into mission_blocks (mission_id, blocked_by_id) values {};\n",
            rows.join(", ")
        ));
    }

    let paths: Vec<String> = mission.documents.iter().map(|d| lit(&d.path)).collect();
    sql.push_str(&format!(
        "delete from mission_documents where mission_id = {}{};\n",
        id,
        if paths.is_empty() {
            String::new()
        } else {
            format!(" and path not in ({})", paths.join(", "))
        }
    ));
    if !paths.is_empty() {
        let rows: Vec<String> = mission
            .documents
            .iter()
            .map(|d| format!("({}, {}, '{}')", id, lit(&d.path), d.role.as_str()))
            .collect();
        sql.push_str(&format!(
            "replace into mission_documents (mission_id, path, role) values {};\n",
            rows.join(", ")
        ));
    }
    // Tasks first: blocks and decisions point at them. A task is never removed.
    if !mission.tasks.is_empty() {
        let rows: Vec<String> = mission
            .tasks
            .iter()
            .map(|t| {
                format!(
                    "({}, {}, {}, '{}')",
                    id,
                    t.seq,
                    lit(&t.description),
                    t.status.as_str()
                )
            })
            .collect();
        sql.push_str(&format!(
            "insert into tasks (mission_id, seq, description, status) values {} \
             on duplicate key update description = values(description), status = values(status);\n",
            rows.join(", ")
        ));
    }
    sql.push_str(&format!(
        "delete from task_blocks where mission_id = {};\n",
        id
    ));
    let blocks: Vec<String> = mission
        .tasks
        .iter()
        .flat_map(|t| {
            let id = &id;
            t.blocked_by
                .iter()
                .map(move |by| format!("({}, {}, {})", id, t.seq, by))
        })
        .collect();
    if !blocks.is_empty() {
        sql.push_str(&format!(
            "replace into task_blocks (mission_id, task_seq, blocked_by_seq) values {};\n",
            blocks.join(", ")
        ));
    }
    if !mission.decisions.is_empty() {
        let rows: Vec<String> = mission
            .decisions
            .iter()
            .map(|d| {
                format!(
                    "({}, {}, {}, {}, {}, {}, {})",
                    id,
                    d.seq,
                    lit(&d.text),
                    lit(&d.decided_by),
                    lit(&d.decided_on),
                    d.task.map_or("NULL".to_string(), |t| t.to_string()),
                    d.adr.as_deref().map_or("NULL".to_string(), lit)
                )
            })
            .collect();
        sql.push_str(&format!(
            "insert into decisions (mission_id, seq, text, decided_by, decided_on, task_seq, adr_path) values {} \
             on duplicate key update text = values(text), decided_by = values(decided_by), \
             decided_on = values(decided_on), task_seq = values(task_seq), \
             adr_path = values(adr_path);\n",
            rows.join(", ")
        ));
    }
    if !mission.questions.is_empty() {
        let opt = |v: &Option<String>| v.as_deref().map_or("NULL".to_string(), lit);
        let rows: Vec<String> = mission
            .questions
            .iter()
            .map(|q| {
                format!(
                    "({}, {}, {}, '{}', {}, {}, {}, {})",
                    id,
                    q.seq,
                    lit(&q.text),
                    q.status.as_str(),
                    opt(&q.answer),
                    opt(&q.answered_by),
                    opt(&q.answered_on),
                    lit(&q.asked_on)
                )
            })
            .collect();
        sql.push_str(&format!(
            "insert into questions (mission_id, seq, text, status, answer, answered_by, answered_on, asked_on) values {} \
             on duplicate key update text = values(text), status = values(status), \
             answer = values(answer), answered_by = values(answered_by), \
             answered_on = values(answered_on), asked_on = values(asked_on);\n",
            rows.join(", ")
        ));
    }
    sql
}

/// Read every mission with its outcomes, blockers and links, and with its tasks,
/// decisions and questions when the layout `version` has the tables. A version 3 ledger
/// has no task, decision or question tables.
fn read_missions(db: &Dolt, version: i64) -> Result<Vec<Mission>, String> {
    read_missions_with(version, &mut |sql| db.rows(sql))
}

/// Read the missions with every query going through `rows`, so a caller can run the
/// queries one by one or replay the results of a batch.
fn read_missions_with(
    version: i64,
    rows: &mut dyn FnMut(&str) -> Result<Vec<serde_json::Value>, String>,
) -> Result<Vec<Mission>, String> {
    let with_records = version >= dolt::TASKS_LAYOUT_VERSION;
    let bad = |column: &str, id: &str| {
        format!(
            "error: the missions table holds an unreadable {} for {}",
            column, id
        )
    };
    let mut missions = Vec::new();
    let mut columns = String::from("next_objective");
    if with_records {
        columns.push_str(", next_task, next_question");
    }
    // Below version 7 the lists are two text columns, read as items by the line rule.
    let with_lists = version >= dolt::LIST_LAYOUT_VERSION;
    columns.push_str(if with_lists {
        ", next_constraint, next_out_of_scope"
    } else {
        ", constraints_text, out_of_scope"
    });
    for row in rows(&format!(
        "select id, name, category, status, parent, purpose, decision_authority, {} \
         from missions order by id",
        columns
    ))? {
        let id = text(&row, "id").unwrap_or_default();
        missions.push(Mission {
            name: text(&row, "name").unwrap_or_default(),
            category: Category::parse(&text(&row, "category").unwrap_or_default())
                .map_err(|_| bad("category", &id))?,
            status: Status::parse(&text(&row, "status").unwrap_or_default())
                .map_err(|_| bad("status", &id))?,
            parent: text(&row, "parent"),
            purpose: text(&row, "purpose").unwrap_or_default(),
            decision_authority: text(&row, "decision_authority").unwrap_or_default(),
            constraints: model::lines_to_items(&text(&row, "constraints_text").unwrap_or_default()),
            out_of_scope: model::lines_to_items(&text(&row, "out_of_scope").unwrap_or_default()),
            next_constraint: integer(&row, "next_constraint").unwrap_or(1),
            next_out_of_scope: integer(&row, "next_out_of_scope").unwrap_or(1),
            next_objective: integer(&row, "next_objective").unwrap_or(1),
            objective: Vec::new(),
            blocked_by: Vec::new(),
            documents: Vec::new(),
            next_task: integer(&row, "next_task").unwrap_or(1),
            next_question: integer(&row, "next_question").unwrap_or(1),
            tasks: Vec::new(),
            decisions: Vec::new(),
            questions: Vec::new(),
            id,
        });
    }
    let slot = |missions: &mut Vec<Mission>, id: &str| -> Option<usize> {
        missions.iter().position(|m| m.id == id)
    };
    for row in rows(
        "select mission_id, seq, outcome, met from objective_outcomes order by mission_id, seq",
    )? {
        let id = text(&row, "mission_id").unwrap_or_default();
        if let Some(at) = slot(&mut missions, &id) {
            missions[at].objective.push(Outcome {
                seq: integer(&row, "seq").unwrap_or(0),
                outcome: text(&row, "outcome").unwrap_or_default(),
                met: flag(&row, "met"),
            });
        }
    }
    if with_lists {
        for (table, pick) in [("mission_constraints", 0), ("mission_out_of_scope", 1)] {
            for row in rows(&format!(
                "select mission_id, seq, text from {} order by mission_id, seq",
                table
            ))? {
                let id = text(&row, "mission_id").unwrap_or_default();
                if let Some(at) = slot(&mut missions, &id) {
                    let list = match pick {
                        0 => &mut missions[at].constraints,
                        _ => &mut missions[at].out_of_scope,
                    };
                    list.push(Item {
                        seq: integer(&row, "seq").unwrap_or(0),
                        text: text(&row, "text").unwrap_or_default(),
                    });
                }
            }
        }
    }
    for row in rows(
        "select mission_id, blocked_by_id from mission_blocks order by mission_id, blocked_by_id",
    )? {
        let id = text(&row, "mission_id").unwrap_or_default();
        if let Some(at) = slot(&mut missions, &id) {
            missions[at]
                .blocked_by
                .push(text(&row, "blocked_by_id").unwrap_or_default());
        }
    }
    for row in
        rows("select mission_id, path, role from mission_documents order by mission_id, path")?
    {
        let id = text(&row, "mission_id").unwrap_or_default();
        if let Some(at) = slot(&mut missions, &id) {
            missions[at].documents.push(Document {
                path: text(&row, "path").unwrap_or_default(),
                role: Role::parse(&text(&row, "role").unwrap_or_default())
                    .map_err(|_| bad("role", &id))?,
            });
        }
    }
    if with_records {
        read_records(
            rows,
            &mut missions,
            version >= dolt::ADR_LAYOUT_VERSION,
            &bad,
        )?;
    }
    for mission in &mut missions {
        mission.follow_list_counters();
    }
    Ok(missions)
}

/// Read every task with its blockers, and every decision and question.
fn read_records(
    rows: &mut dyn FnMut(&str) -> Result<Vec<serde_json::Value>, String>,
    missions: &mut [Mission],
    with_adr: bool,
    bad: &dyn Fn(&str, &str) -> String,
) -> Result<(), String> {
    let slot = |missions: &[Mission], id: &str| missions.iter().position(|m| m.id == id);
    for row in
        rows("select mission_id, seq, description, status from tasks order by mission_id, seq")?
    {
        let id = text(&row, "mission_id").unwrap_or_default();
        if let Some(at) = slot(missions, &id) {
            missions[at].tasks.push(Task {
                seq: integer(&row, "seq").unwrap_or(0),
                description: text(&row, "description").unwrap_or_default(),
                status: Status::parse(&text(&row, "status").unwrap_or_default())
                    .map_err(|_| bad("task status", &id))?,
                blocked_by: Vec::new(),
            });
        }
    }
    for row in rows(
        "select mission_id, task_seq, blocked_by_seq from task_blocks \
         order by mission_id, task_seq, blocked_by_seq",
    )? {
        let id = text(&row, "mission_id").unwrap_or_default();
        let task_seq = integer(&row, "task_seq").unwrap_or(0);
        if let Some(at) = slot(missions, &id) {
            if let Some(task) = missions[at].tasks.iter_mut().find(|t| t.seq == task_seq) {
                task.blocked_by
                    .push(integer(&row, "blocked_by_seq").unwrap_or(0));
            }
        }
    }
    // A ledger below version 6 has no `adr_path` column.
    let adr = if with_adr {
        "adr_path"
    } else {
        "null as adr_path"
    };
    for row in rows(&format!(
        "select mission_id, seq, text, decided_by, decided_on, task_seq, {} from decisions \
         order by mission_id, seq",
        adr
    ))? {
        let id = text(&row, "mission_id").unwrap_or_default();
        if let Some(at) = slot(missions, &id) {
            missions[at].decisions.push(Decision {
                seq: integer(&row, "seq").unwrap_or(0),
                text: text(&row, "text").unwrap_or_default(),
                decided_by: text(&row, "decided_by").unwrap_or_default(),
                decided_on: date(&row, "decided_on").unwrap_or_default(),
                task: integer(&row, "task_seq"),
                adr: text(&row, "adr_path").filter(|p| !p.is_empty()),
            });
        }
    }
    for row in rows(
        "select mission_id, seq, text, status, answer, answered_by, answered_on, asked_on \
         from questions order by mission_id, seq",
    )? {
        let id = text(&row, "mission_id").unwrap_or_default();
        if let Some(at) = slot(missions, &id) {
            missions[at].questions.push(Question {
                seq: integer(&row, "seq").unwrap_or(0),
                text: text(&row, "text").unwrap_or_default(),
                status: QuestionStatus::parse(&text(&row, "status").unwrap_or_default())
                    .ok_or_else(|| bad("question status", &id))?,
                answer: text(&row, "answer"),
                answered_by: text(&row, "answered_by"),
                answered_on: date(&row, "answered_on"),
                asked_on: date(&row, "asked_on").unwrap_or_default(),
            });
        }
    }
    Ok(())
}

/// A `date` column as `YYYY-MM-DD`. Dolt's JSON output may carry a time part.
fn date(row: &serde_json::Value, column: &str) -> Option<String> {
    text(row, column).map(|v| v.chars().take(10).collect())
}

/// A revision as the plain string literal `as of` needs, which takes no `CONVERT`. Only
/// a branch name or a commit hash reaches here; any other character is dropped.
pub(super) fn plain(revision: &str) -> String {
    let safe: String = revision
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '~' | '^'))
        .collect();
    format!("'{}'", safe)
}

/// The manifest version at `revision`, or in the working set when it is `None`. `None`
/// when the manifest cannot be read there.
fn manifest_version_of(db: &Dolt, revision: Option<&str>) -> Option<i64> {
    version_of_rows(&db.rows(&manifest_version_query(revision)).ok()?)
}

fn manifest_version_query(revision: Option<&str>) -> String {
    let as_of = revision.map_or(String::new(), |r| format!(" as of {}", plain(r)));
    format!("select v from manifest{} where k = 'version'", as_of)
}

fn version_of_rows(rows: &[serde_json::Value]) -> Option<i64> {
    rows.first()
        .and_then(|row| text(row, "v"))
        .and_then(|v| v.trim().parse().ok())
}

/// The rows of `table` that differ between `base` and the working set in a column the
/// table has on both sides. A schema change makes Dolt count every row as modified; this
/// counts only the rows whose own values changed.
fn edited_rows(db: &Dolt, base: &str, table: &str) -> Result<i64, String> {
    let now: Vec<String> = db
        .rows(&format!(
            "select column_name from information_schema.columns where table_schema = database() and table_name = {}",
            lit(table)
        ))?
        .iter()
        .filter_map(|row| text(row, "COLUMN_NAME").or_else(|| text(row, "column_name")))
        .collect();
    let before = db.rows(&format!(
        "show create table `{}` as of {}",
        table.replace('`', "``"),
        plain(base)
    ));
    // A table that did not exist at the base has no earlier columns: every row is new.
    let Ok(before) = before else {
        return Ok(0);
    };
    let create = before
        .first()
        .and_then(|row| text(row, "Create Table"))
        .unwrap_or_default();
    let common: Vec<&String> = now
        .iter()
        .filter(|column| create.contains(&format!("\n  `{}` ", column)))
        .collect();
    let diff = db.rows(&format!(
        "select * from dolt_diff({}, 'WORKING', {}) where diff_type = 'modified'",
        lit(base),
        lit(table)
    ))?;
    Ok(diff
        .iter()
        .filter(|row| {
            common
                .iter()
                .any(|c| row.get(format!("from_{}", c)) != row.get(format!("to_{}", c)))
        })
        .count() as i64)
}

/// Merge the remote branch into the local one, after the local rows that collide with
/// rows the remote added have taken new numbers (see [`renumber`]). When the merge fails,
/// the renumbering commit is dropped too, so the local branch stays as it was.
///
/// Returns the commit `HEAD` is at after a merge with no renumbering, which the check for
/// conflicts read in the same process; `None` when a renumbering followed the merge.
fn merge_renumbering(
    db: &Dolt,
    remote_ref: &str,
    renumbered: &mut Vec<Renumbered>,
) -> Result<Option<String>, String> {
    let kept = renumbered.len();
    let Some(prepared) = renumber::prepare(db, remote_ref, renumbered)? else {
        return merge_remote(db, remote_ref, None);
    };
    let merged = merge_remote(db, remote_ref, Some(&prepared.fixup_sql));
    if let Err(error) = merged {
        renumbered.truncate(kept);
        return Err(match renumber::undo(db, &prepared) {
            Ok(()) => error,
            Err(undo) => format!("{}\n{}", error, undo),
        });
    }
    Ok(None)
}

/// Merge the remote branch into the local one. A conflict aborts the merge and fails;
/// the local commit stays. `dolt pull` exits 0 on a conflict, so the tables are read.
/// `fixup` is SQL to run on the merged rows before the merge commit, which is then made
/// with that SQL in it.
///
/// The tables are read, and `HEAD`, in one process. A merge with no `fixup` returns the
/// commit `HEAD` is at.
fn merge_remote(
    db: &Dolt,
    remote_ref: &str,
    fixup: Option<&str>,
) -> Result<Option<String>, String> {
    let (args, output) = if fixup.is_some() {
        db.output(["merge", "--no-commit", remote_ref])?
    } else {
        db.output(["merge", remote_ref])?
    };
    let queries = [
        "select `table` as t from dolt_conflicts".to_string(),
        "select `table` as t from dolt_constraint_violations".to_string(),
        hash_query("HEAD"),
    ];
    let mut head = None;
    let sets = match db.rows_each(&queries) {
        Ok(mut sets) => {
            head = sets.remove(2).first().and_then(|row| text(row, "h"));
            sets
        }
        Err(_) => vec![db.rows(&queries[0])?, db.rows(&queries[1])?],
    };
    let mut tables = Vec::new();
    for rows in sets {
        for row in rows {
            if let Some(table) = text(&row, "t") {
                if !tables.contains(&table) {
                    tables.push(table);
                }
            }
        }
    }
    if !tables.is_empty() {
        let _ = db.output(["merge", "--abort"])?;
        return Err(format!(
            "error: merging {} into {} conflicted in {}; the merge is aborted.\n       \
             Both sides changed the same row. The local commit stays in {}.\n       \
             To drop the local changes instead, run tsk ledger status to see them, then tsk ledger discard,\n       \
             then re-run tsk ledger push",
            remote_ref,
            BRANCH,
            tables.join(", "),
            db.dir().display()
        ));
    }
    if !output.status.success() {
        return Err(dolt::failure_message(&args, &output));
    }
    if let Some(sql) = fixup {
        let finished = db.exec(sql).and_then(|_| {
            db.run(["add", "-A"])?;
            db.run([
                "commit",
                "-m",
                &format!("Merge {} into {}", remote_ref, BRANCH),
            ])
            .map(|_| ())
        });
        if let Err(error) = finished {
            let _ = db.output(["merge", "--abort"])?;
            return Err(error);
        }
        return Ok(None);
    }
    Ok(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_and_ssh_urls_gain_the_git_prefix() {
        let base = Path::new("/r");
        assert_eq!(
            dolt_url("https://github.com/o/r.git", base).unwrap(),
            "git+https://github.com/o/r.git"
        );
        assert_eq!(
            dolt_url("ssh://git@github.com/o/r", base).unwrap(),
            "git+ssh://git@github.com/o/r"
        );
    }

    #[test]
    fn scp_like_urls_become_ssh_urls() {
        assert_eq!(
            dolt_url("git@github.com:o/r.git", Path::new("/r")).unwrap(),
            "git+ssh://git@github.com/o/r.git"
        );
    }

    #[test]
    fn local_paths_and_file_urls_become_absolute_file_urls() {
        let dir = tempfile::tempdir().unwrap();
        let real = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir(real.join("origin.git")).unwrap();
        let abs = real.join("origin.git");
        assert_eq!(
            dolt_url(abs.to_str().unwrap(), Path::new("/elsewhere")).unwrap(),
            format!("git+file://{}", abs.display())
        );
        assert_eq!(
            dolt_url("origin.git", &real).unwrap(),
            format!("git+file://{}", abs.display())
        );
        assert_eq!(
            dolt_url(&format!("file://{}", abs.display()), Path::new("/x")).unwrap(),
            format!("git+file://{}", abs.display())
        );
    }

    #[test]
    fn unsupported_schemes_and_empty_urls_fail() {
        assert!(dolt_url("git://host/o/r", Path::new("/r")).is_err());
        assert!(dolt_url("  ", Path::new("/r")).is_err());
    }
}
