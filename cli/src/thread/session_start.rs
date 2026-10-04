use std::io::{Read, Write};
use std::path::Path;

use serde::Serialize;

use crate::ledger::fetch::{fetch, FetchOutcome};

use super::binding::{resolve_at, UNBOUND_PROMPT};
use super::session::Session;

pub const ENV_FILE_VAR: &str = "CLAUDE_ENV_FILE";
pub const LEDGER_WT_VAR: &str = "TSK_LEDGER_WT";

#[derive(Serialize)]
struct SpecificOutput<'a> {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'a str,
    #[serde(rename = "additionalContext")]
    additional_context: &'a str,
}

#[derive(Serialize)]
struct HookOutput<'a> {
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: SpecificOutput<'a>,
}

pub fn hook_output(context: &str) -> String {
    serde_json::to_string(&HookOutput {
        hook_specific_output: SpecificOutput {
            hook_event_name: "SessionStart",
            additional_context: context,
        },
    })
    .unwrap_or_default()
}

pub fn fetched_context(path: &Path, pending: &PendingNote, thread_message: &str) -> String {
    let wt = path.display();
    let mut context = format!(
        "The ledger was fetched and materialised at {} (also exported as ${}). Read {}/index.md next.",
        wt, LEDGER_WT_VAR, wt
    );
    if !pending.lines.is_empty() {
        context.push(' ');
        context.push_str(&pending_warning(path, pending));
    }
    context.push(' ');
    context.push_str(thread_message);
    context
}

pub struct PendingNote {
    pub remote_ref: String,
    pub origin_commit: String,
    pub lines: Vec<String>,
}

impl PendingNote {
    pub fn from_outcome(outcome: &FetchOutcome) -> PendingNote {
        PendingNote {
            remote_ref: format!(
                "{}'s {}",
                outcome.location.label(),
                outcome.location.ref_name()
            ),
            origin_commit: outcome.commit.clone(),
            lines: outcome.pending.clone(),
        }
    }
}

fn pending_warning(path: &Path, pending: &PendingNote) -> String {
    format!(
        "WARNING: the ledger worktree holds a commit that is not on {}, so it was left as it is rather than reset: {}. A worker restart can end a turn between a commit and its push, which leaves exactly this state, so this may be your own work from a turn you hold no record of making. Do not assume another actor made it, and do not discard it on that basis. Read what it changes first, with: git -C '{}' log -p {}..HEAD. Then push it with tsk ledger push, or discard it deliberately once you know what it is.",
        pending.remote_ref,
        pending.lines.join(";"),
        path.display(),
        pending.origin_commit
    )
}

pub fn thread_message(binding: Option<&str>) -> String {
    match binding {
        Some(binding) => {
            let thread_id = binding.split_once(':').map(|(_, id)| id).unwrap_or(binding);
            format!(
                "An existing thread binding was found: {}. Run /tsk:resume-thread {} next.",
                binding, thread_id
            )
        }
        None => UNBOUND_PROMPT.to_string(),
    }
}

pub fn failure_context(error: &str) -> String {
    format!(
        "Warning: the ledger could not be fetched automatically at session start ({}). Run `tsk ledger fetch` manually before reading index.md.",
        error.replace('\n', " ").trim()
    )
}

fn export_to_env_file(path: &Path) -> Result<(), String> {
    let Some(file) = std::env::var_os(ENV_FILE_VAR).filter(|v| !v.is_empty()) else {
        return Ok(());
    };
    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(|e| format!("could not open {}: {}", Path::new(&file).display(), e))?;
    writeln!(handle, "export {}=\"{}\"", LEDGER_WT_VAR, path.display())
        .map_err(|e| format!("could not write {}: {}", Path::new(&file).display(), e))
}

fn context_after_fetch(session: &Session) -> Result<String, String> {
    let outcome = fetch(&session.repo, &session.state_root)?;
    let binding = resolve_at(session, &outcome.path)
        .ok()
        .flatten()
        .map(|b| b.to_string());
    let mut context = fetched_context(
        &outcome.path,
        &PendingNote::from_outcome(&outcome),
        &thread_message(binding.as_deref()),
    );
    if let Err(e) = export_to_env_file(&outcome.path) {
        context.push_str(&format!(
            " WARNING: {} was not exported ({}); use the path above.",
            LEDGER_WT_VAR, e
        ));
    }
    Ok(context)
}

pub fn run() -> i32 {
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
    let context = Session::discover()
        .and_then(|session| context_after_fetch(&session))
        .unwrap_or_else(|e| failure_context(&e));
    println!("{}", hook_output(&context));
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn no_pending() -> PendingNote {
        PendingNote {
            remote_ref: "origin's refs/heads/tsk/ledger".to_string(),
            origin_commit: "abc123".to_string(),
            lines: Vec::new(),
        }
    }

    #[test]
    fn hook_output_is_one_compact_object_in_hook_order() {
        let text = hook_output("a \"b\"");
        assert_eq!(
            text,
            "{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"a \\\"b\\\"\"}}"
        );
    }

    #[test]
    fn fetched_context_names_the_path_and_the_next_read() {
        let path = PathBuf::from("/s/ledger");
        let context = fetched_context(&path, &no_pending(), "Next.");
        assert_eq!(
            context,
            "The ledger was fetched and materialised at /s/ledger (also exported as $TSK_LEDGER_WT). Read /s/ledger/index.md next. Next."
        );
    }

    #[test]
    fn fetched_context_adds_the_pending_warning() {
        let path = PathBuf::from("/s/ledger");
        let pending = PendingNote {
            lines: vec!["1111111 first".to_string(), "2222222 second".to_string()],
            ..no_pending()
        };
        let context = fetched_context(&path, &pending, "Next.");
        assert!(context.contains(
            "WARNING: the ledger worktree holds a commit that is not on origin's refs/heads/tsk/ledger"
        ));
        assert!(context.contains("1111111 first;2222222 second"));
        assert!(context.contains("git -C '/s/ledger' log -p abc123..HEAD"));
        assert!(context.contains("push it with tsk ledger push"));
        assert!(context.ends_with(" Next."));
    }

    #[test]
    fn thread_message_names_the_binding_and_the_resume_command() {
        assert_eq!(
            thread_message(Some("worktree:1234abcd")),
            "An existing thread binding was found: worktree:1234abcd. Run /tsk:resume-thread 1234abcd next."
        );
        assert_eq!(thread_message(None), UNBOUND_PROMPT);
    }

    #[test]
    fn failure_context_asks_for_a_manual_fetch() {
        let context = failure_context("error: git fetch failed\nfatal: no remote");
        assert!(context.contains("error: git fetch failed fatal: no remote"));
        assert!(context.contains("Run `tsk ledger fetch` manually"));
    }
}
