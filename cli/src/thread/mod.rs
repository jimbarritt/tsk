pub mod binding;
pub mod clock;
pub mod entry;
pub mod lookup;
pub mod ops;
pub mod session;
pub mod session_start;

use std::io::Read;

use clap::Subcommand;
use serde::Serialize;

use binding::{
    is_skipped, open_mission_lines, resolve, resolve_local, unbound_prompt, write_skip,
    UNBOUND_PROMPT,
};
use ops::StartOutcome;
use session::Session;

#[derive(Subcommand)]
pub enum ThreadCommands {
    #[command(
        about = "Mint a thread for a mission, bind this session or worktree to it, and push; prints started:<thread-id>, or resume-required:<thread-id> with exit status 2 when a binding exists. The mission must have a record, or a briefing document at missions/<category>/<ID>-<slug>.md; an unknown ID is refused. Clears tsk thread skip"
    )]
    Start {
        #[arg(help = "Mission ID, for example M-TSK-SCHEMA")]
        mission_id: String,
        #[arg(
            hide = true,
            help = "No longer needed. Accepted and ignored, so older plugin skills keep working"
        )]
        briefing_path: Option<String>,
    },
    #[command(
        about = "Append a continuation state entry to a thread and push; prints paused:<thread-id>. The mission comes from the thread. When the mission has a record, a task ID must name one of its tasks"
    )]
    Pause {
        #[arg(help = "Thread ID")]
        thread_id: String,
        #[arg(
            help = "Task in progress, for example T-05 or T-5; can be empty",
            allow_hyphen_values = true
        )]
        task_id: String,
        #[arg(
            help = "A short account of where things stand",
            allow_hyphen_values = true
        )]
        whats_next: String,
        #[arg(
            hide = true,
            allow_hyphen_values = true,
            help = "The old form is pause <thread-id> <mission-link> <task-id> <whats-next>. The mission link is ignored"
        )]
        legacy_whats_next: Option<String>,
    },
    #[command(
        about = "Bind this session or worktree to a thread, additively, and print its latest continuation state entry as JSON with a warning when another actor wrote to it"
    )]
    Resume {
        #[arg(help = "Thread ID")]
        thread_id: String,
    },
    #[command(
        about = "Remove this session's or worktree's binding without deleting the thread; prints detached:<thread-id>"
    )]
    Detach,
    #[command(
        about = "Record that this session or worktree works without a thread, so the Stop hook check stops asking for one; prints skipped. The record is local to this clone and is never pushed. tsk thread start and tsk thread resume clear it"
    )]
    Skip,
    #[command(
        about = "Delete a thread, its continuation state and every cloud session binding to it, and push; prints stopped:<thread-id>"
    )]
    Stop {
        #[arg(help = "Thread ID; defaults to the thread this session or worktree is bound to")]
        thread_id: Option<String>,
    },
    #[command(
        about = "List every thread on the ledger as one JSON object per line, most recently paused first"
    )]
    List,
    #[command(
        about = "Print this session's or worktree's binding as cloud:<thread-id> or worktree:<thread-id>; exit status 1 and no output with no binding"
    )]
    Binding {
        #[arg(long, help = "Read the existing ledger worktree without fetching")]
        no_fetch: bool,
    },
    #[command(
        about = "Stop hook check: exit 0 with no output when bound, when this session or worktree skipped a thread, or when the ledger holds no missions; otherwise print a block decision as JSON; reads and discards stdin; never fetches"
    )]
    Guard,
    #[command(
        about = "SessionStart hook: reads the session ID and source from stdin, exits with no output when another run already claimed that event, fetches the ledger, exports TSK_LEDGER_WT to CLAUDE_ENV_FILE when set, and prints the hook JSON with the session context in the --format shape (claude by default, or copilot); accepts session_id or sessionId on stdin; exits 0 even when the fetch fails"
    )]
    SessionStart {
        #[arg(
            long,
            value_enum,
            default_value = "claude",
            help = "Hook output format: claude (hookSpecificOutput JSON) or copilot (additionalContext JSON)"
        )]
        format: session_start::HookFormat,
        #[arg(
            long,
            value_enum,
            value_name = "TARGET",
            help = "In a Claude Code cloud session (CLAUDE_CODE_REMOTE=true), install the tsk plugin first, as tsk install-plugin does; only claude-cli is accepted. Step lines go to stderr. A failure adds a warning to the context"
        )]
        install_plugin: Option<crate::install_plugin::Target>,
    },
}

#[derive(Serialize)]
struct BlockDecision<'a> {
    decision: &'a str,
    reason: &'a str,
}

pub fn block_decision(reason: &str) -> String {
    serde_json::to_string(&BlockDecision {
        decision: "block",
        reason,
    })
    .unwrap_or_default()
}

/// The reason the Stop hook blocks, or `None` when it lets the session end: this session
/// or worktree skipped a thread, it is bound, or the ledger holds no missions. Reads the
/// ledger store as it is and never fetches.
fn guard_reason() -> Option<String> {
    let Ok(session) = Session::discover() else {
        return Some(UNBOUND_PROMPT.to_string());
    };
    if is_skipped(&session) {
        return None;
    }
    let local = session.store.exists_locally().unwrap_or(false);
    if !local {
        return Some(UNBOUND_PROMPT.to_string());
    }
    if resolve_local(&session).ok().flatten().is_some() {
        return None;
    }
    match (
        session.store.missions_load(),
        session.store.doc_list("missions/"),
    ) {
        (Ok(missions), Ok(documents)) if missions.is_empty() && documents.is_empty() => None,
        (Ok(missions), _) => Some(unbound_prompt(&open_mission_lines(&missions))),
        _ => Some(UNBOUND_PROMPT.to_string()),
    }
}

pub fn run(action: ThreadCommands) -> Result<i32, String> {
    if let ThreadCommands::Guard = action {
        let _ = std::io::stdin().read_to_end(&mut Vec::new());
        if let Some(reason) = guard_reason() {
            println!("{}", block_decision(&reason));
        }
        return Ok(0);
    }

    if let ThreadCommands::SessionStart {
        format,
        install_plugin,
    } = action
    {
        if install_plugin == Some(crate::install_plugin::Target::CopilotCli) {
            return Err(
                "error: --install-plugin accepts only claude-cli: GitHub Copilot CLI reads plugin hooks itself"
                    .to_string(),
            );
        }
        return Ok(session_start::run(format, install_plugin.is_some()));
    }

    let session = Session::discover()?;
    match action {
        ThreadCommands::Start {
            mission_id,
            briefing_path,
        } => {
            if briefing_path.is_some() {
                eprintln!(
                    "note: the briefing path argument of tsk thread start is no longer needed and is ignored"
                );
            }
            match ops::start(&session, &mission_id)? {
                StartOutcome::Started(id) => {
                    println!("started:{}", id);
                    Ok(0)
                }
                StartOutcome::ResumeRequired(id) => {
                    println!("resume-required:{}", id);
                    Ok(2)
                }
            }
        }
        ThreadCommands::Pause {
            thread_id,
            task_id,
            whats_next,
            legacy_whats_next,
        } => {
            // The old form puts a mission link before the task. It is ignored.
            let (task_id, whats_next) = match legacy_whats_next {
                Some(last) => {
                    eprintln!(
                        "note: the mission link argument of tsk thread pause is no longer needed and is ignored"
                    );
                    (whats_next, last)
                }
                None => (task_id, whats_next),
            };
            ops::pause(&session, &thread_id, &task_id, &whats_next)?;
            println!("paused:{}", thread_id);
            Ok(0)
        }
        ThreadCommands::Skip => {
            write_skip(&session)?;
            println!("skipped");
            Ok(0)
        }
        ThreadCommands::Resume { thread_id } => {
            let outcome = ops::resume(&session, &thread_id)?;
            println!("{}", ops::render_resume(&outcome));
            Ok(0)
        }
        ThreadCommands::Detach => {
            let id = ops::detach(&session)?;
            println!("detached:{}", id);
            Ok(0)
        }
        ThreadCommands::Stop { thread_id } => {
            let id = ops::stop(&session, thread_id.as_deref())?;
            println!("stopped:{}", id);
            Ok(0)
        }
        ThreadCommands::List => {
            for row in ops::list(&session)? {
                println!(
                    "{}",
                    serde_json::to_string(&row)
                        .map_err(|e| format!("error: could not render a thread: {}", e))?
                );
            }
            Ok(0)
        }
        ThreadCommands::Binding { no_fetch } => {
            let binding = if no_fetch {
                resolve_local(&session)?
            } else {
                session.refresh()?;
                resolve(&session)?
            };
            match binding {
                Some(binding) => {
                    println!("{}", binding);
                    Ok(0)
                }
                None => Ok(1),
            }
        }
        ThreadCommands::Guard | ThreadCommands::SessionStart { .. } => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_decision_is_compact_json_with_the_unbound_prompt() {
        let text = block_decision(UNBOUND_PROMPT);
        assert!(
            text.starts_with("{\"decision\":\"block\",\"reason\":\"No thread binding was found")
        );
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["reason"], UNBOUND_PROMPT);
    }
}
