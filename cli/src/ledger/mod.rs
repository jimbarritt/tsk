pub mod docs;
pub mod dolt;
pub mod fetch;
pub mod git;
pub mod location;
pub mod manifest;
pub mod migrate;
pub mod migrate_docs;
pub mod nexus;
pub mod push;
pub mod register;
pub mod status;
pub mod store;

use clap::Subcommand;
use std::io::{Read, Write};
use std::path::PathBuf;

use store::{LedgerStore, PushReport, RefreshOutcome};

#[derive(Subcommand)]
pub enum LedgerCommands {
    #[command(
        about = "Fetch the ledger branch and materialise it at the fixed worktree path, refreshing in place; prints the path"
    )]
    Fetch,
    #[command(about = "Print the ledger worktree path without fetching or writing anything")]
    Path,
    #[command(
        about = "Commit every change in the ledger worktree, rebase onto the latest ledger branch, and push it with a bounded compare and swap retry; prints the commit the branch holds"
    )]
    Push {
        #[arg(help = "Commit message for the worktree changes")]
        message: String,
    },
    #[command(
        about = "Print what is not pushed, one fact per line: data and documents, clean or local changes with a line per changed table or file, and whether origin is ahead or behind; fetches the remote refs and changes nothing else"
    )]
    Status,
    #[command(
        about = "Drop every local change that is not pushed, in the data and the documents, and reset both to origin's state; without --yes it prints what it would drop and exits 1"
    )]
    Discard {
        #[arg(
            long,
            help = "Discard now. Without it nothing changes and the changes are printed"
        )]
        yes: bool,
    },
    #[command(
        name = "migrate-to-dolt",
        about = "Move a file ledger to Dolt in one Dolt commit on the same ledger ref; archives the file ledger's commit on refs/heads/archive/tsk-ledger-files first; prints migrated:<file-commit>:<dolt-head>"
    )]
    MigrateToDolt {
        #[arg(
            long,
            help = "Read the file ledger and build the Dolt database in a temporary directory, check it, print the counts and dry-run:ok, and push nothing"
        )]
        dry_run: bool,
    },
    #[command(
        name = "migrate-docs",
        about = "Move the documents of a version 1 Dolt ledger from the documents table to the tsk/docs branch; prints migrated-docs:<count>"
    )]
    MigrateDocs {
        #[arg(
            long,
            help = "Do everything except the drop and the pushes: write and check the files, print the counts and dry-run:ok"
        )]
        dry_run: bool,
    },
    #[command(
        about = "Read, write, list and remove ledger documents in the local store; none of them fetches or pushes"
    )]
    Doc {
        #[command(subcommand)]
        action: DocCommands,
    },
}

#[derive(Subcommand)]
pub enum DocCommands {
    #[command(
        about = "Print the absolute path of a document's file, for opening it in another tool; exit status 1 when it does not exist"
    )]
    Path {
        #[arg(help = "Document path relative to the ledger root")]
        path: String,
    },
    #[command(
        about = "Print a document exactly as stored, with no added newline; exit status 1 when it does not exist"
    )]
    Get {
        #[arg(help = "Document path relative to the ledger root, for example index.md")]
        path: String,
    },
    #[command(
        about = "Write a document in the local store from a file or stdin; does not push; prints nothing"
    )]
    Put {
        #[arg(help = "Document path relative to the ledger root")]
        path: String,
        #[arg(help = "File holding the body; reads stdin when omitted")]
        file: Option<PathBuf>,
    },
    #[command(about = "List document paths that start with a prefix, one per line, sorted")]
    List {
        #[arg(help = "Path prefix, for example missions/; lists every document when omitted")]
        prefix: Option<String>,
    },
    #[command(
        about = "Remove a document from the local store; exit status 1 when it does not exist"
    )]
    Rm {
        #[arg(help = "Document path relative to the ledger root")]
        path: String,
    },
}

pub fn run(action: LedgerCommands) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    match action {
        LedgerCommands::MigrateToDolt { dry_run } => return migrate::run(&cwd, dry_run),
        LedgerCommands::MigrateDocs { dry_run } => return migrate_docs::run(&cwd, dry_run),
        _ => {}
    }
    let store = store::open(&cwd)?;
    match action {
        LedgerCommands::Fetch => {
            let outcome = report_refresh(store.refresh_for_fetch()?);
            println!("{}", outcome.path.display());
            Ok(())
        }
        LedgerCommands::Path => {
            println!("{}", store.path()?.display());
            Ok(())
        }
        LedgerCommands::Push { message } => {
            let report = push_reporting(store.as_ref(), &message)?;
            println!("{}", report.head);
            Ok(())
        }
        LedgerCommands::Status => {
            let status = store.status()?;
            if let Some(note) = &status.fetch_note {
                eprintln!("{}", note);
            }
            for line in status.lines() {
                println!("{}", line);
            }
            Ok(())
        }
        LedgerCommands::Discard { yes } => run_discard(store.as_ref(), yes),
        LedgerCommands::Doc { action } => run_doc(store.as_ref(), action),
        LedgerCommands::MigrateToDolt { .. } | LedgerCommands::MigrateDocs { .. } => {
            unreachable!("handled before the store opens")
        }
    }
}

/// `tsk ledger discard`. Without `yes` it prints the local changes and refuses.
fn run_discard(store: &dyn LedgerStore, yes: bool) -> Result<(), String> {
    if !yes {
        let status = store.status()?;
        if let Some(note) = &status.fetch_note {
            eprintln!("{}", note);
        }
        for line in status.changes_lines() {
            println!("{}", line);
        }
        if status.is_clean() {
            return Ok(());
        }
        return Err(
            "error: this would discard the local changes above. Rerun with --yes to discard them."
                .to_string(),
        );
    }
    store.discard()?;
    println!("discarded");
    Ok(())
}

fn no_such_document(path: &str) -> String {
    format!("error: no such document: {}", path)
}

fn run_doc(store: &dyn LedgerStore, action: DocCommands) -> Result<(), String> {
    match action {
        DocCommands::Path { path } => {
            let file = store
                .doc_path(&path)?
                .ok_or_else(|| no_such_document(&path))?;
            let file = std::fs::canonicalize(&file).unwrap_or(file);
            println!("{}", file.display());
            Ok(())
        }
        DocCommands::Get { path } => {
            let body = store
                .doc_get(&path)?
                .ok_or_else(|| no_such_document(&path))?;
            let mut out = std::io::stdout();
            out.write_all(body.as_bytes())
                .and_then(|()| out.flush())
                .map_err(|e| format!("error: could not write to stdout: {}", e))
        }
        DocCommands::Put { path, file } => {
            let body = match file {
                Some(file) => std::fs::read_to_string(&file)
                    .map_err(|e| format!("error: could not read {}: {}", file.display(), e))?,
                None => {
                    let mut text = String::new();
                    std::io::stdin()
                        .read_to_string(&mut text)
                        .map_err(|e| format!("error: could not read stdin: {}", e))?;
                    text
                }
            };
            store.doc_put(&path, &body)
        }
        DocCommands::List { prefix } => {
            for path in store.doc_list(prefix.as_deref().unwrap_or(""))? {
                println!("{}", path);
            }
            Ok(())
        }
        DocCommands::Rm { path } => {
            if store.doc_remove(&path)? {
                Ok(())
            } else {
                Err(no_such_document(&path))
            }
        }
    }
}

pub fn fetch_reporting(store: &dyn LedgerStore) -> Result<RefreshOutcome, String> {
    Ok(report_refresh(store.refresh()?))
}

/// [`fetch_reporting`] for the thread commands: local changes are kept, not refused.
pub fn fetch_keeping_local_reporting(store: &dyn LedgerStore) -> Result<RefreshOutcome, String> {
    Ok(report_refresh(store.refresh_keeping_local()?))
}

/// Print the notes of a refresh to stderr and hand the outcome back.
pub(crate) fn report_refresh(outcome: RefreshOutcome) -> RefreshOutcome {
    if let Some(note) = &outcome.pending_note {
        eprintln!("{}", note);
    }
    if let Some(note) = &outcome.unpublished_note {
        eprintln!("{}", note);
    }
    outcome
}

pub fn push_reporting(store: &dyn LedgerStore, message: &str) -> Result<PushReport, String> {
    store.commit_and_push(message, &mut |note| eprintln!("{}", note))
}

pub(crate) fn retry_note(attempt: usize, status: &push::PushStatus) -> String {
    format!(
        "note: push rejected (attempt {}/{}): {}; fetching again and retrying",
        attempt,
        push::MAX_ATTEMPTS,
        status.summary
    )
}

pub(crate) fn push_notes(outcome: &push::PushOutcome) -> Vec<String> {
    let mut notes = Vec::new();
    if !outcome.committed {
        notes.push(format!(
            "note: nothing to commit in {}; checking whether an earlier run left a commit to push",
            outcome.path.display()
        ));
    }
    if outcome.result == push::PushResult::AlreadyOnRemote {
        notes.push(format!(
            "note: {}'s {} already holds this worktree's state; nothing to push",
            outcome.location.label(),
            outcome.location.ref_name()
        ));
    }
    notes
}

pub(crate) fn unpublished_note(outcome: &fetch::FetchOutcome) -> String {
    unpublished_text(
        outcome.location.label(),
        outcome.location.ref_name(),
        &outcome.path,
    )
}

pub(crate) fn unpublished_text(label: &str, ref_name: &str, path: &std::path::Path) -> String {
    format!(
        "note: {}'s {} does not exist yet; {} holds a new ledger.\n      The first tsk ledger push creates the branch",
        label,
        ref_name,
        path.display()
    )
}

pub(crate) fn pending_note(outcome: &fetch::FetchOutcome) -> String {
    let location = &outcome.location;
    let mut note = format!(
        "note: {} holds a commit that is not on {}'s {}:\n",
        outcome.path.display(),
        location.label(),
        location.ref_name()
    );
    for line in &outcome.pending {
        note.push_str(&format!("        {}\n", line));
    }
    note.push_str("      Leaving the worktree as it is rather than resetting over it.\n");
    note.push_str(
        "      Push it with tsk ledger push. To see what would be lost, run tsk ledger status.\n",
    );
    note.push_str("      To drop it deliberately, run tsk ledger discard.");
    note
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn pending_note_names_each_commit_and_the_reset_command() {
        let outcome = fetch::FetchOutcome {
            location: location::LedgerLocation::in_repo(),
            path: PathBuf::from("/s/tsk/repos/x/ledger"),
            commit: "abc123".to_string(),
            pending: vec!["1111111 first".to_string(), "2222222 second".to_string()],
            unpublished: false,
        };
        let note = pending_note(&outcome);
        assert!(note.starts_with("note: /s/tsk/repos/x/ledger holds a commit"));
        assert!(note.contains("refs/heads/tsk/ledger"));
        assert!(note.contains("        1111111 first\n"));
        assert!(note.contains("        2222222 second\n"));
        assert!(note.contains("run tsk ledger status"));
        assert!(note.ends_with("To drop it deliberately, run tsk ledger discard."));
        assert!(!note.contains("reset --hard"));
    }

    #[test]
    fn unpublished_note_names_the_ref_and_the_first_push() {
        let outcome = fetch::FetchOutcome {
            location: location::LedgerLocation::nexus("https://example.test/o/nexus", "work-api"),
            path: PathBuf::from("/s/tsk/repos/x/ledger"),
            commit: "abc123".to_string(),
            pending: Vec::new(),
            unpublished: true,
        };
        let note = unpublished_note(&outcome);
        assert!(note.contains("the nexus's refs/heads/ledgers/work-api does not exist yet"));
        assert!(note.contains("/s/tsk/repos/x/ledger holds a new ledger"));
        assert!(note.ends_with("The first tsk ledger push creates the branch"));
    }

    fn push_outcome(committed: bool, result: push::PushResult) -> push::PushOutcome {
        push::PushOutcome {
            location: location::LedgerLocation::in_repo(),
            path: PathBuf::from("/s/tsk/repos/x/ledger"),
            committed,
            commit: "abc123".to_string(),
            result,
        }
    }

    #[test]
    fn push_notes_are_empty_after_a_commit_and_push() {
        let outcome = push_outcome(true, push::PushResult::Pushed { attempts: 1 });
        assert!(push_notes(&outcome).is_empty());
    }

    #[test]
    fn push_notes_name_nothing_to_commit_and_already_on_remote() {
        let outcome = push_outcome(false, push::PushResult::AlreadyOnRemote);
        let notes = push_notes(&outcome);
        assert_eq!(notes.len(), 2);
        assert!(notes[0].contains("nothing to commit in /s/tsk/repos/x/ledger"));
        assert!(notes[1].contains("origin's refs/heads/tsk/ledger already holds"));
    }

    #[test]
    fn retry_note_names_the_attempt_and_the_summary() {
        let status = push::PushStatus {
            flag: '!',
            summary: "[rejected] (stale info)".to_string(),
        };
        assert_eq!(
            retry_note(2, &status),
            "note: push rejected (attempt 2/5): [rejected] (stale info); fetching again and retrying"
        );
    }
}
