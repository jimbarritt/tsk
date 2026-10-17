//! The Dolt backend: the ledger is a Dolt database in a directory beside the managed
//! repo's other state, published to the same git ref the file backend uses through
//! Dolt's `git+` remotes. Every operation runs the external `dolt` binary. Threads,
//! continuation entries, bindings and events are rows in the tables defined in
//! [`crate::ledger::dolt`]. Documents are not rows: they are Markdown files on a second
//! branch, `tsk/docs`, checked out as a worktree beside the database (see
//! [`crate::ledger::docs`]). A version 1 ledger still keeps them in a `documents` table
//! until `tsk ledger migrate-docs` moves them.

use std::cell::RefCell;
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
use crate::mission::model::{
    self, Category, Decision, Document, Item, Mission, Outcome, Question, QuestionStatus, Role,
    Status, Task,
};

use super::files::FileTree;
use super::{BindingKind, BindingRow, LedgerStore, PushReport, RefreshOutcome, ThreadInfo};

const MANIFEST_SOURCE: &str = "the manifest table";

/// What every command except `tsk ledger fetch` and `tsk ledger migrate-docs` says about
/// a version 1 ledger.
pub const VERSION_1_REFUSAL: &str = "error: this ledger stores its documents in Dolt (version 1). Run tsk ledger migrate-docs to move them to the tsk/docs branch.";

pub struct DoltStore {
    repo: Repo,
    state_root: PathBuf,
    /// Reads from this database instead of the one at the clone's usual path. The
    /// migration uses it to check a database it has built but not yet moved into place.
    db_override: Option<PathBuf>,
    /// The database and layout version of the last manifest check that found the current
    /// layout. Later checks in the same process reuse it, which saves a `dolt` process
    /// each. Anything that can move the database to another state clears it.
    checked: RefCell<Option<(PathBuf, i64)>>,
}

impl DoltStore {
    pub fn open(dir: &Path) -> Result<DoltStore, String> {
        let repo = Repo::discover(dir)?;
        let state_root = state_root_from_env()?;
        Ok(DoltStore {
            repo,
            state_root,
            db_override: None,
            checked: RefCell::new(None),
        })
    }

    /// The same store, reading and writing the database at `path`.
    pub fn with_db(mut self, path: PathBuf) -> DoltStore {
        self.db_override = Some(path);
        self
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
        if let Some(path) = &self.db_override {
            return Ok(Dolt::new(path));
        }
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
        Ok(Dolt::new(path))
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

    /// The worktree of the documents branch, when this clone has an id.
    fn docs_hint(&self) -> Option<PathBuf> {
        self.repo
            .read_clone_id()
            .ok()
            .flatten()
            .map(|id| docs_worktree_path(&self.state_root, &id))
    }

    /// The documents of an already materialised ledger.
    fn docs_tree(&self) -> Result<FileTree, String> {
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
        Ok(FileTree { root, docs: true })
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
        let fetched = docs::fetch(&self.repo, &self.state_root, &location, keep_local)?;
        let dolt_unpublished = outcome.unpublished;
        for line in &fetched.pending {
            outcome.pending.push(format!("documents: {}", line));
        }
        let docs_outcome = FetchOutcome {
            location: fetched.location,
            path: fetched.path,
            commit: fetched.commit,
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
        committed: bool,
        docs_changed: bool,
        notes: &mut dyn FnMut(&str),
    ) -> Result<PushReport, String> {
        self.forget_checked();
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
            }
        };

        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        for attempt in 1..=MAX_ATTEMPTS {
            db.run(["fetch", REMOTE])?;
            // The branch check, the merge base and the remote tip read in one process.
            // When the batch fails (no remote branch yet, say), they read one by one.
            let flags_query = format!(
                "select dolt_merge_base('HEAD', {r}) = hashof('HEAD') as head_in_remote, \
                 dolt_merge_base('HEAD', {r}) = hashof({r}) as remote_in_head",
                r = lit(&remote_ref)
            );
            let batched = db
                .rows_each(&[
                    remote_branch_query(),
                    flags_query.clone(),
                    hash_query(&remote_ref),
                ])
                .ok()
                .filter(|sets| dolt::first_count(&sets[0]) > 0);
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
                    merge_remote(db, &remote_ref)?;
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
                    merge_remote(db, &remote_ref)?;
                }
            }

            let (args, pushed) = db.output(["push", REMOTE, BRANCH])?;
            if pushed.status.success() {
                let head = db.head_hash()?;
                return Ok(finish(
                    outcome(PushResult::Pushed { attempts: attempt }, head),
                    notes,
                ));
            }
            let stderr = String::from_utf8_lossy(&pushed.stderr).to_lowercase();
            let stale = ["rejected", "behind", "non-fast-forward"]
                .iter()
                .any(|word| stderr.contains(word));
            if !stale {
                return Err(dolt::failure_message(&args, &pushed));
            }
            if attempt < MAX_ATTEMPTS {
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
    ref_name: &str,
    repo_id: Option<&str>,
) -> Result<(), String> {
    create_database_with(path, url, ref_name, repo_id, "", "Create the ledger")
}

/// [`create_database`] with `extra_sql` run in the same transaction as the schema, and
/// `message` as the commit message. The schema and the data land in one commit.
pub(crate) fn create_database_with(
    path: &Path,
    url: &str,
    ref_name: &str,
    repo_id: Option<&str>,
    extra_sql: &str,
    message: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("error: could not create {}: {}", path.display(), e))?;
    let build = || -> Result<(), String> {
        let db = Dolt::new(path);
        db.run(["init", "--initial-branch", BRANCH])?;
        let mut sql = dolt::init_sql(repo_id);
        sql.push_str(extra_sql);
        db.exec(&sql)?;
        db.run(["add", "-A"])?;
        db.run(["commit", "-m", message])?;
        db.run(["remote", "add", "--ref", ref_name, REMOTE, url])?;
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
    if found_url != url || found_ref != location.ref_name() {
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
    fn data_side(&self, db: &Dolt, online: bool) -> Result<SideStatus, String> {
        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        // Each step reads in one `dolt sql` process. When a batch fails, the step reads
        // query by query instead, which gives the same result or the same error.
        let (has_remote, local_version) =
            match db.rows_each(&[remote_branch_query(), manifest_version_query(None)]) {
                Ok(sets) => (dolt::first_count(&sets[0]) > 0, version_of_rows(&sets[1])),
                Err(_) => (remote_branch_exists(db)?, manifest_version_of(db, None)),
            };
        let (base, remote) = if has_remote {
            let queries = [
                format!("select dolt_merge_base('HEAD', {}) as b", lit(&remote_ref)),
                format!(
                    "select count(*) as c from dolt_log({})",
                    lit(&format!("{}..{}", remote_ref, BRANCH))
                ),
                format!(
                    "select count(*) as c from dolt_log({})",
                    lit(&format!("{}..{}", BRANCH, remote_ref))
                ),
            ];
            let (base, ahead, behind) = match db.rows_each(&queries) {
                Ok(sets) => (
                    sets[0]
                        .first()
                        .and_then(|row| text(row, "b"))
                        .unwrap_or_else(|| "HEAD".to_string()),
                    dolt::first_count(&sets[1]) as usize,
                    dolt::first_count(&sets[2]) as usize,
                ),
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
            };
            let state = if online {
                RemoteState::from_counts(ahead, behind)
            } else {
                RemoteState::Unknown
            };
            (base, state)
        } else if online {
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

    /// Refresh the Dolt database, and return the manifest version it holds.
    pub(crate) fn refresh_data(&self, keep_local: bool) -> Result<(RefreshOutcome, i64), String> {
        self.forget_checked();
        dolt::ensure_identity()?;
        let location = self.repo.location(&self.state_root)?;
        let clone_id = self.repo.ensure_clone_id()?;
        let path = dolt_db_path(&self.state_root, &clone_id);
        let url = remote_url(&self.repo, &location)?;
        let remote_label = format!("{}'s {}", location.label(), location.ref_name());
        let present = is_database(&path);
        if path.exists() && !present {
            return Err(format!(
                "error: {} exists but is not a Dolt database; move it aside and re-run",
                path.display()
            ));
        }

        if remote_tip(&self.repo.git, &location)?.is_none() {
            let version = if present {
                let db = Dolt::new(&path);
                verify_remote(&db, &url, &location)?;
                self.check_manifest_of(&db, &location)?
            } else {
                create_database(&path, &url, location.ref_name(), location.repo_id())?;
                dolt::LAYOUT_VERSION
            };
            let head = Dolt::new(&path).head_hash()?;
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
        let mut dirty = false;
        let mut pending = Vec::new();
        if present {
            db = Dolt::new(&path);
            // The remote check and the dirty check read in one process. When the batch
            // fails, they read one by one, which gives the same result or error.
            match db.rows_each(&[
                remote_query(),
                "select count(*) as c from dolt_status".to_string(),
            ]) {
                Ok(sets) => {
                    verify_remote_rows(&db, &sets[0], &url, &location)?;
                    dirty = dolt::first_count(&sets[1]) > 0;
                }
                Err(_) => {
                    verify_remote(&db, &url, &location)?;
                    dirty = db.count("select count(*) as c from dolt_status")? > 0;
                }
            }
            if dirty && !keep_local {
                return Err(dirty_error(&path));
            }
            db.run(["fetch", REMOTE])?;
        } else {
            self.check_remote_kind(&location)?;
            let parent = path
                .parent()
                .ok_or_else(|| format!("error: {} has no parent directory", path.display()))?;
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            Dolt::new(parent).run(["clone", "--ref", location.ref_name(), url.as_str(), &name])?;
            db = Dolt::new(&path);
        }
        // One process for the three reads. When the batch fails (the remote branch is
        // missing, say), they read one by one, which gives the same error.
        let remote_ref = format!("{}/{}", REMOTE, BRANCH);
        let mut queries = vec![remote_branch_query(), hash_query(&remote_ref)];
        if present {
            queries.push(pending_query());
        }
        let batched = db.rows_each(&queries).ok().filter(|sets| {
            dolt::first_count(&sets[0]) > 0 && sets[1].first().and_then(|r| text(r, "h")).is_some()
        });
        let remote_head = if let Some(sets) = batched {
            if present {
                pending = pending_from_rows(&sets[2]);
            }
            sets[1]
                .first()
                .and_then(|r| text(r, "h"))
                .unwrap_or_default()
        } else {
            if !remote_branch_exists(&db)? {
                return Err(format!(
                    "error: {} on {} holds no Dolt branch '{}'; it is not a Dolt ledger",
                    location.ref_name(),
                    location.label(),
                    BRANCH
                ));
            }
            let head = db.hash_of(&remote_ref)?;
            if present {
                pending = pending_commits(&db)?;
            }
            head
        };
        if present && pending.is_empty() && !dirty {
            db.run(["reset", "--hard", &format!("{}/{}", REMOTE, BRANCH)])?;
        }
        let version = self.check_manifest_of(&db, &location)?;
        self.remember_checked(&db, version);
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
        if self.db()?.count("select count(*) as c from dolt_status")? > 0 {
            return Ok(true);
        }
        let docs = Git::new(&self.docs_tree()?.root);
        Ok(!docs.run(["status", "--porcelain=v2", "-z"])?.is_empty())
    }

    fn status(&self) -> Result<LedgerStatus, String> {
        let db = self.db()?;
        let location = self.repo.location(&self.state_root)?;
        self.check_current(&db, &location)?;
        let docs = Git::new(&self.docs_tree()?.root);

        let data_fetch = db.run(["fetch", REMOTE]).map(|_| ());
        let docs_fetch = fetch_tip(&self.repo.git, &location.docs());
        let data = self.data_side(&db, data_fetch.is_ok())?;
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
            shared_branch: false,
            fetch_note,
        })
    }

    fn discard(&self) -> Result<(), String> {
        self.forget_checked();
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
        db.run(["fetch", REMOTE]).map_err(refused)?;
        let docs_tip = fetch_tip(&self.repo.git, &location.docs()).map_err(refused)?;

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
        let location = self.repo.location(&self.state_root)?;
        let db = self.db()?;
        self.check_current(&db, &location)?;
        let docs_path = self.docs_tree()?.root;

        // Stage everything and count the staged tables in one process; if that fails,
        // the same two steps run on their own and give the same error.
        let staged_query = "select count(*) as c from dolt_status where staged = 1";
        let staged =
            match db.rows_each(&["call dolt_add('-A')".to_string(), staged_query.to_string()]) {
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
                notes(&crate::ledger::retry_note(attempt, status))
            };
            docs::push(&self.repo, &location, &docs_path, message, &mut on_retry)?
        };
        let docs_changed =
            docs_outcome.committed || matches!(docs_outcome.result, PushResult::Pushed { .. });

        // Every document a mission links must still exist. The documents are already
        // pushed, so this stops before the Dolt data goes and keeps both local commits.
        let docs_tree = self.docs_tree()?;
        model::check_links(
            &self.missions_in(&db, &location)?,
            &|path| docs_tree.doc_path(path).is_file(),
            "The documents are pushed. The Dolt data stays committed locally and is not pushed yet",
        )?;

        let mut report = self.push_data(&location, &db, committed, docs_changed, notes)?;
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
        self.db()?.head_hash()
    }

    fn manifest(&self) -> Result<ManifestInfo, String> {
        self.manifest_rows(&self.db()?)
    }

    fn check_manifest(&self) -> Result<i64, String> {
        let location = self.repo.location(&self.state_root)?;
        self.check_current(&self.db()?, &location)
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
        Ok(self.db()?.count(&format!(
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
        db.exec(&sql)
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
        db.exec(&sql)
    }

    fn thread_delete(&self, id: &str) -> Result<(), String> {
        self.db()?.exec(&format!(
            "delete from continuation_entries where thread_id = {t};\n\
             delete from threads where thread_id = {t};",
            t = lit(id)
        ))
    }

    fn thread_ids(&self) -> Result<Vec<String>, String> {
        let rows = self.db()?.rows("select thread_id from threads")?;
        let mut ids: Vec<String> = rows
            .iter()
            .filter_map(|row| text(row, "thread_id"))
            .collect();
        ids.sort();
        Ok(ids)
    }

    fn thread_info(&self, id: &str) -> Result<ThreadInfo, String> {
        let db = self.db()?;
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
        let next = db.count(&format!(
            "select coalesce(max(seq), 0) + 1 as n from continuation_entries where thread_id = {}",
            lit(id)
        ))?;
        db.exec(&format!(
            "insert into continuation_entries (thread_id, seq, entry) values ({}, {}, {});",
            lit(id),
            next,
            lit(line)
        ))
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

    fn bindings_list(&self) -> Result<Vec<BindingRow>, String> {
        let db = self.db()?;
        // One query on a current ledger. A ledger below version 8 has no `bindings` table:
        // its cloud rows are read from the old table, and nothing is upgraded.
        let (rows, legacy) = match db.rows(
            "select actor, kind, thread_id, registered_at from bindings \
             order by registered_at, actor",
        ) {
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
        Ok(rows
            .iter()
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
            .collect())
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
        db.exec(&format!(
            "insert into external_events (position, envelope) values {};",
            values.join(", ")
        ))
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

    fn events_count(&self) -> Result<u64, String> {
        Ok(self
            .db()?
            .count("select count(*) as c from external_events")?
            .max(0) as u64)
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
        self.db()?.exec(&format!(
            "replace into watermark (id, processed_through, updated_at) values (1, {}, {});",
            value,
            lit(updated_at)
        ))
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
        db.exec(&sql)
    }

    fn describe_document(&self, path: &str) -> String {
        match self.docs_hint() {
            Some(root) => root.join(path).display().to_string(),
            None => format!("document {}", path),
        }
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
        db.exec(&script)
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

    /// A fresh clone meets a ref that holds a file ledger: stop with the way out.
    fn check_remote_kind(&self, location: &LedgerLocation) -> Result<(), String> {
        let Some(commit) = fetch_tip(&self.repo.git, location)? else {
            return Ok(());
        };
        match ref_kind(&self.repo.git, &commit)? {
            RefKind::Files => Err(format!(
                "error: {} on {} holds a file ledger, not a Dolt ledger.\n       \
                 Run tsk ledger migrate-to-dolt to convert it, or set TSK_LEDGER_BACKEND=git to keep using the file ledger",
                location.ref_name(),
                location.label()
            )),
            RefKind::Dolt => Ok(()),
            RefKind::Unknown => Err(format!(
                "error: {} on {} has no Dolt manifest and no .tsk-ledger.toml; it is not a tsk ledger",
                location.ref_name(),
                location.label()
            )),
        }
    }
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
fn plain(revision: &str) -> String {
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

/// Merge the remote branch into the local one. A conflict aborts the merge and fails;
/// the local commit stays. `dolt pull` exits 0 on a conflict, so the tables are read.
fn merge_remote(db: &Dolt, remote_ref: &str) -> Result<(), String> {
    let (args, output) = db.output(["merge", remote_ref])?;
    let mut tables = Vec::new();
    for query in [
        "select `table` as t from dolt_conflicts",
        "select `table` as t from dolt_constraint_violations",
    ] {
        for row in db.rows(query)? {
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
    Ok(())
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
