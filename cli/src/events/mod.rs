pub mod batch;
pub mod json;
pub mod ops;
pub mod queue;

use std::io::Read;
use std::path::PathBuf;

use clap::Subcommand;

use crate::ledger::location::{state_root_from_env, Repo};
use batch::{parse_batch, BatchDefaults};
use ops::{AdvanceOutcome, AppendRequest, Ledger};

#[derive(Subcommand)]
pub enum EventsCommands {
    #[command(
        about = "Append one event envelope to the ledger's external event queue and push; prints queued"
    )]
    Append {
        #[arg(help = "Event source, for example github", allow_hyphen_values = true)]
        source: String,
        #[arg(
            help = "Kind of event, for example dependabot_alert",
            allow_hyphen_values = true
        )]
        event_type: String,
        #[arg(
            help = "What happened, for example created, fixed or polled",
            allow_hyphen_values = true
        )]
        action: String,
        #[arg(
            help = "Repository the event concerns, owner/repo",
            allow_hyphen_values = true
        )]
        repo: String,
        #[arg(
            help = "File holding the source's JSON payload; the first JSON value in it is queued, or null when it is empty"
        )]
        payload_file: PathBuf,
    },
    #[command(
        about = "Read event lines from stdin as NDJSON and append them all to the ledger's external event queue with one fetch and one push; prints queued:<count>",
        long_about = "Read event lines from stdin as NDJSON and append them all to the ledger's external event queue with one fetch and one push; prints queued:<count>.\n\nEach non-blank line is a JSON object with the string fields event_type and repo, a payload holding any JSON value, and optionally the string fields source and action. A line's source or action overrides --source or --action. Every line is checked before the fetch: a bad line stops with exit status 1, names the line number, and writes nothing. Empty stdin fetches and pushes nothing and prints queued:0."
    )]
    AppendBatch {
        #[arg(
            long,
            help = "Event source for every line that has no source field, for example github",
            allow_hyphen_values = true
        )]
        source: Option<String>,
        #[arg(
            long,
            help = "Action for every line that has no action field, for example polled",
            allow_hyphen_values = true
        )]
        action: Option<String>,
    },
    #[command(
        about = "Fetch the ledger and print the queued events past the watermark as {\"new_count\":N,\"total_count\":M,\"events\":[...]}, oldest first; writes nothing"
    )]
    ReadNew,
    #[command(
        about = "Set the watermark to a processed event count and push; prints watermark:<count>; prints nothing when already at the count; exit status 1 for a lower count"
    )]
    AdvanceWatermark {
        #[arg(
            help = "Number of queue lines processed, from read-new's total_count",
            allow_hyphen_values = true
        )]
        count: String,
    },
}

pub fn run(action: EventsCommands) -> Result<i32, String> {
    let count = match &action {
        EventsCommands::AdvanceWatermark { count } => Some(ops::parse_count(count)?),
        _ => None,
    };
    let batch = match &action {
        EventsCommands::AppendBatch { source, action } => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| format!("{}: could not read stdin: {}", batch::BATCH_COMMAND, e))?;
            let events = parse_batch(
                &text,
                &BatchDefaults {
                    source: source.as_deref(),
                    action: action.as_deref(),
                },
            )?;
            if events.is_empty() {
                println!("queued:0");
                return Ok(0);
            }
            Some(events)
        }
        _ => None,
    };
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = Repo::discover(&cwd)?;
    let state_root = state_root_from_env()?;
    let ledger = Ledger {
        repo: &repo,
        state_root: &state_root,
    };
    match action {
        EventsCommands::Append {
            source,
            event_type,
            action,
            repo,
            payload_file,
        } => {
            ops::append(
                &ledger,
                &AppendRequest {
                    source: &source,
                    event_type: &event_type,
                    action: &action,
                    repo: &repo,
                    payload_file: &payload_file,
                },
            )?;
            println!("queued");
            Ok(0)
        }
        EventsCommands::AppendBatch { .. } => {
            let queued = ops::append_batch(&ledger, &batch.unwrap_or_default())?;
            println!("queued:{}", queued);
            Ok(0)
        }
        EventsCommands::ReadNew => {
            println!("{}", ops::read_new(&ledger)?.render());
            Ok(0)
        }
        EventsCommands::AdvanceWatermark { .. } => {
            match ops::advance(&ledger, count.unwrap_or_default())? {
                AdvanceOutcome::Advanced(count) => println!("watermark:{}", count),
                AdvanceOutcome::AlreadyAt(current) => {
                    eprintln!(
                        "note: watermark already at {}; nothing to advance.",
                        current
                    )
                }
            }
            Ok(0)
        }
    }
}
