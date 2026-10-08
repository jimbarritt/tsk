use std::path::Path;

use clap::{Subcommand, ValueEnum};
use serde_json::{json, Value};

use crate::config;
use crate::ledger::location::{state_root_from_env, Repo};
use crate::ledger::nexus::{load_index, machine_name, valid_repo_id, Entry, NexusIndex};
use crate::ledger::register::{
    derive_repo_id, register, repo_name_from_url, NewEntry, Outcome, TerritoryChoice,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LedgerChoice {
    Nexus,
    Repo,
}

impl LedgerChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            LedgerChoice::Nexus => "nexus",
            LedgerChoice::Repo => "repo",
        }
    }
}

#[derive(Subcommand)]
pub enum NexusCommands {
    #[command(
        about = "Record the nexus repo URL in the user config, replacing any URL already recorded (the same as tsk config attach-nexus)"
    )]
    Add {
        #[arg(help = "Clone URL of the nexus repo")]
        url: String,
    },
    #[command(
        about = "Print the user config file path, the attached nexus URL, and the territories and repos its nexus.json lists"
    )]
    List {
        #[arg(long, help = "Print one JSON object in place of the text listing")]
        json: bool,
    },
    #[command(
        about = "Add this repo's entry to nexus.json on the nexus's default branch, then commit and push it with the machine's git credentials"
    )]
    RegisterRepo {
        #[arg(
            long,
            help = "Repo ID for the entry; defaults to the repo name, lowercased, with other characters as hyphens"
        )]
        id: Option<String>,
        #[arg(
            long,
            value_name = "TERRITORY_ID",
            help = "Territory to add the entry to; required when the nexus has more than one, and creates it when the nexus has none"
        )]
        territory: Option<String>,
        #[arg(
            long,
            value_name = "NAME",
            requires = "territory",
            help = "Name of a territory the command creates; defaults to its id"
        )]
        territory_name: Option<String>,
        #[arg(
            long,
            value_enum,
            default_value = "nexus",
            help = "Where the repo's ledger lives"
        )]
        ledger: LedgerChoice,
        #[arg(
            long,
            help = "Register a repo with no remote URL: the entry names this machine, and the id is written to the repo's tsk-repo-id"
        )]
        local: bool,
    },
}

pub fn run(action: NexusCommands) -> Result<(), String> {
    let file = config::required_config_file()?;
    match action {
        NexusCommands::Add { url } => config::attach_and_report(&file, &url),
        NexusCommands::List { json } => {
            let nexus = config::read_nexus_url(&file)?;
            let index = match &nexus {
                Some(url) => Some(load_index(&state_root_from_env()?, url)?),
                None => None,
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&listing_json(
                        &file,
                        nexus.as_deref(),
                        index.as_ref()
                    ))
                    .map_err(|e| format!("error: could not serialise the listing: {}", e))?
                );
            } else {
                print!("{}", listing_text(&file, nexus.as_deref(), index.as_ref()));
            }
            Ok(())
        }
        NexusCommands::RegisterRepo {
            id,
            territory,
            territory_name,
            ledger,
            local,
        } => register_repo(
            &file,
            id,
            TerritoryChoice {
                id: territory,
                name: territory_name,
            },
            ledger,
            local,
        ),
    }
}

fn register_repo(
    config_file: &Path,
    id: Option<String>,
    territory: TerritoryChoice,
    ledger: LedgerChoice,
    local: bool,
) -> Result<(), String> {
    let nexus = config::read_nexus_url(config_file)?
        .ok_or_else(|| "error: no nexus is attached; run tsk nexus add <url> first".to_string())?;
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = Repo::discover(&cwd)?;
    let entry = new_entry(&repo, id, ledger, local)?;
    let state_root = state_root_from_env()?;
    let outcome = register(&state_root, &nexus, &entry, &territory, &mut || {
        eprintln!("note: the nexus's default branch moved before the push; fetching again and retrying once")
    })?;
    if local {
        repo.write_repo_id(&entry.id)?;
    }
    print!("{}", outcome_text(&outcome, &entry, &nexus));
    Ok(())
}

fn new_entry(
    repo: &Repo,
    id: Option<String>,
    ledger: LedgerChoice,
    local: bool,
) -> Result<NewEntry, String> {
    let (name, url, machine) = if local {
        let machine = machine_name().ok_or_else(|| {
            "error: could not read this machine's name; set TSK_MACHINE_NAME".to_string()
        })?;
        let top = repo.git.run_line(["rev-parse", "--show-toplevel"])?;
        let name = Path::new(&top)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        (name, None, Some(machine))
    } else {
        let origin = repo.raw_origin_url()?.ok_or_else(|| {
            "error: this repo has no origin remote URL; run with --local to register it for this machine only"
                .to_string()
        })?;
        let name = repo_name_from_url(&origin).ok_or_else(|| {
            format!(
                "error: the origin URL {} is not a host URL that other machines can match; run with --local to register it for this machine only",
                origin
            )
        })?;
        (name, Some(origin), None)
    };
    let id = match id {
        Some(id) if valid_repo_id(&id) => id,
        Some(id) => {
            return Err(format!(
                "error: \"{}\" is not a valid repo id; use lowercase letters, digits and hyphens, starting with a letter or digit",
                id
            ))
        }
        None => derive_repo_id(&name).ok_or_else(|| {
            format!(
                "error: no valid repo id can be made from the repo name \"{}\"; pass --id <id>",
                name
            )
        })?,
    };
    Ok(NewEntry {
        id,
        url,
        local: machine,
        ledger: ledger.as_str().to_string(),
    })
}

pub fn outcome_text(outcome: &Outcome, entry: &NewEntry, nexus: &str) -> String {
    match outcome {
        Outcome::AlreadyRegistered { territory, entry } => {
            format!("already registered in territory {}: {}\n", territory, entry)
        }
        Outcome::Registered {
            territory,
            created,
            commit,
            branch,
            ..
        } => {
            let mut text = String::new();
            if *created {
                text.push_str(&format!("created territory {}\n", territory));
            }
            text.push_str(&format!(
                "registered in territory {}: {}\n",
                territory,
                entry.to_json()
            ));
            text.push_str(&format!("pushed {} to {} of {}\n", commit, branch, nexus));
            text.push_str("next: run tsk ledger fetch in this repo\n");
            text
        }
    }
}

fn entry_ledger(entry: &Entry) -> &str {
    entry.ledger.as_deref().unwrap_or("repo")
}

fn entry_place(entry: &Entry) -> String {
    match (&entry.url, &entry.local) {
        (Some(url), _) => format!("url: {}", url),
        (None, Some(machine)) => format!("local: {}", machine),
        (None, None) => "url: none".to_string(),
    }
}

pub fn listing_text(config_file: &Path, nexus: Option<&str>, index: Option<&NexusIndex>) -> String {
    let mut text = format!("config: {}\n", config_file.display());
    let Some(nexus) = nexus else {
        text.push_str("nexus: none attached\n");
        text.push_str("hint: run tsk nexus add <url> to attach one\n");
        return text;
    };
    text.push_str(&format!("nexus: {}\n", nexus));
    let territories = index.map(|i| i.territories()).unwrap_or(&[]);
    if territories.is_empty() {
        text.push_str("territories: none\n");
    }
    for territory in territories {
        if territory.name.is_empty() {
            text.push_str(&format!("territory: {}\n", territory.id));
        } else {
            text.push_str(&format!(
                "territory: {} ({})\n",
                territory.id, territory.name
            ));
        }
        if territory.repos.is_empty() {
            text.push_str("  no repos\n");
        }
        for entry in &territory.repos {
            text.push_str(&format!(
                "  {}  {}  ledger: {}\n",
                entry.id,
                entry_place(entry),
                entry_ledger(entry)
            ));
        }
    }
    text
}

pub fn listing_json(config_file: &Path, nexus: Option<&str>, index: Option<&NexusIndex>) -> Value {
    let territories: Vec<Value> = index
        .map(|i| i.territories())
        .unwrap_or(&[])
        .iter()
        .map(|territory| {
            let repos: Vec<Value> = territory
                .repos
                .iter()
                .map(|entry| {
                    json!({
                        "id": entry.id,
                        "url": entry.url,
                        "local": entry.local,
                        "ledger": entry_ledger(entry),
                    })
                })
                .collect();
            json!({
                "id": territory.id,
                "name": territory.name,
                "repos": repos,
            })
        })
        .collect();
    json!({
        "config": config_file.display().to_string(),
        "nexus": nexus,
        "territories": territories,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> NexusIndex {
        NexusIndex::parse(
            r#"{"version":1,"territories":[
              {"id":"work","name":"Work","repos":[
                {"id":"work-api","url":"https://example.test/acme/work-api","ledger":"nexus"},
                {"id":"scratch","local":"laptop","ledger":"nexus"},
                {"id":"tsk","url":"https://example.test/o/tsk"}
              ]},
              {"id":"empty"}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn text_lists_each_territory_and_repo() {
        let text = listing_text(
            Path::new("/c/tsk/config.toml"),
            Some("https://example.test/o/nexus"),
            Some(&index()),
        );
        assert_eq!(
            text,
            "config: /c/tsk/config.toml\n\
             nexus: https://example.test/o/nexus\n\
             territory: work (Work)\n\
             \x20 work-api  url: https://example.test/acme/work-api  ledger: nexus\n\
             \x20 scratch  local: laptop  ledger: nexus\n\
             \x20 tsk  url: https://example.test/o/tsk  ledger: repo\n\
             territory: empty\n\
             \x20 no repos\n"
        );
    }

    #[test]
    fn text_without_a_nexus_names_the_add_command() {
        let text = listing_text(Path::new("/c/config.toml"), None, None);
        assert_eq!(
            text,
            "config: /c/config.toml\nnexus: none attached\nhint: run tsk nexus add <url> to attach one\n"
        );
    }

    #[test]
    fn text_with_no_territories_says_so() {
        let empty = NexusIndex::parse("{}").unwrap();
        let text = listing_text(Path::new("/c"), Some("n"), Some(&empty));
        assert!(text.ends_with("nexus: n\nterritories: none\n"), "{}", text);
    }

    #[test]
    fn json_has_a_stable_shape() {
        let value = listing_json(
            Path::new("/c/config.toml"),
            Some("https://example.test/o/nexus"),
            Some(&index()),
        );
        assert_eq!(value["config"], "/c/config.toml");
        assert_eq!(value["nexus"], "https://example.test/o/nexus");
        assert_eq!(value["territories"][0]["id"], "work");
        assert_eq!(value["territories"][0]["name"], "Work");
        assert_eq!(
            value["territories"][0]["repos"][1],
            json!({"id": "scratch", "url": null, "local": "laptop", "ledger": "nexus"})
        );
        assert_eq!(value["territories"][0]["repos"][2]["ledger"], "repo");
        assert_eq!(value["territories"][1]["name"], "");
        assert_eq!(value["territories"][1]["repos"], json!([]));
    }

    fn work_api() -> NewEntry {
        NewEntry {
            id: "work-api".to_string(),
            url: Some("https://example.test/acme/work-api".to_string()),
            local: None,
            ledger: "nexus".to_string(),
        }
    }

    #[test]
    fn registered_text_names_the_entry_the_push_and_the_next_step() {
        let outcome = Outcome::Registered {
            territory: "work".to_string(),
            created: true,
            commit: "abc123".to_string(),
            branch: "refs/heads/main".to_string(),
            attempts: 1,
        };
        assert_eq!(
            outcome_text(&outcome, &work_api(), "https://example.test/o/nexus"),
            "created territory work\n\
             registered in territory work: {\"id\":\"work-api\",\"ledger\":\"nexus\",\"url\":\"https://example.test/acme/work-api\"}\n\
             pushed abc123 to refs/heads/main of https://example.test/o/nexus\n\
             next: run tsk ledger fetch in this repo\n"
        );
    }

    #[test]
    fn already_registered_text_prints_the_stored_entry() {
        let outcome = Outcome::AlreadyRegistered {
            territory: "work".to_string(),
            entry: json!({"id": "work-api", "url": "git@example.test:acme/work-api.git"}),
        };
        assert_eq!(
            outcome_text(&outcome, &work_api(), "n"),
            "already registered in territory work: {\"id\":\"work-api\",\"url\":\"git@example.test:acme/work-api.git\"}\n"
        );
    }

    #[test]
    fn json_without_a_nexus_has_null_and_no_territories() {
        let value = listing_json(Path::new("/c/config.toml"), None, None);
        assert_eq!(
            value,
            json!({"config": "/c/config.toml", "nexus": null, "territories": []})
        );
    }
}
