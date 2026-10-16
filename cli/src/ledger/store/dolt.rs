//! The Dolt backend: the ledger is a Dolt database in a directory beside the managed
//! repo's other state, published to the same git ref the file backend uses through
//! Dolt's `git+` remotes. Every operation runs the external `dolt` binary. Threads,
//! continuation entries, bindings and events are rows in the tables defined in
//! [`crate::ledger::dolt`]. Documents are not rows: they are Markdown files on a second
//! branch, `tsk/docs`, checked out as a worktree beside the database (see
//! [`crate::ledger::docs`]). A version 1 ledger still keeps them in a `documents` table
//! until `tsk ledger migrate-docs` moves them.

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
use crate::ledger::status::{self, LedgerStatus, RemoteState, SideStatus};
use crate::mission::model::{
    self, Category, Decision, Document, Mission, Outcome, Question, QuestionStatus, Role, Status,
    Task,
};
use crate::thread::lookup::{CloudBinding, Lookup};

use super::files::FileTree;
use super::{LedgerStore, PushReport, RefreshOutcome, ThreadInfo};

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
}

impl DoltStore {
    pub fn open(dir: &Path) -> Result<DoltStore, String> {
        let repo = Repo::discover(dir)?;
        let state_root = state_root_from_env()?;
        Ok(DoltStore {
            repo,
            state_root,
            db_override: None,
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
        let rows = db.rows("select k, v from manifest")?;
        let value = |key: &str| {
            rows.iter()
                .find(|row| text(row, "k").as_deref() == Some(key))
                .and_then(|row| text(row, "v"))
        };
        let version = value("version").ok_or_else(|| {
            "error: the ledger's manifest table has no version row; it is not a tsk ledger"
                .to_string()
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
        let version = self.check_manifest_of(db, location)?;
        if version == LEGACY_LAYOUT_VERSION {
            return Err(VERSION_1_REFUSAL.to_string());
        }
        Ok(version)
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
            if remote_branch_exists(db)? {
                let rows = db.rows(&format!(
                    "select dolt_merge_base('HEAD', {r}) = hashof('HEAD') as head_in_remote, \
                     dolt_merge_base('HEAD', {r}) = hashof({r}) as remote_in_head",
                    r = lit(&remote_ref)
                ));
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

fn verify_remote(db: &Dolt, url: &str, location: &LedgerLocation) -> Result<(), String> {
    let rows = db.rows(&format!(
        "select url, params from dolt_remotes where name = {}",
        lit(REMOTE)
    ))?;
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

fn pending_commits(db: &Dolt) -> Result<Vec<String>, String> {
    let rows = db.rows(&format!(
        "select commit_hash, message from dolt_log({})",
        lit(&format!("{}/{}..{}", REMOTE, BRANCH, BRANCH))
    ))?;
    Ok(rows
        .iter()
        .map(|row| {
            let hash = text(row, "commit_hash").unwrap_or_default();
            let message = text(row, "message").unwrap_or_default();
            format!(
                "{} {}",
                hash.chars().take(7).collect::<String>(),
                message.lines().next().unwrap_or("")
            )
        })
        .collect())
}

fn remote_branch_exists(db: &Dolt) -> Result<bool, String> {
    Ok(db.count(&format!(
        "select count(*) as c from dolt_remote_branches where name = {}",
        lit(REMOTE_BRANCH_NAME)
    ))? > 0)
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
        let has_remote = remote_branch_exists(db)?;
        let (base, remote) = if has_remote {
            let base = db
                .rows(&format!(
                    "select dolt_merge_base('HEAD', {}) as b",
                    lit(&remote_ref)
                ))
                .ok()
                .and_then(|rows| rows.first().and_then(|row| text(row, "b")))
                .unwrap_or_else(|| "HEAD".to_string());
            let count = |range: String| -> Result<usize, String> {
                Ok(db.count(&format!(
                    "select count(*) as c from dolt_log({})",
                    lit(&range)
                ))? as usize)
            };
            let ahead = count(format!("{}..{}", remote_ref, BRANCH))?;
            let behind = count(format!("{}..{}", BRANCH, remote_ref))?;
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
        let summary = db.rows(&format!(
            "select from_table_name, to_table_name, data_change, schema_change from dolt_diff_summary({}, 'WORKING')",
            lit(&base)
        ))?;
        let stats = db.rows(&format!(
            "select table_name, rows_added, rows_modified, rows_deleted from dolt_diff_stat({}, 'WORKING')",
            lit(&base)
        ))?;
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
            let counts = format!(
                "added {}\tmodified {}\tdeleted {}",
                number("rows_added"),
                number("rows_modified"),
                number("rows_deleted")
            );
            let schema = flag(row, "schema_change");
            let rows_changed = flag(row, "data_change") || stat.is_some();
            changes.push((
                name,
                match (rows_changed, schema) {
                    (false, true) => "schema changed".to_string(),
                    (true, true) => format!("{}\tschema changed", counts),
                    _ => counts,
                },
            ));
        }
        changes.sort();
        Ok(SideStatus {
            changes: changes
                .into_iter()
                .map(|(name, counts)| format!("{}\t{}", name, counts))
                .collect(),
            remote,
        })
    }

    /// Refresh the Dolt database, and return the manifest version it holds.
    pub(crate) fn refresh_data(&self, keep_local: bool) -> Result<(RefreshOutcome, i64), String> {
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
            verify_remote(&db, &url, &location)?;
            dirty = db.count("select count(*) as c from dolt_status")? > 0;
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
        if !remote_branch_exists(&db)? {
            return Err(format!(
                "error: {} on {} holds no Dolt branch '{}'; it is not a Dolt ledger",
                location.ref_name(),
                location.label(),
                BRANCH
            ));
        }
        let remote_head = db.hash_of(&format!("{}/{}", REMOTE, BRANCH))?;
        if present {
            pending = pending_commits(&db)?;
            if pending.is_empty() && !dirty {
                db.run(["reset", "--hard", &format!("{}/{}", REMOTE, BRANCH)])?;
            }
        }
        let version = self.check_manifest_of(&db, &location)?;
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
                    .map(|(letter, path)| format!("{}\t{}", letter, path))
                    .collect(),
                remote: side.remote,
            },
            fetch_note,
        })
    }

    fn discard(&self) -> Result<(), String> {
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

        db.run(["add", "-A"])?;
        let committed = db.count("select count(*) as c from dolt_status where staged = 1")? > 0;
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
        mission_id: &str,
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
            sql.push_str(&dolt::upgrade_sql(version));
        }
        sql.push_str(&format!(
            "insert into threads (thread_id, briefing_path, mission_id) values ({}, {}, {});",
            lit(id),
            briefing_path.map_or("NULL".to_string(), lit),
            lit(mission_id)
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
        let column = if self.manifest_rows(&db)?.version >= dolt::LAYOUT_VERSION {
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

    fn cloud_lookup(&self) -> Result<Lookup, String> {
        let rows = self.db()?.rows(
            "select session_id, thread_id, registered_at from cloud_session_bindings \
             order by registered_at, session_id",
        )?;
        let mut lookup = Lookup::default();
        for row in &rows {
            if let Some(session) = text(row, "session_id") {
                lookup.set(
                    &session,
                    CloudBinding::new(
                        &text(row, "thread_id").unwrap_or_default(),
                        &text(row, "registered_at").unwrap_or_default(),
                    ),
                );
            }
        }
        Ok(lookup)
    }

    fn cloud_bind(
        &self,
        session: &str,
        thread_id: &str,
        registered_at: &str,
    ) -> Result<(), String> {
        self.db()?.exec(&format!(
            "replace into cloud_session_bindings (session_id, thread_id, registered_at) \
             values ({}, {}, {});",
            lit(session),
            lit(thread_id),
            lit(registered_at)
        ))
    }

    fn cloud_unbind(&self, session: &str) -> Result<(), String> {
        self.db()?.exec(&format!(
            "delete from cloud_session_bindings where session_id = {};",
            lit(session)
        ))
    }

    fn cloud_unbind_thread(&self, thread_id: &str) -> Result<(), String> {
        self.db()?.exec(&format!(
            "delete from cloud_session_bindings where thread_id = {};",
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
            sql.push_str(&dolt::upgrade_sql(version));
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

    fn read_index_hint(&self, _path: &Path) -> String {
        "Read the index next with `tsk ledger doc get index.md`.".to_string()
    }
}

impl DoltStore {
    /// Every mission in `db`. A version 2 ledger has no mission tables: it has no
    /// missions. A version 1 ledger is refused.
    fn missions_in(&self, db: &Dolt, location: &LedgerLocation) -> Result<Vec<Mission>, String> {
        let version = self.check_current(db, location)?;
        if version < dolt::MISSIONS_LAYOUT_VERSION {
            return Ok(Vec::new());
        }
        // A version 3 ledger has no task, decision or question tables.
        read_missions(db, version >= dolt::TASKS_LAYOUT_VERSION)
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

/// The statements that make the tables hold `mission`: its row, and its outcomes, blockers
/// and links, with the rows it no longer has removed. A value that did not change writes
/// the same cell, so Dolt merges two clones that changed different fields.
fn save_sql(mission: &Mission) -> String {
    let id = lit(&mission.id);
    let mut sql = format!(
        "insert into missions (id, name, category, status, parent, purpose, decision_authority, \
         constraints_text, out_of_scope, next_objective, next_task, next_question) values \
         ({id}, {name}, '{category}', '{status}', {parent}, {purpose}, {authority}, \
         {constraints}, {scope}, {next}, {next_task}, {next_question}) \
         on duplicate key update name = values(name), category = values(category), \
         status = values(status), parent = values(parent), purpose = values(purpose), \
         decision_authority = values(decision_authority), \
         constraints_text = values(constraints_text), out_of_scope = values(out_of_scope), \
         next_objective = values(next_objective), next_task = values(next_task), \
         next_question = values(next_question);\n",
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
        constraints = nullable(&mission.constraints),
        scope = nullable(&mission.out_of_scope),
        next = mission.next_objective,
        next_task = mission.next_task,
        next_question = mission.next_question,
    );

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
                    "({}, {}, {}, {}, {}, {})",
                    id,
                    d.seq,
                    lit(&d.text),
                    lit(&d.decided_by),
                    lit(&d.decided_on),
                    d.task.map_or("NULL".to_string(), |t| t.to_string())
                )
            })
            .collect();
        sql.push_str(&format!(
            "insert into decisions (mission_id, seq, text, decided_by, decided_on, task_seq) values {} \
             on duplicate key update text = values(text), decided_by = values(decided_by), \
             decided_on = values(decided_on), task_seq = values(task_seq);\n",
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
/// decisions and questions when `with_records` says the tables exist.
fn read_missions(db: &Dolt, with_records: bool) -> Result<Vec<Mission>, String> {
    let bad = |column: &str, id: &str| {
        format!(
            "error: the missions table holds an unreadable {} for {}",
            column, id
        )
    };
    let mut missions = Vec::new();
    let counters = if with_records {
        ", next_task, next_question"
    } else {
        ""
    };
    for row in db.rows(&format!(
        "select id, name, category, status, parent, purpose, decision_authority, \
         constraints_text, out_of_scope, next_objective{} from missions order by id",
        counters
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
            constraints: text(&row, "constraints_text").unwrap_or_default(),
            out_of_scope: text(&row, "out_of_scope").unwrap_or_default(),
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
    for row in db.rows(
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
    for row in db.rows(
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
        db.rows("select mission_id, path, role from mission_documents order by mission_id, path")?
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
        read_records(db, &mut missions, &bad)?;
    }
    Ok(missions)
}

/// Read every task with its blockers, and every decision and question.
fn read_records(
    db: &Dolt,
    missions: &mut [Mission],
    bad: &dyn Fn(&str, &str) -> String,
) -> Result<(), String> {
    let slot = |missions: &[Mission], id: &str| missions.iter().position(|m| m.id == id);
    for row in
        db.rows("select mission_id, seq, description, status from tasks order by mission_id, seq")?
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
    for row in db.rows(
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
    for row in db.rows(
        "select mission_id, seq, text, decided_by, decided_on, task_seq from decisions \
         order by mission_id, seq",
    )? {
        let id = text(&row, "mission_id").unwrap_or_default();
        if let Some(at) = slot(missions, &id) {
            missions[at].decisions.push(Decision {
                seq: integer(&row, "seq").unwrap_or(0),
                text: text(&row, "text").unwrap_or_default(),
                decided_by: text(&row, "decided_by").unwrap_or_default(),
                decided_on: date(&row, "decided_on").unwrap_or_default(),
                task: integer(&row, "task_seq"),
            });
        }
    }
    for row in db.rows(
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
