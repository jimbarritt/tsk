use std::path::Path;

use clap::{Subcommand, ValueEnum};
use serde_json::{json, Value};

use crate::config;
use crate::ledger::location::{state_root_from_env, Repo};
use crate::ledger::nexus::{
    load_index, machine_name, valid_repo_id, Entry, NexusIndex, NEXUS_FILE,
};
use crate::ledger::refs::{LedgerRefs, RefsSource};
use crate::ledger::register::{
    derive_repo_id, point_at_nexus, register, repo_name_from_url, NewEntry, Outcome,
    TerritoryChoice,
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
        about = "Add this repo's entry to nexus.json on the nexus's default branch, then commit and push it with the machine's git credentials. On an empty nexus it creates nexus.json"
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
            help = "Territory to add the entry to; required when the nexus has more than one, and creates it when the nexus has none or has no nexus.json"
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
            value_parser = ["branches", "custom"],
            help = "Which refs hold the repo's ledger: branches under refs/heads/*, or a custom ref (refs/tsk/*). Written to the entry as ledger_refs; omit it to leave the choice to the repo config and discovery"
        )]
        ledger_refs: Option<String>,
        #[arg(
            long,
            help = "Register a repo with no remote URL: the entry names this machine, and the id is written to the repo's tsk-repo-id"
        )]
        local: bool,
    },
    #[command(
        about = "Move this repo's ledger (records and documents) from its own repo into the attached nexus: copy both refs with their history, then set the nexus.json entry to ledger nexus. The old refs on origin stay until you delete them"
    )]
    MoveLedger {
        #[arg(
            long,
            help = "Repo ID for the entry; defaults to the repo name, lowercased, with other characters as hyphens"
        )]
        id: Option<String>,
        #[arg(
            long,
            value_name = "TERRITORY_ID",
            help = "Territory to add the entry to when the nexus has no entry for this repo"
        )]
        territory: Option<String>,
        #[arg(
            long,
            value_parser = ["branches", "custom"],
            help = "Which refs hold the ledger in the nexus; defaults to the refs it uses now"
        )]
        ledger_refs: Option<String>,
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
            ledger_refs,
            local,
        } => register_repo(
            &file,
            id,
            TerritoryChoice {
                id: territory,
                name: territory_name,
            },
            ledger,
            ledger_refs,
            local,
        ),
        NexusCommands::MoveLedger {
            id,
            territory,
            ledger_refs,
        } => move_ledger(&file, id, territory, ledger_refs),
    }
}

fn move_ledger(
    config_file: &Path,
    id: Option<String>,
    territory: Option<String>,
    ledger_refs: Option<String>,
) -> Result<(), String> {
    let nexus = config::read_nexus_url(config_file)?
        .ok_or_else(|| "error: no nexus is attached; run tsk nexus add <url> first".to_string())?;
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = Repo::discover(&cwd)?;
    let state_root = state_root_from_env()?;
    let source = repo.location(&state_root)?;
    if source.repo_id().is_some() {
        return Err(format!(
            "error: this repo's ledger is already held in the nexus, on {}",
            source.ref_name()
        ));
    }

    // Nothing that is only on this machine may be left behind.
    let store = crate::ledger::store::open(&cwd)?;
    let status = store.status()?;
    if let Some(note) = &status.fetch_note {
        return Err(format!(
            "error: could not check the ledger against origin, so it cannot be moved: {}",
            note.trim_start_matches("error: ")
        ));
    }
    if status.has_unpushed() {
        return Err(
            "error: the ledger store on this machine holds changes that origin lacks; run tsk ledger push first"
                .to_string(),
        );
    }
    let prefix = ledger_prefix(&cwd)?;
    drop(store);

    let refs = match ledger_refs.as_deref() {
        Some(text) => LedgerRefs::parse(text)
            .ok_or_else(|| format!("error: \"{}\" is not branches or custom", text))?,
        None => source.refs(),
    };
    let entry = new_entry(
        &repo,
        id,
        LedgerChoice::Nexus,
        Some(refs.as_str().to_string()),
        false,
        prefix,
    )?;
    let target = crate::ledger::location::LedgerLocation::nexus(&nexus, &entry.id)
        .with_refs(refs, RefsSource::Nexus);

    let pairs = [
        (source.clone(), target.clone()),
        (source.docs(), target.docs()),
    ];
    let git = &repo.git;
    let mut moved = Vec::new();
    for (from, to) in &pairs {
        let tip = crate::ledger::store::dolt::remote_tip(git, from)?;
        let Some(tip) = tip else {
            if from.ref_name() == source.ref_name() {
                return Err(format!(
                    "error: origin has no {}, so there is no ledger to move",
                    from.ref_name()
                ));
            }
            continue;
        };
        match crate::ledger::store::dolt::remote_tip(git, to)? {
            Some(held) if held == tip => {
                moved.push(format!("{} already holds {}", to.ref_name(), short(&tip)));
                continue;
            }
            Some(held) => {
                return Err(format!(
                    "error: the nexus already holds {} at {}, which is not origin's {} at {}; tsk does not overwrite a ledger",
                    to.ref_name(),
                    short(&held),
                    from.ref_name(),
                    short(&tip)
                ))
            }
            None => {}
        }
        git.run(["fetch", "--quiet", from.remote(), from.ref_name()])?;
        let refspec = format!("{}:{}", tip, to.ref_name());
        git.run(["push", "--quiet", to.remote(), refspec.as_str()])
            .map_err(|e| {
                format!(
                    "error: could not push {} to the nexus:\n{}",
                    to.ref_name(),
                    e.trim_start_matches("error: ")
                )
            })?;
        moved.push(format!(
            "copied {} {} to {} in the nexus",
            from.ref_name(),
            short(&tip),
            to.ref_name()
        ));
    }

    let outcome = point_at_nexus(
        &state_root,
        &nexus,
        &entry,
        &TerritoryChoice {
            id: territory,
            name: None,
        },
        &mut || {
            eprintln!("note: the nexus's default branch moved before the push; fetching again and retrying once")
        },
    )?;
    crate::ledger::refs::forget_cache(&repo, &state_root);
    crate::ledger::refs::memo_forget();

    for line in &moved {
        println!("{}", line);
    }
    match &outcome {
        Outcome::AlreadyRegistered { .. } => println!(
            "{} already says the entry \"{}\" holds its ledger in the nexus",
            NEXUS_FILE, entry.id
        ),
        Outcome::Registered { territory, .. } => println!(
            "set the entry \"{}\" in territory \"{}\" of {} to ledger nexus, ledger_refs {}",
            entry.id,
            territory,
            NEXUS_FILE,
            refs.as_str()
        ),
    }
    println!(
        "The old refs on origin are unchanged. When every clone has run tsk ledger fetch, delete them with:\n  git push origin --delete {} {}",
        source.ref_name(),
        source.docs_ref_name()
    );
    Ok(())
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

fn register_repo(
    config_file: &Path,
    id: Option<String>,
    territory: TerritoryChoice,
    ledger: LedgerChoice,
    ledger_refs: Option<String>,
    local: bool,
) -> Result<(), String> {
    let nexus = config::read_nexus_url(config_file)?
        .ok_or_else(|| "error: no nexus is attached; run tsk nexus add <url> first".to_string())?;
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = Repo::discover(&cwd)?;
    let prefix = ledger_prefix(&cwd)?;
    let entry = new_entry(&repo, id, ledger, ledger_refs, local, prefix)?;
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

/// Register this repo in the attached nexus for `tsk ledger init`, before its ledger
/// exists, with the mission prefix the new ledger takes. The outcome goes to stderr.
pub fn register_for_init(
    config_file: &Path,
    id: Option<String>,
    territory: Option<String>,
    ledger: LedgerChoice,
    prefix: String,
) -> Result<(), String> {
    let nexus = config::read_nexus_url(config_file)?
        .ok_or_else(|| "error: no nexus is attached; run tsk nexus add <url> first".to_string())?;
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = Repo::discover(&cwd)?;
    let entry = new_entry(&repo, id, ledger, None, false, prefix)?;
    let state_root = state_root_from_env()?;
    let territory = TerritoryChoice {
        id: territory,
        name: None,
    };
    let outcome = register(&state_root, &nexus, &entry, &territory, &mut || {
        eprintln!("note: the nexus's default branch moved before the push; fetching again and retrying once")
    })?;
    eprint!("{}", outcome_text(&outcome, &entry, &nexus));
    Ok(())
}

/// The mission prefix of this repo's ledger. A repo cannot register without one: the prefix
/// keeps mission IDs apart between the repos of a nexus.
fn ledger_prefix(cwd: &Path) -> Result<String, String> {
    let none = "run tsk mission prefix <PREFIX> first: it chooses 3 or 4 capital letters and checks that no other repo in the nexus uses them";
    match crate::ledger::store::open(cwd).and_then(|store| store.mission_prefix()) {
        Ok(Some(prefix)) => Ok(prefix),
        Ok(None) => Err(format!(
            "error: this repo's ledger has no mission prefix, and a repo cannot register in a nexus without one: {}",
            none
        )),
        Err(e) => Err(format!(
            "error: could not read the mission prefix of this repo's ledger ({}); a repo cannot register in a nexus without one: {}",
            e.trim_start_matches("error: "),
            none
        )),
    }
}

fn new_entry(
    repo: &Repo,
    id: Option<String>,
    ledger: LedgerChoice,
    ledger_refs: Option<String>,
    local: bool,
    prefix: String,
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
        ledger_refs,
        prefix: Some(prefix),
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
            created_file,
            commit,
            branch,
            ..
        } => {
            let mut text = String::new();
            if *created_file {
                text.push_str(&format!("created {} on {}\n", NEXUS_FILE, branch));
            }
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
            ledger_refs: None,
            prefix: None,
        }
    }

    #[test]
    fn registered_text_names_the_entry_the_push_and_the_next_step() {
        let outcome = Outcome::Registered {
            territory: "work".to_string(),
            created: true,
            created_file: false,
            commit: "abc123".to_string(),
            branch: "refs/heads/main".to_string(),
            attempts: 1,
        };
        assert_eq!(
            outcome_text(&outcome, &work_api(), "https://example.test/o/nexus"),
            "created territory work\n\
             registered in territory work: {\"id\":\"work-api\",\"url\":\"https://example.test/acme/work-api\",\"ledger\":\"nexus\"}\n\
             pushed abc123 to refs/heads/main of https://example.test/o/nexus\n\
             next: run tsk ledger fetch in this repo\n"
        );
    }

    #[test]
    fn registered_text_names_a_created_nexus_json_first() {
        let outcome = Outcome::Registered {
            territory: "work".to_string(),
            created: true,
            created_file: true,
            commit: "abc123".to_string(),
            branch: "refs/heads/main".to_string(),
            attempts: 1,
        };
        let text = outcome_text(&outcome, &work_api(), "n");
        assert!(
            text.starts_with("created nexus.json on refs/heads/main\ncreated territory work\n"),
            "{}",
            text
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
