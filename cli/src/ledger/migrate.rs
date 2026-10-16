//! `tsk ledger migrate-to-dolt`: move a file ledger to Dolt in one Dolt commit on the
//! same ledger ref.
//!
//! The records come from the fetched commit's tree, never from the ledger worktree, so
//! uncommitted edits there cannot leak in. The file ledger's commit is pushed to an
//! archive branch first, because Dolt's first push replaces the git branch with its own
//! history. A new database is built beside the target, read back through the Dolt
//! backend and compared with what the tree held, and only then published and moved into
//! place. The documents do not go into the database: they are written as files on the
//! `tsk/docs` branch, in a temporary worktree that is checked the same way and then
//! pushed first and moved into place.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::events::queue::{parse_watermark, QUEUE_FILE, WATERMARK_FILE};
use crate::ledger::docs;
use crate::ledger::dolt::{self, lit, Dolt, BRANCH, REMOTE};
use crate::ledger::fetch::{check_manifest, fetch_tip, ref_kind, RefKind};
use crate::ledger::git::{split_nul, Git};
use crate::ledger::location::{
    docs_worktree_path, dolt_db_path, mint_token, state_root_from_env, LedgerLocation, Repo,
};
use crate::ledger::manifest::{self, MANIFEST_FILE};
use crate::ledger::push::{commit_changes, parse_push_status};
use crate::ledger::store::dolt::{create_database_with, remote_tip, remote_url, DoltStore};
use crate::ledger::store::{check_document_path, LedgerStore};
use crate::thread::entry::STORE_FILE;
use crate::thread::lookup::LOOKUP_FILE;
use crate::thread::ops::{index_mission, mission_link};

const INDEX_FILE: &str = "index.md";
const MAX_DOCUMENT_PATH: usize = 1024;
const ROWS_PER_INSERT: usize = 50;

/// Everything a file ledger holds, in the shape the Dolt tables take.
#[derive(Debug, Default, PartialEq)]
struct Records {
    repo_id: Option<String>,
    /// Path and body, sorted by path.
    documents: Vec<(String, String)>,
    /// Thread ID, briefing path and continuation entries, sorted by ID.
    threads: Vec<ThreadRecord>,
    /// Session, thread ID and registration time.
    bindings: Vec<(String, String, String)>,
    events: Vec<String>,
    /// Processed-through count and update time, when the file ledger has a watermark.
    watermark: Option<(u64, String)>,
    /// Paths in the tree that are not records. They stay in the archive branch.
    skipped: Vec<String>,
    /// Observations to print as notes.
    warnings: Vec<String>,
}

#[derive(Debug, Default, PartialEq)]
struct ThreadRecord {
    id: String,
    briefing: String,
    /// The mission the thread names on its `Mission:` line, if it has one.
    mission_id: Option<String>,
    entries: Vec<String>,
}

impl Records {
    fn entry_count(&self) -> usize {
        self.threads.iter().map(|t| t.entries.len()).sum()
    }

    fn summary(&self) -> String {
        format!(
            "{} documents, {} threads, {} continuation entries, {} cloud bindings, {} events",
            self.documents.len(),
            self.threads.len(),
            self.entry_count(),
            self.bindings.len(),
            self.events.len()
        )
    }
}

/// Removes a half-built linked worktree unless it is kept.
pub(crate) struct TempWorktree {
    pub(crate) git: Git,
    pub(crate) path: PathBuf,
    pub(crate) keep: bool,
}

impl Drop for TempWorktree {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        let path = self.path.to_string_lossy().into_owned();
        let _ = self
            .git
            .output(["worktree", "remove", "--force", path.as_str()]);
        let _ = std::fs::remove_dir_all(&self.path);
        let _ = self.git.output(["worktree", "prune"]);
    }
}

/// Removes a half-built directory unless the build is kept.
struct TempDir {
    path: PathBuf,
    keep: bool,
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

pub fn run(cwd: &Path, dry_run: bool) -> Result<(), String> {
    Dolt::new(".").run(["version"])?;
    dolt::ensure_identity()?;

    let repo = Repo::discover(cwd)?;
    let state_root = state_root_from_env()?;
    let location = repo.location(&state_root)?;

    let commit = fetch_tip(&repo.git, &location)?.ok_or_else(|| {
        format!(
            "error: no ledger to migrate: {} does not exist on {}",
            location.ref_name(),
            location.label()
        )
    })?;
    match ref_kind(&repo.git, &commit)? {
        RefKind::Dolt => {
            return Err(format!(
                "error: the ledger is already a Dolt ledger: {} on {} holds Dolt data; nothing was written",
                location.ref_name(),
                location.label()
            ))
        }
        RefKind::Files | RefKind::Unknown => {
            check_manifest(&repo.git, &commit, &location)?;
        }
    }

    let records = read_records(&repo.git, &commit)?;
    eprintln!("note: read the file ledger at {}", commit);
    eprintln!("note: {}", records.summary());
    for warning in &records.warnings {
        eprintln!("note: {}", warning);
    }
    if !records.skipped.is_empty() {
        eprintln!(
            "note: {} files are not records and stay only in the archive: {}",
            records.skipped.len(),
            records.skipped.join(", ")
        );
    }

    let clone_id = repo.ensure_clone_id()?;
    let target = dolt_db_path(&state_root, &clone_id);
    let parent = target
        .parent()
        .ok_or_else(|| format!("error: {} has no parent directory", target.display()))?
        .to_path_buf();
    std::fs::create_dir_all(&parent)
        .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ledger-db".to_string());

    let docs_target = docs_worktree_path(&state_root, &clone_id);
    if docs_target.exists() {
        return Err(format!(
            "error: {} exists already; move it aside and re-run tsk ledger migrate-to-dolt",
            docs_target.display()
        ));
    }

    let url = remote_url(&repo, &location)?;
    let mut temp = TempDir {
        path: parent.join(format!("{}.migrate-{}", name, mint_token(8, &commit))),
        keep: false,
    };
    let message = format!("Migrate the file ledger at {} to Dolt", commit);
    create_database_with(
        &temp.path,
        &url,
        location.ref_name(),
        records.repo_id.as_deref(),
        &insert_sql(&records),
        &message,
    )?;

    let store = DoltStore::open(cwd)?.with_db(temp.path.clone());
    verify(&store, &records)?;
    eprintln!("note: the Dolt database reads back identical to the file ledger");

    let mut temp_docs = TempWorktree {
        git: repo.git.clone(),
        path: parent.join(format!("docs.migrate-{}", mint_token(8, &commit))),
        keep: false,
    };
    stage_documents(&repo, &location, &temp_docs.path, &records, &message)?;
    eprintln!(
        "note: the documents read back identical from the {} branch",
        location.docs_ref_name()
    );

    if dry_run {
        println!("dry-run:ok");
        return Ok(());
    }

    let archive = archive_ref(&location);
    archive_commit(&repo.git, &location, &archive, &commit)?;

    let now = remote_tip(&repo.git, &location)?;
    if now.as_deref() != Some(commit.as_str()) {
        return Err(format!(
            "error: {} on {} moved while the migration ran (it was {}, it is now {}); another writer pushed.\n       \
             Nothing on the ledger ref was changed. Re-run tsk ledger migrate-to-dolt",
            location.ref_name(),
            location.label(),
            commit,
            now.as_deref().unwrap_or("absent")
        ));
    }

    // The documents go first, so no record names a document the remote lacks.
    {
        let mut on_retry = |attempt: usize, status: &crate::ledger::push::PushStatus| {
            eprintln!("{}", crate::ledger::retry_note(attempt, status))
        };
        docs::push(&repo, &location, &temp_docs.path, &message, &mut on_retry).map_err(|e| {
            format!(
                "{}\n       The file ledger is safe on {} and {} is unchanged.\n       \
                 Re-run tsk ledger migrate-to-dolt",
                e,
                archive,
                location.ref_name()
            )
        })?;
    }

    let db = Dolt::new(&temp.path);
    let head = db.head_hash()?;
    db.run(["push", REMOTE, BRANCH]).map_err(|e| {
        format!(
            "{}\n       The file ledger is safe on {} and the temporary database is removed.\n       \
             Check {} on {}, then re-run tsk ledger migrate-to-dolt",
            e,
            archive,
            location.ref_name(),
            location.label()
        )
    })?;

    temp.keep = true;
    install(&temp.path, &target, &name, &parent)?;
    temp_docs.keep = true;
    let moved = temp_docs.path.to_string_lossy().into_owned();
    let docs_target_arg = docs_target.to_string_lossy().into_owned();
    repo.git
        .run(["worktree", "move", moved.as_str(), docs_target_arg.as_str()])
        .map_err(|e| {
            format!(
                "{}\n       The ledger is already migrated; move {} to {} yourself",
                e,
                temp_docs.path.display(),
                docs_target.display()
            )
        })?;
    eprintln!("note: the documents are at {}", docs_target.display());
    eprintln!("note: pushed the Dolt ledger to {}", location.ref_name());
    eprintln!(
        "note: archived the file ledger at {} on {}",
        commit, archive
    );
    eprintln!("note: the database is at {}", target.display());
    eprintln!("note: the old ledger worktree is left as it was");
    println!("migrated:{}:{}", commit, head);
    Ok(())
}

/// Where the file ledger's last commit is kept.
fn archive_ref(location: &LedgerLocation) -> String {
    match location.repo_id() {
        Some(id) => format!("refs/heads/archive/ledgers/{}-files", id),
        None => "refs/heads/archive/tsk-ledger-files".to_string(),
    }
}

/// Push `commit` to the archive ref with a create-only lease. An archive that already
/// holds the commit is left alone. One that holds an ancestor of it (an earlier run that
/// stopped before the ledger moved) is advanced, with a lease on that ancestor. One that
/// holds anything else is an error and is never overwritten.
fn archive_commit(
    git: &Git,
    location: &LedgerLocation,
    archive: &str,
    commit: &str,
) -> Result<(), String> {
    let (args, output) = git.output(["ls-remote", location.remote(), archive])?;
    if !output.status.success() {
        return Err(crate::ledger::git::failure_message(&args, &output));
    }
    let mut lease_from = String::new();
    let listing = String::from_utf8_lossy(&output.stdout).into_owned();
    let existing = listing
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(_, name)| *name == archive)
        .map(|(sha, _)| sha.to_string());
    match existing {
        Some(sha) if sha == commit => {
            eprintln!("note: {} already holds {}; not pushing it again", archive, commit);
            return Ok(());
        }
        Some(sha) if is_ancestor(git, &sha, commit) => {
            lease_from = sha;
        }
        Some(sha) => {
            return Err(format!(
                "error: {} on {} already holds {}, which is not an ancestor of the ledger commit {}.\n       \
                 It is not overwritten. Move or delete that branch deliberately, then re-run",
                archive,
                location.label(),
                sha,
                commit
            ))
        }
        None => {}
    }
    let lease = format!("--force-with-lease={}:{}", archive, lease_from);
    let refspec = format!("{}:{}", commit, archive);
    let (args, output) = git.output([
        "push",
        "--porcelain",
        lease.as_str(),
        location.remote(),
        refspec.as_str(),
    ])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let status = parse_push_status(&stdout, archive)
        .ok_or_else(|| crate::ledger::git::failure_message(&args, &output))?;
    if status.rejected() || !output.status.success() {
        return Err(format!(
            "error: could not create {} on {}: {}",
            archive,
            location.label(),
            status.summary
        ));
    }
    Ok(())
}

/// True when `ancestor` is in the local object database and an ancestor of `commit`.
fn is_ancestor(git: &Git, ancestor: &str, commit: &str) -> bool {
    git.output(["merge-base", "--is-ancestor", ancestor, commit])
        .map(|(_, output)| output.status.code() == Some(0))
        .unwrap_or(false)
}

/// Move the built database to the target, setting an existing directory aside first.
fn install(temp: &Path, target: &Path, name: &str, parent: &Path) -> Result<(), String> {
    if target.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let aside = parent.join(format!("{}.pre-migrate-{}", name, stamp));
        std::fs::rename(target, &aside).map_err(|e| {
            format!(
                "error: could not move {} aside to {}: {}.\n       \
                 The ledger is already migrated; move the directory yourself and rename {} to {}",
                target.display(),
                aside.display(),
                e,
                temp.display(),
                target.display()
            )
        })?;
        eprintln!(
            "note: moved the earlier {} to {}",
            target.display(),
            aside.display()
        );
    }
    std::fs::rename(temp, target).map_err(|e| {
        format!(
            "error: could not move {} to {}: {}.\n       \
             The ledger is already migrated; rename the directory yourself",
            temp.display(),
            target.display(),
            e
        )
    })
}

/// Write every document as a file in a new worktree of the documents branch, commit it,
/// and check what the commit holds against the records. The worktree starts from the
/// remote's documents branch when there is one, or from the orphan commit of a new one.
fn stage_documents(
    repo: &Repo,
    location: &LedgerLocation,
    path: &Path,
    records: &Records,
    message: &str,
) -> Result<(), String> {
    docs::fetch_at(repo, location, path)?;
    let wt = Git::new(path);
    for (doc, body) in &records.documents {
        let file = path.join(doc);
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
        }
        std::fs::write(&file, body)
            .map_err(|e| format!("error: could not write {}: {}", file.display(), e))?;
    }
    commit_changes(&wt, message)?;
    for (doc, body) in &records.documents {
        let spec = format!("HEAD:{}", doc);
        let (args, output) = wt.output(["cat-file", "blob", spec.as_str()])?;
        if !output.status.success() {
            return Err(crate::ledger::git::failure_message(&args, &output));
        }
        expect(
            &format!("document {} on the documents branch", doc),
            Some(body.clone()),
            String::from_utf8(output.stdout).ok(),
        )?;
    }
    Ok(())
}

fn utf8(bytes: Vec<u8>, path: &str) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|_| {
        format!(
            "error: {} in the file ledger is not valid UTF-8; the Dolt ledger stores text only",
            path
        )
    })
}

struct TreeEntry {
    sha: String,
    path: String,
}

fn list_tree(git: &Git, commit: &str) -> Result<Vec<TreeEntry>, String> {
    let listing = git.run(["ls-tree", "-r", "-z", commit])?;
    let mut entries = Vec::new();
    for record in split_nul(&listing) {
        let Some((meta, path)) = record.split_once('\t') else {
            continue;
        };
        let mut fields = meta.split(' ');
        let (_mode, kind, sha) = (fields.next(), fields.next(), fields.next());
        if kind != Some("blob") {
            continue;
        }
        entries.push(TreeEntry {
            sha: sha.unwrap_or_default().to_string(),
            path: path.to_string(),
        });
    }
    Ok(entries)
}

fn read_blob(git: &Git, sha: &str, path: &str) -> Result<Vec<u8>, String> {
    let (args, output) = git.output(["cat-file", "blob", sha])?;
    if !output.status.success() {
        return Err(format!(
            "{} (reading {})",
            crate::ledger::git::failure_message(&args, &output),
            path
        ));
    }
    Ok(output.stdout)
}

/// Lines as the file backend lists them: split on `\n`, an unterminated last line kept.
fn event_lines(text: &str) -> Vec<String> {
    text.split_inclusive('\n')
        .map(|line| line.strip_suffix('\n').unwrap_or(line).to_string())
        .collect()
}

fn read_records(git: &Git, commit: &str) -> Result<Records, String> {
    let mut records = Records::default();
    let mut threads: BTreeMap<String, (Option<String>, Option<String>)> = BTreeMap::new();
    let mut lookup_text = None;
    let mut queue_text = None;
    let mut watermark_text = None;
    let mut manifest_text = None;

    for entry in list_tree(git, commit)? {
        let path = entry.path.as_str();
        let text = |label: &str| -> Result<String, String> {
            utf8(read_blob(git, &entry.sha, path)?, label)
        };
        if path == MANIFEST_FILE {
            manifest_text = Some(text(path)?);
        } else if let Some(rest) = path.strip_prefix("threads/") {
            if rest == LOOKUP_FILE {
                lookup_text = Some(text(path)?);
                continue;
            }
            match rest.split_once('/') {
                Some((id, file)) if !id.starts_with('.') => {
                    let slot = threads.entry(id.to_string()).or_default();
                    if file == INDEX_FILE {
                        slot.0 = Some(text(path)?);
                    } else if file == STORE_FILE {
                        slot.1 = Some(text(path)?);
                    } else {
                        records.skipped.push(path.to_string());
                    }
                }
                _ => records.skipped.push(path.to_string()),
            }
        } else if let Some(rest) = path.strip_prefix("external-events/") {
            if rest == QUEUE_FILE {
                queue_text = Some(text(path)?);
            } else if rest == WATERMARK_FILE {
                watermark_text = Some(text(path)?);
            } else {
                records.skipped.push(path.to_string());
            }
        } else if check_document_path(path).is_err() {
            records.skipped.push(path.to_string());
        } else {
            if path.len() > MAX_DOCUMENT_PATH {
                return Err(format!(
                    "error: the document path {} is longer than {} bytes",
                    path, MAX_DOCUMENT_PATH
                ));
            }
            records.documents.push((path.to_string(), text(path)?));
        }
    }

    records.documents.sort();
    let manifest_text = manifest_text.ok_or_else(|| {
        format!(
            "error: the file ledger has no {}; it is not a tsk ledger",
            MANIFEST_FILE
        )
    })?;
    records.repo_id = manifest::read(&manifest_text)?.repo_id;

    for (id, (index, store)) in threads {
        let briefing = index.as_deref().map(mission_link).unwrap_or_default();
        let mission_id = index.as_deref().and_then(index_mission);
        if index.is_none() {
            records.warnings.push(format!(
                "thread {} has no {}; its briefing is empty",
                id, INDEX_FILE
            ));
        }
        let entries = store
            .map(|text| {
                text.lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        records.threads.push(ThreadRecord {
            id,
            briefing,
            mission_id,
            entries,
        });
    }

    if let Some(text) = lookup_text {
        let lookup = crate::thread::lookup::Lookup::parse(&text, Path::new(LOOKUP_FILE))?;
        for (session, binding) in lookup.entries() {
            if !binding.extra.is_empty() {
                records.warnings.push(format!(
                    "the binding for session {} carries fields other than thread_id and registered_at; they are not migrated",
                    session
                ));
            }
            if binding.thread_id.is_none() || binding.registered_at.is_none() {
                records.warnings.push(format!(
                    "the binding for session {} lacks thread_id or registered_at; the missing value is stored empty",
                    session
                ));
            }
            records.bindings.push((
                session.clone(),
                binding.thread_id.clone().unwrap_or_default(),
                binding.registered_at.clone().unwrap_or_default(),
            ));
        }
    }

    if let Some(text) = queue_text {
        records.events = event_lines(&text);
    }

    if let Some(text) = watermark_text {
        let through = parse_watermark(&text)?;
        let updated_at = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("updated_at")
                    .and_then(|u| u.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        records.watermark = Some((through, updated_at));
    }
    Ok(records)
}

/// The insert statements for every record, for the one transaction that also creates
/// the tables.
fn insert_sql(records: &Records) -> String {
    let mut sql = String::new();
    let mut batch = |table: &str, columns: &str, rows: Vec<String>| {
        for chunk in rows.chunks(ROWS_PER_INSERT) {
            sql.push_str(&format!(
                "insert into {} ({}) values {};\n",
                table,
                columns,
                chunk.join(", ")
            ));
        }
    };
    batch(
        "threads",
        "thread_id, briefing_path, mission_id",
        records
            .threads
            .iter()
            .map(|t| {
                let nullable = |v: &str| {
                    if v.is_empty() {
                        "NULL".to_string()
                    } else {
                        lit(v)
                    }
                };
                format!(
                    "({}, {}, {})",
                    lit(&t.id),
                    nullable(&t.briefing),
                    nullable(t.mission_id.as_deref().unwrap_or(""))
                )
            })
            .collect(),
    );
    batch(
        "continuation_entries",
        "thread_id, seq, entry",
        records
            .threads
            .iter()
            .flat_map(|t| {
                t.entries
                    .iter()
                    .enumerate()
                    .map(move |(i, entry)| format!("({}, {}, {})", lit(&t.id), i + 1, lit(entry)))
            })
            .collect(),
    );
    batch(
        "cloud_session_bindings",
        "session_id, thread_id, registered_at",
        records
            .bindings
            .iter()
            .map(|(s, t, r)| format!("({}, {}, {})", lit(s), lit(t), lit(r)))
            .collect(),
    );
    batch(
        "external_events",
        "position, envelope",
        records
            .events
            .iter()
            .enumerate()
            .map(|(i, line)| format!("({}, {})", i + 1, lit(line)))
            .collect(),
    );
    if let Some((through, updated_at)) = &records.watermark {
        sql.push_str(&format!(
            "insert into watermark (id, processed_through, updated_at) values (1, {}, {});\n",
            through,
            lit(updated_at)
        ));
    }
    sql
}

fn mismatch(what: &str, expected: &dyn std::fmt::Debug, found: &dyn std::fmt::Debug) -> String {
    format!(
        "error: the Dolt database differs from the file ledger in {}: expected {:?}, read {:?}.\n       Nothing was pushed",
        what, expected, found
    )
}

fn expect<T: PartialEq + std::fmt::Debug>(what: &str, expected: T, found: T) -> Result<(), String> {
    if expected == found {
        Ok(())
    } else {
        Err(mismatch(what, &expected, &found))
    }
}

/// Read the whole ledger back through the store trait and compare it with the records
/// parsed from the tree, using the rules the file backend applies when it reads. The
/// documents are not in the database: [`stage_documents`] checks them.
fn verify(store: &dyn LedgerStore, records: &Records) -> Result<(), String> {
    let info = store.manifest()?;
    expect(
        "the manifest version",
        crate::ledger::dolt::LAYOUT_VERSION,
        info.version,
    )?;
    expect(
        "the manifest repo_id",
        records.repo_id.clone(),
        info.repo_id,
    )?;

    let ids: Vec<String> = records.threads.iter().map(|t| t.id.clone()).collect();
    expect("the thread list", ids, store.thread_ids()?)?;
    for thread in &records.threads {
        let info = store.thread_info(&thread.id)?;
        expect(
            &format!("the briefing of thread {}", thread.id),
            thread.briefing.clone(),
            info.briefing_path.unwrap_or_default(),
        )?;
        expect(
            &format!("the mission of thread {}", thread.id),
            thread.mission_id.clone(),
            info.mission_id,
        )?;
        expect(
            &format!("the continuation entries of thread {}", thread.id),
            thread.entries.clone(),
            store.continuation_list(&thread.id)?,
        )?;
    }

    let mut expected = records.bindings.clone();
    expected.sort();
    let mut found: Vec<(String, String, String)> = store
        .cloud_lookup()?
        .entries()
        .iter()
        .map(|(session, binding)| {
            (
                session.clone(),
                binding.thread_id.clone().unwrap_or_default(),
                binding.registered_at.clone().unwrap_or_default(),
            )
        })
        .collect();
    found.sort();
    expect("the cloud session bindings", expected, found)?;

    let events = (!records.events.is_empty()).then(|| records.events.clone());
    expect("the external events", events, store.events_list()?)?;
    expect(
        "the external event count",
        records.events.len() as u64,
        store.events_count()?,
    )?;
    expect(
        "the watermark",
        records.watermark.as_ref().map(|w| w.0).unwrap_or(0),
        store.watermark_get()?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_lines_keep_an_unterminated_last_line() {
        assert_eq!(event_lines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(event_lines("a\nb"), vec!["a", "b"]);
        assert!(event_lines("").is_empty());
    }

    #[test]
    fn insert_sql_numbers_entries_and_events_from_one() {
        let records = Records {
            threads: vec![ThreadRecord {
                id: "t1".into(),
                briefing: "missions/M-X.md".into(),
                mission_id: Some("M-X".into()),
                entries: vec!["{\"a\":1}".into(), "{\"a\":2}".into()],
            }],
            events: vec!["e1".into()],
            watermark: Some((1, "2026-01-01T00:00:00Z".into())),
            ..Records::default()
        };
        let sql = insert_sql(&records);
        assert!(sql.contains("insert into continuation_entries"));
        assert!(sql.contains(", 1, CONVERT") && sql.contains(", 2, CONVERT"));
        assert!(sql.contains("insert into external_events (position, envelope) values (1, "));
        assert!(sql.contains("insert into watermark"));
        assert!(!sql.contains("documents"));
    }

    #[test]
    fn archive_ref_is_per_repo_for_a_nexus_ledger() {
        assert_eq!(
            archive_ref(&LedgerLocation::in_repo()),
            "refs/heads/archive/tsk-ledger-files"
        );
        assert_eq!(
            archive_ref(&LedgerLocation::nexus("https://x/y", "work-api")),
            "refs/heads/archive/ledgers/work-api-files"
        );
    }
}
