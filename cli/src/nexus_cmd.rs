use std::path::Path;

use clap::Subcommand;
use serde_json::{json, Value};

use crate::config;
use crate::ledger::location::state_root_from_env;
use crate::ledger::nexus::{load_index, Entry, NexusIndex};

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

    #[test]
    fn json_without_a_nexus_has_null_and_no_territories() {
        let value = listing_json(Path::new("/c/config.toml"), None, None);
        assert_eq!(
            value,
            json!({"config": "/c/config.toml", "nexus": null, "territories": []})
        );
    }
}
