//! The Dolt backend: the ledger is a Dolt database in a directory beside the managed
//! repo's other state, published to the same git ref the file backend uses through
//! Dolt's `git+` remotes. Every operation runs the external `dolt` binary. Documents,
//! threads, continuation entries, bindings and events are rows in the tables defined in
//! [`crate::ledger::dolt`].

use std::path::{Path, PathBuf};

use crate::ledger::dolt::{
    self, flag, integer, lit, text, Dolt, BRANCH, REMOTE, REMOTE_BRANCH_NAME,
};
use crate::ledger::fetch::{fetch_tip, ref_kind, RefKind};
use crate::ledger::git::Git;
use crate::ledger::location::{dolt_db_path, state_root_from_env, LedgerLocation, Repo};
use crate::ledger::manifest::{self, ManifestInfo};
use crate::ledger::push::{PushOutcome, PushResult, PushStatus, MAX_ATTEMPTS};
use crate::thread::lookup::{CloudBinding, Lookup};

use super::{LedgerStore, PushReport, RefreshOutcome};

const INITIAL_INDEX: &str = crate::ledger::fetch::INITIAL_INDEX;
const MANIFEST_SOURCE: &str = "the manifest table";

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

    fn check_manifest_of(&self, db: &Dolt, location: &LedgerLocation) -> Result<i64, String> {
        let info = self.manifest_rows(db)?;
        manifest::check_values(
            info.version,
            info.repo_id.as_deref(),
            location.repo_id(),
            MANIFEST_SOURCE,
        )
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

/// Create a new ledger database at `path`: the schema, the manifest rows and, when
/// `seed_index` is set, an `index.md` document holding the same text a new file ledger
/// starts with. The result is committed and has the remote configured, so the first
/// push creates the branch. The directory is removed again when any step fails.
pub fn create_database(
    path: &Path,
    url: &str,
    ref_name: &str,
    repo_id: Option<&str>,
    seed_index: bool,
) -> Result<(), String> {
    create_database_with(
        path,
        url,
        ref_name,
        repo_id,
        seed_index,
        "",
        "Create the ledger",
    )
}

/// [`create_database`] with `extra_sql` run in the same transaction as the schema, and
/// `message` as the commit message. The schema and the data land in one commit.
pub(crate) fn create_database_with(
    path: &Path,
    url: &str,
    ref_name: &str,
    repo_id: Option<&str>,
    seed_index: bool,
    extra_sql: &str,
    message: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("error: could not create {}: {}", path.display(), e))?;
    let build = || -> Result<(), String> {
        let db = Dolt::new(path);
        db.run(["init", "--initial-branch", BRANCH])?;
        let mut sql = dolt::init_sql(repo_id);
        if seed_index {
            sql.push_str(&format!(
                "insert into documents (path, body) values ({}, {});\n",
                lit("index.md"),
                lit(INITIAL_INDEX)
            ));
        }
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
         Push them with tsk ledger push, or discard them deliberately with: cd '{}' && dolt reset --hard\n       \
         To read the path without refreshing, use tsk ledger path",
        path.display(),
        path.display()
    )
}

impl LedgerStore for DoltStore {
    fn refresh(&self) -> Result<RefreshOutcome, String> {
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
            if present {
                let db = Dolt::new(&path);
                verify_remote(&db, &url, &location)?;
                self.check_manifest_of(&db, &location)?;
            } else {
                create_database(&path, &url, location.ref_name(), location.repo_id(), true)?;
            }
            let head = Dolt::new(&path).head_hash()?;
            return Ok(RefreshOutcome {
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
            });
        }

        let db;
        let mut pending = Vec::new();
        if present {
            db = Dolt::new(&path);
            verify_remote(&db, &url, &location)?;
            if db.count("select count(*) as c from dolt_status")? > 0 {
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
            if pending.is_empty() {
                db.run(["reset", "--hard", &format!("{}/{}", REMOTE, BRANCH)])?;
            }
        }
        self.check_manifest_of(&db, &location)?;
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
            note.push_str("      Push it with tsk ledger push, or discard it\n");
            note.push_str(&format!(
                "      deliberately with: cd '{}' && dolt reset --hard {}/{}",
                path.display(),
                REMOTE,
                BRANCH
            ));
            note
        });
        Ok(RefreshOutcome {
            path,
            remote_label,
            remote_head,
            pending,
            unpublished: false,
            pending_note,
            unpublished_note: None,
        })
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
        let path = db.dir().to_path_buf();
        self.check_manifest_of(&db, &location)?;

        db.run(["add", "-A"])?;
        let committed = db.count("select count(*) as c from dolt_status where staged = 1")? > 0;
        if committed {
            db.run(["commit", "-m", message])?;
        }

        let outcome = |result: PushResult, commit: String| PushOutcome {
            location: location.clone(),
            path: path.clone(),
            committed,
            commit,
            result,
        };
        let finish = |outcome: PushOutcome, notes: &mut dyn FnMut(&str)| {
            for note in crate::ledger::push_notes(&outcome) {
                notes(&note);
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
            if remote_branch_exists(&db)? {
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
                    merge_remote(&db, &remote_ref)?;
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

    fn head_id(&self) -> Result<String, String> {
        self.db()?.head_hash()
    }

    fn manifest(&self) -> Result<ManifestInfo, String> {
        self.manifest_rows(&self.db()?)
    }

    fn check_manifest(&self) -> Result<i64, String> {
        let location = self.repo.location(&self.state_root)?;
        self.check_manifest_of(&self.db()?, &location)
    }

    fn doc_get(&self, path: &str) -> Result<Option<String>, String> {
        super::check_document_path(path)?;
        let rows = self.db()?.rows(&format!(
            "select body from documents where path = {}",
            lit(path)
        ))?;
        Ok(rows.first().and_then(|row| text(row, "body")))
    }

    fn doc_put(&self, path: &str, body: &str) -> Result<(), String> {
        super::check_document_path(path)?;
        self.db()?.exec(&format!(
            "replace into documents (path, body) values ({}, {});",
            lit(path),
            lit(body)
        ))
    }

    fn doc_list(&self, prefix: &str) -> Result<Vec<String>, String> {
        let rows = self.db()?.rows("select path from documents")?;
        let mut paths: Vec<String> = rows
            .iter()
            .filter_map(|row| text(row, "path"))
            .filter(|path| path.starts_with(prefix))
            .collect();
        paths.sort();
        Ok(paths)
    }

    fn doc_remove(&self, path: &str) -> Result<bool, String> {
        super::check_document_path(path)?;
        let db = self.db()?;
        if !self.doc_exists(path)? {
            return Ok(false);
        }
        db.exec(&format!(
            "delete from documents where path = {};",
            lit(path)
        ))?;
        Ok(true)
    }

    fn doc_exists(&self, path: &str) -> Result<bool, String> {
        Ok(self.db()?.count(&format!(
            "select count(*) as c from documents where path = {}",
            lit(path)
        ))? > 0)
    }

    fn thread_exists(&self, id: &str) -> Result<bool, String> {
        Ok(self.db()?.count(&format!(
            "select count(*) as c from threads where thread_id = {}",
            lit(id)
        ))? > 0)
    }

    fn thread_create(&self, id: &str, briefing_path: &str) -> Result<(), String> {
        if self.thread_exists(id)? {
            return Err(format!("error: threads/{} already exists", id));
        }
        self.db()?.exec(&format!(
            "insert into threads (thread_id, briefing_path) values ({}, {});",
            lit(id),
            lit(briefing_path)
        ))
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

    fn thread_briefing(&self, id: &str) -> Result<Option<String>, String> {
        let rows = self.db()?.rows(&format!(
            "select briefing_path from threads where thread_id = {}",
            lit(id)
        ))?;
        Ok(rows.first().and_then(|row| text(row, "briefing_path")))
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

    fn describe_document(&self, path: &str) -> String {
        self.describe(&format!("document {}", path))
    }

    fn describe_thread(&self, id: &str) -> String {
        self.describe(&format!("thread {}", id))
    }

    fn describe_threads(&self) -> String {
        self.describe("the threads table")
    }

    fn pending_inspect_hint(&self, path: &Path, _remote_head: &str) -> String {
        format!(
            "cd '{}' && dolt log {r}/{b}..{b} && dolt diff {r}/{b} {b}",
            path.display(),
            r = REMOTE,
            b = BRANCH
        )
    }
}

impl DoltStore {
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
             Resolve it there with dolt, or discard it with dolt reset --hard {}, then re-run tsk ledger push",
            remote_ref,
            BRANCH,
            tables.join(", "),
            db.dir().display(),
            remote_ref
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
