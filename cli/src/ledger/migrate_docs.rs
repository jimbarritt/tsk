//! `tsk ledger migrate-docs`: move the documents of a version 1 Dolt ledger out of the
//! `documents` table and onto the `tsk/docs` branch as plain files.
//!
//! The rows are read first and every path and body is checked, so nothing is written for a
//! ledger that cannot move. The files are committed to the documents worktree and read
//! back from the commit, and only then does the Dolt database drop the table, create the mission
//! tables and record version 3, in one Dolt commit. The documents are pushed before the Dolt data, so a
//! record never names a document the remote lacks. A rerun after a partial failure
//! compares what the branch already holds and goes on from there.

use std::collections::BTreeMap;
use std::path::Path;

use crate::ledger::docs;
use crate::ledger::dolt::{self, text, Dolt, BRANCH, REMOTE};
use crate::ledger::git::Git;
use crate::ledger::location::{dolt_db_path, state_root_from_env, LedgerLocation, Repo};
use crate::ledger::migrate::TempWorktree;
use crate::ledger::push::{commit_changes, PushStatus};
use crate::ledger::store::check_docs_path;
use crate::ledger::store::dolt::{remote_tip, DoltStore};
use crate::ledger::{report_refresh, retry_note};

const DOCS_COMMIT: &str = "Move the documents out of Dolt";
const DROP_COMMIT: &str = "Move the documents to the tsk/docs branch";

pub fn run(cwd: &Path, dry_run: bool) -> Result<(), String> {
    Dolt::new(".").run(["version"])?;
    dolt::ensure_identity()?;

    let repo = Repo::discover(cwd)?;
    let state_root = state_root_from_env()?;
    let location = repo.location(&state_root)?;
    let store = DoltStore::open(cwd)?;

    // Refresh the database without the version 2 refusal. The documents worktree is not
    // touched here: a version 1 ledger has none.
    let (outcome, version) = store.refresh_data(false)?;
    let outcome = report_refresh(outcome);
    if version != dolt::LEGACY_LAYOUT_VERSION {
        return Err("error: the documents are already on the tsk/docs branch".to_string());
    }
    let db = Dolt::new(dolt_db_path(&state_root, &repo.ensure_clone_id()?));
    let ledger_tip = remote_tip(&repo.git, &location)?;

    let documents = read_documents(&db)?;
    eprintln!(
        "note: read {} documents from the documents table in {}",
        documents.len(),
        outcome.path.display()
    );

    // A version 1 ledger has no documents worktree, and the commands that refuse it rely
    // on that. A worktree this run creates is therefore removed again unless the table
    // is dropped: on a dry run, and on any failure before the drop.
    let created_worktree = !docs::worktree_path(&repo, &state_root)?.exists();
    let fetched = docs::fetch(&repo, &state_root, &location, false)?;
    let mut guard = TempWorktree {
        git: repo.git.clone(),
        path: fetched.path.clone(),
        keep: !created_worktree,
    };
    if !fetched.pending.is_empty() {
        eprintln!(
            "note: {} holds {} commits that are not on {}'s {}; they are pushed with the documents",
            fetched.path.display(),
            fetched.pending.len(),
            location.label(),
            fetched.location.ref_name()
        );
    }
    let docs_path = fetched.path.clone();

    let written = write_files(&docs_path, &documents)?;
    let wt = Git::new(&docs_path);
    let committed = commit_changes(&wt, DOCS_COMMIT)?;
    verify(&wt, &documents)?;
    eprintln!(
        "note: {} files written, {} already on the branch{}",
        written,
        documents.len() - written,
        if committed { "; committed" } else { "" }
    );
    eprintln!(
        "note: the documents read back identical from the {} branch",
        location.docs_ref_name()
    );

    if dry_run {
        eprintln!("note: dry run: the table is not dropped and nothing is pushed");
        println!("dry-run:ok");
        return Ok(());
    }

    check_unmoved(&repo, &location, &ledger_tip, false)?;

    db.exec(&format!(
        "drop table documents;\n{}",
        dolt::upgrade_sql(dolt::DOCS_LAYOUT_VERSION)
    ))?;
    db.run(["add", "-A"])?;
    db.run(["commit", "-m", DROP_COMMIT])?;
    guard.keep = true;

    // The documents go first, so no record names a document the remote lacks.
    let mut on_retry =
        |attempt: usize, status: &PushStatus| eprintln!("{}", retry_note(attempt, status));
    docs::push(&repo, &location, &docs_path, DOCS_COMMIT, &mut on_retry).map_err(|e| {
        format!(
            "{}\n       The table is dropped in {} and the documents are committed in {}.\n       \
             Re-run tsk ledger push to publish both",
            e,
            db.dir().display(),
            docs_path.display()
        )
    })?;

    check_unmoved(&repo, &location, &ledger_tip, true)?;
    db.run(["push", REMOTE, BRANCH]).map_err(|e| {
        format!(
            "{}\n       The documents are on {}. The drop is committed in {}.\n       \
             Re-run tsk ledger push to publish it",
            e,
            location.docs_ref_name(),
            db.dir().display()
        )
    })?;
    eprintln!(
        "note: pushed {} and then {}",
        location.docs_ref_name(),
        location.ref_name()
    );
    println!("migrated-docs:{}", documents.len());
    Ok(())
}

/// Every row of the `documents` table, as normalised path and body, sorted by path. A
/// path that cannot be a file, two rows that name one file, a row that is a directory of
/// another, and a body or path that is not valid UTF-8 stop the migration with the path.
fn read_documents(db: &Dolt) -> Result<Vec<(String, String)>, String> {
    let rows = db.rows(
        "select path, body, length(path) as path_bytes, length(body) as body_bytes \
         from documents order by path",
    )?;
    let mut found: BTreeMap<String, String> = BTreeMap::new();
    for row in &rows {
        let raw = text(row, "path").unwrap_or_default();
        let body = text(row, "body").unwrap_or_default();
        let path_bytes = dolt::integer(row, "path_bytes").unwrap_or(-1);
        let body_bytes = dolt::integer(row, "body_bytes").unwrap_or(-1);
        if path_bytes != raw.len() as i64 {
            return Err(format!(
                "error: the document path {:?} in the documents table is not valid UTF-8; nothing was changed",
                raw
            ));
        }
        if body_bytes != body.len() as i64 {
            return Err(format!(
                "error: the document {} in the documents table is not valid UTF-8; nothing was changed",
                raw
            ));
        }
        let path = check_docs_path(&raw).map_err(|e| {
            format!(
                "error: the document {} in the documents table cannot be a file on the tsk/docs branch ({}); nothing was changed",
                raw,
                e.trim_start_matches("error: ")
            )
        })?;
        if found.insert(path.clone(), body).is_some() {
            return Err(format!(
                "error: two documents in the documents table name the file {}; nothing was changed",
                path
            ));
        }
    }
    let paths: Vec<&String> = found.keys().collect();
    for path in &paths {
        let prefix = format!("{}/", path);
        if let Some(inner) = paths.iter().find(|other| other.starts_with(&prefix)) {
            return Err(format!(
                "error: the document {} cannot be a file: the document {} needs it as a directory; nothing was changed",
                path, inner
            ));
        }
    }
    Ok(found.into_iter().collect())
}

/// Write each document as a file. A file that already holds the body is left alone. One
/// that holds a seed file of a new branch is replaced. One that holds anything else stops
/// the migration before any file is written. Returns the number of files written.
fn write_files(root: &Path, documents: &[(String, String)]) -> Result<usize, String> {
    let mut to_write = Vec::new();
    for (path, body) in documents {
        let file = root.join(path);
        if file.is_dir() {
            return Err(format!(
                "error: {} is a directory on the tsk/docs branch; the document {} cannot be written. Nothing was changed",
                file.display(),
                path
            ));
        }
        match std::fs::read(&file) {
            Ok(existing) if existing == body.as_bytes() => {}
            Ok(existing) => {
                let seed = String::from_utf8(existing)
                    .map(|text| docs::is_seed(path, &text))
                    .unwrap_or(false);
                if !seed {
                    return Err(format!(
                        "error: the tsk/docs branch already holds {} with different content from the documents table. Nothing was changed",
                        path
                    ));
                }
                to_write.push((file, body));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => to_write.push((file, body)),
            Err(e) => return Err(format!("error: could not read {}: {}", file.display(), e)),
        }
    }
    for (file, body) in &to_write {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
        }
        std::fs::write(file, body)
            .map_err(|e| format!("error: could not write {}: {}", file.display(), e))?;
    }
    Ok(to_write.len())
}

/// Read every file back from the commit, which is what a push sends, and compare it with
/// the rows. Any difference stops the migration before a push.
fn verify(wt: &Git, documents: &[(String, String)]) -> Result<(), String> {
    let status = wt.run(["status", "--porcelain=v2", "-z"])?;
    if !status.is_empty() {
        return Err(
            "error: the documents worktree holds uncommitted changes after the commit; nothing was pushed"
                .to_string(),
        );
    }
    for (path, body) in documents {
        let spec = format!("HEAD:{}", path);
        let (args, output) = wt.output(["cat-file", "blob", spec.as_str()])?;
        if !output.status.success() {
            return Err(crate::ledger::git::failure_message(&args, &output));
        }
        if output.stdout != body.as_bytes() {
            return Err(format!(
                "error: the document {} on the tsk/docs branch differs from the documents table; nothing was pushed",
                path
            ));
        }
    }
    Ok(())
}

/// Stop when the ledger ref moved since the migration read it: another writer pushed.
fn check_unmoved(
    repo: &Repo,
    location: &LedgerLocation,
    expected: &Option<String>,
    docs_pushed: bool,
) -> Result<(), String> {
    let now = remote_tip(&repo.git, location)?;
    if &now == expected {
        return Ok(());
    }
    Err(format!(
        "error: {} on {} moved while the migration ran (it was {}, it is now {}); another writer pushed.\n       {}",
        location.ref_name(),
        location.label(),
        expected.as_deref().unwrap_or("absent"),
        now.as_deref().unwrap_or("absent"),
        if docs_pushed {
            "The documents are on the tsk/docs branch and the drop is committed locally. Run tsk ledger push"
        } else {
            "Nothing was pushed and the table is not dropped. Re-run tsk ledger migrate-docs"
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_files_replaces_a_seed_and_refuses_other_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.md"), "# Ledger index\n").unwrap();
        std::fs::write(dir.path().join("keep.md"), "same").unwrap();
        let docs = vec![
            ("index.md".to_string(), "# Real\n".to_string()),
            ("keep.md".to_string(), "same".to_string()),
            ("new/n.md".to_string(), "".to_string()),
        ];
        assert_eq!(write_files(dir.path(), &docs).unwrap(), 2);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("index.md")).unwrap(),
            "# Real\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new/n.md")).unwrap(),
            ""
        );

        std::fs::write(dir.path().join("keep.md"), "edited").unwrap();
        let err = write_files(dir.path(), &docs).unwrap_err();
        assert!(err.contains("keep.md"), "{}", err);
        assert!(err.contains("Nothing was changed"), "{}", err);
    }
}
