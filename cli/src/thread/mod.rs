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

use binding::{resolve_at, resolve_local, UNBOUND_PROMPT};
use ops::StartOutcome;
use session::Session;

#[derive(Subcommand)]
pub enum ThreadCommands {
    #[command(
        about = "Mint a thread for a mission, scaffold it on the ledger, bind this session or worktree to it, and push; prints started:<thread-id>, or resume-required:<thread-id> with exit status 2 when a binding exists"
    )]
    Start {
        #[arg(help = "Mission ID, for example M-BOOT-04")]
        mission_id: String,
        #[arg(
            help = "Mission briefing path relative to the ledger root, for example missions/operational/M-BOOT-04-official-ledger.md"
        )]
        briefing_path: String,
    },
    #[command(
        about = "Append a continuation state entry to a thread and push; prints paused:<thread-id>"
    )]
    Pause {
        #[arg(help = "Thread ID")]
        thread_id: String,
        #[arg(
            help = "Mission briefing the thread works on, relative to the ledger root",
            allow_hyphen_values = true
        )]
        mission_link: String,
        #[arg(
            help = "Task in progress, for example T-05; can be empty",
            allow_hyphen_values = true
        )]
        task_id: String,
        #[arg(
            help = "A short account of where things stand",
            allow_hyphen_values = true
        )]
        whats_next: String,
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
        about = "Stop hook check: exit 0 with no output when bound, otherwise print a block decision as JSON; reads and discards stdin; never fetches"
    )]
    Guard,
    #[command(
        about = "SessionStart hook: reads the session ID and source from stdin, exits with no output when another run already claimed that event, fetches the ledger, exports TSK_LEDGER_WT to CLAUDE_ENV_FILE when set, and prints the hook JSON with the session context; exits 0 even when the fetch fails"
    )]
    SessionStart,
}

#[derive(Serialize)]
struct BlockDecision<'a> {
    decision: &'a str,
    reason: &'a str,
}

pub fn block_decision() -> String {
    serde_json::to_string(&BlockDecision {
        decision: "block",
        reason: UNBOUND_PROMPT,
    })
    .unwrap_or_default()
}

pub fn run(action: ThreadCommands) -> Result<i32, String> {
    if let ThreadCommands::Guard = action {
        let _ = std::io::stdin().read_to_end(&mut Vec::new());
        let bound = Session::discover()
            .and_then(|session| resolve_local(&session))
            .map(|binding| binding.is_some())
            .unwrap_or(false);
        if !bound {
            println!("{}", block_decision());
        }
        return Ok(0);
    }

    if let ThreadCommands::SessionStart = action {
        return Ok(session_start::run());
    }

    let session = Session::discover()?;
    match action {
        ThreadCommands::Start {
            mission_id,
            briefing_path,
        } => match ops::start(&session, &mission_id, &briefing_path)? {
            StartOutcome::Started(id) => {
                println!("started:{}", id);
                Ok(0)
            }
            StartOutcome::ResumeRequired(id) => {
                println!("resume-required:{}", id);
                Ok(2)
            }
        },
        ThreadCommands::Pause {
            thread_id,
            mission_link,
            task_id,
            whats_next,
        } => {
            ops::pause(&session, &thread_id, &mission_link, &task_id, &whats_next)?;
            println!("paused:{}", thread_id);
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
                let wt = session.refresh()?;
                resolve_at(&session, &wt)?
            };
            match binding {
                Some(binding) => {
                    println!("{}", binding);
                    Ok(0)
                }
                None => Ok(1),
            }
        }
        ThreadCommands::Guard | ThreadCommands::SessionStart => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_decision_is_compact_json_with_the_unbound_prompt() {
        let text = block_decision();
        assert!(
            text.starts_with("{\"decision\":\"block\",\"reason\":\"No thread binding was found")
        );
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["reason"], UNBOUND_PROMPT);
    }
}
