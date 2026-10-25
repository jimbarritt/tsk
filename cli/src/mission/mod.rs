//! `tsk mission`: mission records in the ledger. Every write changes the local store only;
//! `tsk ledger push` publishes. The rules live in [`model`] and work on plain values, so
//! the file and Dolt backends share them. A backend only loads and saves missions.

pub mod model;
pub mod mv;
pub mod prefix;
pub mod records;
pub mod show;
pub mod table;

use std::io::Read;

use clap::Subcommand;

use crate::ledger::store::{self, check_docs_path, LedgerStore};
use model::{Category, ListKind, Mission};

#[derive(Subcommand)]
pub enum MissionCommands {
    #[command(
        about = "Create a mission in the local store; an administrative mission starts in-progress"
    )]
    Create {
        #[arg(
            help = "Mission ID, for example M-TSK-SCHEMA. With a mission prefix (tsk mission prefix) it is added when missing, and a number at the end marks a part of a mission"
        )]
        id: String,
        #[arg(allow_hyphen_values = true, help = "One-line name")]
        name: String,
        #[arg(long, help = "operational (the default) or administrative")]
        category: Option<String>,
        #[arg(long, help = "ID of the parent mission")]
        parent: Option<String>,
    },
    #[command(
        about = "Set a field: name, status, purpose or decision-authority; the value is read from stdin when it is -. Constraints and out-of-scope are lists: see the constraint and out-of-scope commands"
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
    #[command(about = "Add, remove and list the constraints of a mission, one line each")]
    Constraint {
        #[command(subcommand)]
        action: ItemCommands,
    },
    #[command(
        name = "out-of-scope",
        about = "Add, remove and list what is out of scope for a mission, one line each"
    )]
    OutOfScope {
        #[command(subcommand)]
        action: ItemCommands,
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
        about = "Print the mission prefix of this ledger, or set it: 3 or 4 capital letters that namespace every mission ID, such as TSK (ADR 0027). A new mission ID then reads M-<prefix>-<NAME>, and a number at the end marks a part of a mission. Setting it renames no mission. Exits 1 and says how to set it when the ledger has none"
    )]
    Prefix {
        #[arg(help = "The prefix to set; prints the prefix when omitted")]
        prefix: Option<String>,
    },
    #[command(
        about = "Rename a mission: only its ID changes, and every record, thread, block and parent follows. Linked documents named for the mission are renamed too. Prints one line per change, then one line for each record whose text names the old ID, which is not changed. A local write"
    )]
    Mv {
        #[arg(help = "The mission ID now")]
        from: String,
        #[arg(help = "The new mission ID; it must not be the ID of a mission")]
        to: String,
        #[arg(long, help = "Print the lines and change nothing")]
        dry_run: bool,
        #[arg(
            long,
            help = "Also replace the old ID with the new one in the text of every mission, as tsk mission rewrite does, in place of the mentions lines"
        )]
        rewrite: bool,
    },
    #[command(
        about = "Replace an ID with another, as a whole word, in the text of every mission: names, purposes, objectives, constraints, out-of-scope items, tasks, decisions, questions and answers. Prints one line for each text it changed. The old ID can be that of a mission renamed earlier; the new one has to be a mission. Documents are not changed. A local write"
    )]
    Rewrite {
        #[arg(help = "The ID to replace")]
        from: String,
        #[arg(help = "The ID of a mission that replaces it")]
        to: String,
        #[arg(long, help = "Print the lines and change nothing")]
        dry_run: bool,
    },
    #[command(
        about = "Print one mission as plain text for a person: purpose, decision authority, objectives, constraints, out of scope, blockers, tasks, decisions, open and answered questions, and linked documents. The data of mission list <id> --json, with no new field"
    )]
    Show {
        #[arg(help = "The mission ID")]
        id: String,
    },
    #[command(
        about = "List missions, one per line: id, status, blocked-by, parent, name; or one mission with an id. The columns are aligned with spaces; --porcelain prints them tab-separated; --json prints the full records"
    )]
    List {
        id: Option<String>,
        #[arg(long, help = "Print the full record as JSON")]
        json: bool,
        #[arg(
            long,
            help = "Print tab-separated columns, the stable form for scripts, instead of aligned columns"
        )]
        porcelain: bool,
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

#[derive(Subcommand)]
pub enum ItemCommands {
    #[command(about = "Add an item; prints its number")]
    Add {
        id: String,
        #[arg(allow_hyphen_values = true, help = "One-line text")]
        text: String,
    },
    #[command(about = "Remove item n; its number is not reused")]
    Remove { id: String, n: i64 },
    #[command(about = "List the items: n, text")]
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
            let prefix = store.mission_prefix()?;
            let missions = store.missions_load()?;
            // The prefix is added when it is missing, and the form is checked after.
            let placed = prefix::place(&missions, prefix.as_deref(), &id, parent.as_deref(), None)?;
            let created = model::create(
                &missions,
                &placed.id,
                &name,
                category,
                placed.parent.as_deref(),
            )?;
            store.mission_save(&created)?;
            if placed.added {
                eprintln!("note: the mission prefix was added to the ID");
                println!("{}", placed.id);
            } else if prefix.is_none() && !missions.is_empty() {
                eprintln!("{}", prefix::missing_note());
            }
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
        MissionCommands::Constraint { action } => items(store, ListKind::Constraint, action)?,
        MissionCommands::OutOfScope { action } => items(store, ListKind::OutOfScope, action)?,
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
        MissionCommands::Show { id } => show::run(store, &id)?,
        MissionCommands::Prefix { prefix } => prefix::run(store, prefix.as_deref())?,
        MissionCommands::Mv {
            from,
            to,
            dry_run,
            rewrite,
        } => mv::mission_mv(store, &from, &to, dry_run, rewrite)?,
        MissionCommands::Rewrite { from, to, dry_run } => mv::rewrite(store, &from, &to, dry_run)?,
        MissionCommands::List {
            id,
            json,
            porcelain,
        } => list(store, id.as_deref(), json, porcelain)?,
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

fn items(store: &dyn LedgerStore, kind: ListKind, action: ItemCommands) -> Result<(), String> {
    match action {
        ItemCommands::Add { id, text } => {
            let saved = change(store, |missions| {
                model::item_add(missions, &id, kind, &text)
            })?;
            let next = match kind {
                ListKind::Constraint => saved.next_constraint,
                ListKind::OutOfScope => saved.next_out_of_scope,
            };
            println!("{}", next - 1);
        }
        ItemCommands::Remove { id, n } => {
            change(store, |missions| model::item_remove(missions, &id, kind, n))?;
        }
        ItemCommands::List { id } => {
            let missions = store.missions_load()?;
            let mission = model::find(&missions, &id)?;
            let shown = match kind {
                ListKind::Constraint => &mission.constraints,
                ListKind::OutOfScope => &mission.out_of_scope,
            };
            for item in shown {
                println!("{}", model::item_line(item));
            }
        }
    }
    Ok(())
}

fn list(
    store: &dyn LedgerStore,
    id: Option<&str>,
    json: bool,
    porcelain: bool,
) -> Result<(), String> {
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
        let lines = shown
            .into_iter()
            .map(|mission| model::list_line(&missions, mission))
            .collect();
        let empty = match id {
            Some(_) => "no mission to show",
            None => "no missions yet: create one with tsk mission create, or run tsk thread start",
        };
        table::print_lines_or(lines, porcelain, empty);
    }
    Ok(())
}
