//! `tsk mission`: mission records in the ledger. Every write changes the local store only;
//! `tsk ledger push` publishes. The rules live in [`model`] and work on plain values, so
//! the file and Dolt backends share them. A backend only loads and saves missions.

pub mod model;
pub mod records;

use std::io::Read;

use clap::Subcommand;

use crate::ledger::store::{self, check_docs_path, LedgerStore};
use model::{Category, Mission};

#[derive(Subcommand)]
pub enum MissionCommands {
    #[command(
        about = "Create a mission in the local store; an administrative mission starts in-progress"
    )]
    Create {
        #[arg(help = "Mission ID, for example M-TSK-SCHEMA")]
        id: String,
        #[arg(allow_hyphen_values = true, help = "One-line name")]
        name: String,
        #[arg(long, help = "operational (the default) or administrative")]
        category: Option<String>,
        #[arg(long, help = "ID of the parent mission")]
        parent: Option<String>,
    },
    #[command(
        about = "Set a field: name, status, purpose, constraints, out-of-scope or decision-authority; the value is read from stdin when it is -"
    )]
    Set {
        id: String,
        field: String,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    #[command(about = "Record that a mission is blocked by another; refuses a cycle")]
    Block {
        id: String,
        #[arg(help = "ID of the blocking mission")]
        by_id: String,
    },
    #[command(about = "Remove a block")]
    Unblock { id: String, by_id: String },
    #[command(about = "Add, tick, remove and list objective outcomes")]
    Objective {
        #[command(subcommand)]
        action: ObjectiveCommands,
    },
    #[command(about = "Link a document on the tsk/docs branch to a mission")]
    Link {
        id: String,
        #[arg(help = "Document path, as tsk ledger doc list prints it")]
        path: String,
        #[arg(long, help = "intelligence, report or design")]
        role: String,
    },
    #[command(about = "Remove a document link")]
    Unlink { id: String, path: String },
    #[command(
        about = "List missions, one per line: id, status, blocked-by, parent, name; or one mission with an id"
    )]
    List {
        id: Option<String>,
        #[arg(long, help = "Print the full record as JSON")]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum ObjectiveCommands {
    #[command(about = "Add an outcome; prints its number")]
    Add {
        id: String,
        #[arg(allow_hyphen_values = true)]
        outcome: String,
    },
    #[command(about = "Mark outcome n as met")]
    Done { id: String, n: i64 },
    #[command(about = "Mark outcome n as not met")]
    Undone { id: String, n: i64 },
    #[command(about = "Remove outcome n; its number is not reused")]
    Remove { id: String, n: i64 },
    #[command(about = "List the outcomes: n, met or open, outcome")]
    List { id: String },
}

pub fn run(action: MissionCommands) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let store = store::open(&cwd)?;
    run_with(store.as_ref(), action)
}

fn read_stdin() -> Result<String, String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| format!("error: could not read stdin: {}", e))?;
    Ok(text)
}

/// Load, change one mission by the shared rules and save it. Nothing is saved on an error.
pub(super) fn change(
    store: &dyn LedgerStore,
    apply: impl FnOnce(&[Mission]) -> Result<Mission, String>,
) -> Result<Mission, String> {
    let missions = store.missions_load()?;
    let updated = apply(&missions)?;
    store.mission_save(&updated)?;
    Ok(updated)
}

pub fn run_with(store: &dyn LedgerStore, action: MissionCommands) -> Result<(), String> {
    match action {
        MissionCommands::Create {
            id,
            name,
            category,
            parent,
        } => {
            let category = match category {
                Some(value) => Category::parse(&value)?,
                None => Category::Operational,
            };
            change(store, |missions| {
                model::create(missions, &id, &name, category, parent.as_deref())
            })?;
        }
        MissionCommands::Set { id, field, value } => {
            let value = if value == "-" {
                let text = read_stdin()?;
                // A name or a status is one token: drop the line break stdin ends with.
                if field == "name" || field == "status" {
                    model::strip_line_break(&text).to_string()
                } else {
                    text
                }
            } else {
                value
            };
            change(store, |missions| model::set(missions, &id, &field, &value))?;
        }
        MissionCommands::Block { id, by_id } => {
            change(store, |missions| model::block(missions, &id, &by_id))?;
        }
        MissionCommands::Unblock { id, by_id } => {
            change(store, |missions| model::unblock(missions, &id, &by_id))?;
        }
        MissionCommands::Objective { action } => objective(store, action)?,
        MissionCommands::Link { id, path, role } => {
            // The stored path is the normalised one; a path that cannot be a document
            // on the branch does not exist.
            let path = check_docs_path(&path).unwrap_or(path);
            let exists = store.doc_exists(&path)?;
            change(store, |missions| {
                model::link(missions, &id, &path, &role, exists)
            })?;
        }
        MissionCommands::Unlink { id, path } => {
            let path = check_docs_path(&path).unwrap_or(path);
            change(store, |missions| model::unlink(missions, &id, &path))?;
        }
        MissionCommands::List { id, json } => list(store, id.as_deref(), json)?,
    }
    Ok(())
}

fn objective(store: &dyn LedgerStore, action: ObjectiveCommands) -> Result<(), String> {
    match action {
        ObjectiveCommands::Add { id, outcome } => {
            let saved = change(store, |missions| {
                model::objective_add(missions, &id, &outcome)
            })?;
            println!("{}", saved.next_objective - 1);
        }
        ObjectiveCommands::Done { id, n } => {
            change(store, |missions| {
                model::objective_met(missions, &id, n, true)
            })?;
        }
        ObjectiveCommands::Undone { id, n } => {
            change(store, |missions| {
                model::objective_met(missions, &id, n, false)
            })?;
        }
        ObjectiveCommands::Remove { id, n } => {
            change(store, |missions| model::objective_remove(missions, &id, n))?;
        }
        ObjectiveCommands::List { id } => {
            let missions = store.missions_load()?;
            let mission = missions
                .iter()
                .find(|m| m.id == id)
                .ok_or_else(|| format!("error: no such mission: {}", id))?;
            for outcome in &mission.objective {
                println!("{}", model::objective_line(outcome));
            }
        }
    }
    Ok(())
}

fn list(store: &dyn LedgerStore, id: Option<&str>, json: bool) -> Result<(), String> {
    let missions = store.missions_load()?;
    let shown: Vec<&Mission> = match id {
        Some(id) => vec![missions
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| format!("error: no such mission: {}", id))?],
        None => missions.iter().collect(),
    };
    if json {
        let records: Vec<model::JsonMission> =
            shown.iter().map(|m| model::to_json(&missions, m)).collect();
        let printed = match (id, records.first()) {
            (Some(_), Some(record)) => serde_json::to_string(record),
            _ => serde_json::to_string(&records),
        };
        println!(
            "{}",
            printed.map_err(|e| format!("error: could not print JSON: {}", e))?
        );
    } else {
        for mission in shown {
            println!("{}", model::list_line(&missions, mission));
        }
    }
    Ok(())
}
