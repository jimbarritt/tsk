pub mod fetch;
pub mod git;
pub mod location;
pub mod manifest;
pub mod nexus;
pub mod push;
pub mod register;

use clap::Subcommand;

use location::{state_root_from_env, worktree_path, Repo};

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
}

pub fn run(action: LedgerCommands) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = Repo::discover(&cwd)?;
    match action {
        LedgerCommands::Fetch => {
            let state_root = state_root_from_env()?;
            let outcome = fetch_reporting(&repo, &state_root)?;
            println!("{}", outcome.path.display());
            Ok(())
        }
        LedgerCommands::Path => {
            let state_root = state_root_from_env()?;
            let clone_id = repo.read_clone_id()?.ok_or_else(|| {
                format!(
                    "error: this clone has no clone id yet ({} is absent or empty).\n       Run tsk ledger fetch, which mints it.",
                    repo.clone_id_file().display()
                )
            })?;
            println!("{}", worktree_path(&state_root, &clone_id).display());
            Ok(())
        }
        LedgerCommands::Push { message } => {
            let state_root = state_root_from_env()?;
            let outcome = push_reporting(&repo, &state_root, &message)?;
            println!("{}", outcome.commit);
            Ok(())
        }
    }
}

pub fn fetch_reporting(
    repo: &Repo,
    state_root: &std::path::Path,
) -> Result<fetch::FetchOutcome, String> {
    let outcome = fetch::fetch(repo, state_root)?;
    if !outcome.pending.is_empty() {
        eprintln!("{}", pending_note(&outcome));
    }
    if outcome.unpublished {
        eprintln!("{}", unpublished_note(&outcome));
    }
    Ok(outcome)
}

pub fn push_reporting(
    repo: &Repo,
    state_root: &std::path::Path,
    message: &str,
) -> Result<push::PushOutcome, String> {
    let outcome = push::push(repo, state_root, message, &mut |attempt, status| {
        eprintln!("{}", retry_note(attempt, status));
    })?;
    for note in push_notes(&outcome) {
        eprintln!("{}", note);
    }
    Ok(outcome)
}

fn retry_note(attempt: usize, status: &push::PushStatus) -> String {
    format!(
        "note: push rejected (attempt {}/{}): {}; fetching again and retrying",
        attempt,
        push::MAX_ATTEMPTS,
        status.summary
    )
}

fn push_notes(outcome: &push::PushOutcome) -> Vec<String> {
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

fn unpublished_note(outcome: &fetch::FetchOutcome) -> String {
    format!(
        "note: {}'s {} does not exist yet; {} holds a new ledger.\n      The first tsk ledger push creates the branch",
        outcome.location.label(),
        outcome.location.ref_name(),
        outcome.path.display()
    )
}

fn pending_note(outcome: &fetch::FetchOutcome) -> String {
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
    note.push_str("      Push it with tsk ledger push, or discard it\n");
    note.push_str(&format!(
        "      deliberately with: git -C \"{}\" reset --hard {}",
        outcome.path.display(),
        outcome.commit
    ));
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
        assert!(note.ends_with("reset --hard abc123"));
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
