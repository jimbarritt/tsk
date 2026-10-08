use std::path::Path;

use serde_json::{json, Map, Value};

use super::git::{failure_message, split_nul, Git};
use super::nexus::{normalise_url, open_clone, valid_repo_id, NEXUS_FILE, TRACKING_REF};
use super::push::parse_push_status;

pub const PUSH_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntry {
    pub id: String,
    pub url: Option<String>,
    pub local: Option<String>,
    pub ledger: String,
}

impl NewEntry {
    pub fn to_json(&self) -> Value {
        let mut entry = Map::new();
        entry.insert("id".to_string(), json!(self.id));
        if let Some(url) = &self.url {
            entry.insert("url".to_string(), json!(url));
        }
        if let Some(local) = &self.local {
            entry.insert("local".to_string(), json!(local));
        }
        entry.insert("ledger".to_string(), json!(self.ledger));
        Value::Object(entry)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerritoryChoice {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    AlreadyRegistered { territory: String, entry: Value },
    Added { territory: String, created: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    AlreadyRegistered {
        territory: String,
        entry: Value,
    },
    Registered {
        territory: String,
        created: bool,
        commit: String,
        branch: String,
        attempts: usize,
    },
}

pub fn derive_repo_id(name: &str) -> Option<String> {
    let mut id = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if !id.is_empty() && !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_end_matches('-').to_string();
    valid_repo_id(&id).then_some(id)
}

pub fn repo_name_from_url(url: &str) -> Option<String> {
    let normalised = normalise_url(url)?;
    normalised.rsplit('/').next().map(str::to_string)
}

fn same_url(a: &str, b: &str) -> bool {
    match (normalise_url(a), normalise_url(b)) {
        (Some(a), Some(b)) => a == b,
        _ => a.trim() == b.trim(),
    }
}

fn describe(entry: &Value) -> String {
    match (entry["url"].as_str(), entry["local"].as_str()) {
        (Some(url), _) => url.to_string(),
        (None, Some(local)) => format!("local: {}", local),
        (None, None) => "no url".to_string(),
    }
}

pub fn add_entry(
    doc: &mut Value,
    entry: &NewEntry,
    territory: &TerritoryChoice,
) -> Result<Edit, String> {
    let invalid = || {
        format!(
            "error: {} in the nexus is not a JSON object with a territories array",
            NEXUS_FILE
        )
    };
    let root = doc.as_object_mut().ok_or_else(invalid)?;
    let territories = root
        .entry("territories")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(invalid)?;

    for t in territories.iter() {
        let territory_id = t["id"].as_str().unwrap_or("").to_string();
        let Some(repos) = t.get("repos").and_then(Value::as_array) else {
            continue;
        };
        for existing in repos {
            let existing_id = existing["id"].as_str().unwrap_or("");
            let url_match = match (&entry.url, existing["url"].as_str()) {
                (Some(new), Some(old)) => same_url(new, old),
                _ => false,
            };
            let local_match = entry.url.is_none()
                && existing["url"].is_null()
                && entry.local.is_some()
                && existing["local"].as_str() == entry.local.as_deref();
            if url_match {
                if existing_id == entry.id {
                    return Ok(Edit::AlreadyRegistered {
                        territory: territory_id,
                        entry: existing.clone(),
                    });
                }
                return Err(format!(
                    "error: the nexus already lists {} under the id \"{}\" in territory \"{}\"; \
                     run with --id {} to match it, or remove that entry first",
                    entry.url.as_deref().unwrap_or(""),
                    existing_id,
                    territory_id,
                    existing_id
                ));
            }
            if existing_id == entry.id {
                if local_match {
                    return Ok(Edit::AlreadyRegistered {
                        territory: territory_id,
                        entry: existing.clone(),
                    });
                }
                return Err(format!(
                    "error: the nexus already has an entry with the id \"{}\" in territory \"{}\", for {}; \
                     choose another id with --id",
                    entry.id,
                    territory_id,
                    describe(existing)
                ));
            }
        }
    }

    let ids: Vec<String> = territories
        .iter()
        .map(|t| t["id"].as_str().unwrap_or("").to_string())
        .collect();
    let (index, created) = match (&territory.id, ids.len()) {
        (Some(id), _) => match ids.iter().position(|t| t == id) {
            Some(index) => (index, false),
            None if ids.is_empty() => {
                if !valid_repo_id(id) {
                    return Err(format!(
                        "error: \"{}\" is not a valid territory id; use lowercase letters, digits and hyphens, starting with a letter or digit",
                        id
                    ));
                }
                let name = territory.name.clone().unwrap_or_else(|| id.clone());
                territories.push(json!({"id": id, "name": name, "repos": []}));
                (0, true)
            }
            None => {
                return Err(format!(
                    "error: the nexus has no territory \"{}\"; its territories are: {}",
                    id,
                    ids.join(", ")
                ))
            }
        },
        (None, 1) => (0, false),
        (None, 0) => {
            return Err(
                "error: the nexus has no territories; pass --territory <id>, and --territory-name <name>, to create one"
                    .to_string(),
            )
        }
        (None, _) => {
            return Err(format!(
                "error: the nexus has {} territories ({}); pass --territory <id> to choose one",
                ids.len(),
                ids.join(", ")
            ))
        }
    };

    let chosen = territories[index].as_object_mut().ok_or_else(invalid)?;
    let repos = chosen
        .entry("repos")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(invalid)?;
    repos.push(entry.to_json());
    Ok(Edit::Added {
        territory: ids
            .get(index)
            .cloned()
            .or_else(|| territory.id.clone())
            .unwrap_or_default(),
        created,
    })
}

pub fn render(doc: &Value) -> Result<String, String> {
    let text = serde_json::to_string_pretty(doc)
        .map_err(|e| format!("error: could not serialise {}: {}", NEXUS_FILE, e))?;
    Ok(format!("{}\n", text))
}

pub fn parse_default_branch(ls_remote: &str) -> Option<String> {
    ls_remote.lines().find_map(|line| {
        let (target, name) = line.strip_prefix("ref: ")?.split_once('\t')?;
        (name == "HEAD" && target.starts_with("refs/heads/")).then(|| target.to_string())
    })
}

fn default_branch(git: &Git, nexus_url: &str) -> Result<String, String> {
    let listing = git.run(["ls-remote", "--symref", nexus_url, "HEAD"])?;
    parse_default_branch(&listing).ok_or_else(|| {
        format!(
            "error: could not read the default branch of the nexus {}; it has no HEAD branch",
            nexus_url
        )
    })
}

fn fetch_branch(git: &Git, nexus_url: &str, branch: &str) -> Result<String, String> {
    let refspec = format!("+{}:{}", branch, TRACKING_REF);
    git.run(["fetch", "--quiet", nexus_url, refspec.as_str()])
        .map_err(|e| {
            format!(
                "error: could not fetch the nexus {}:\n{}",
                nexus_url,
                e.trim_start_matches("error: ")
            )
        })?;
    git.run_line(["rev-parse", "--verify", TRACKING_REF])
}

fn commit_nexus_json(git: &Git, tip: &str, text: &str, message: &str) -> Result<String, String> {
    let blob = git.run_stdin(["hash-object", "-w", "--stdin"], text)?;
    let listing = git.run(["ls-tree", "-z", tip])?;
    let mut records: Vec<String> = split_nul(&listing)
        .into_iter()
        .filter(|record| record.split_once('\t').map(|(_, name)| name) != Some(NEXUS_FILE))
        .map(str::to_string)
        .collect();
    records.push(format!("100644 blob {}\t{}", blob, NEXUS_FILE));
    let input: String = records.iter().map(|r| format!("{}\0", r)).collect();
    let tree = git.run_stdin(["mktree", "-z"], &input)?;
    git.run_line(["commit-tree", tree.as_str(), "-p", tip, "-m", message])
}

fn push_commit(
    git: &Git,
    nexus_url: &str,
    branch: &str,
    commit: &str,
    expected: &str,
) -> Result<bool, String> {
    let lease = format!("--force-with-lease={}:{}", branch, expected);
    let refspec = format!("{}:{}", commit, branch);
    let (args, output) = git.output([
        "push",
        "--porcelain",
        lease.as_str(),
        nexus_url,
        refspec.as_str(),
    ])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let status =
        parse_push_status(&stdout, branch).ok_or_else(|| failure_message(&args, &output))?;
    if status.rejected() {
        return Ok(false);
    }
    if !output.status.success() {
        return Err(failure_message(&args, &output));
    }
    Ok(true)
}

pub fn register(
    state_root: &Path,
    nexus_url: &str,
    entry: &NewEntry,
    territory: &TerritoryChoice,
    on_retry: &mut dyn FnMut(),
) -> Result<Outcome, String> {
    let git = open_clone(state_root)?;
    let branch = default_branch(&git, nexus_url)?;
    for attempt in 1..=PUSH_ATTEMPTS {
        let tip = fetch_branch(&git, nexus_url, &branch)?;
        let object = format!("{}:{}", tip, NEXUS_FILE);
        let text = git.run(["show", object.as_str()]).map_err(|_| {
            format!(
                "error: the nexus {} has no {} on {}",
                nexus_url, NEXUS_FILE, branch
            )
        })?;
        let mut doc: Value = serde_json::from_str(&text)
            .map_err(|e| format!("error: {} in the nexus is not valid: {}", NEXUS_FILE, e))?;
        let (territory_id, created) = match add_entry(&mut doc, entry, territory)? {
            Edit::AlreadyRegistered { territory, entry } => {
                return Ok(Outcome::AlreadyRegistered { territory, entry })
            }
            Edit::Added { territory, created } => (territory, created),
        };
        let message = format!("Register {} in territory {}", entry.id, territory_id);
        let commit = commit_nexus_json(&git, &tip, &render(&doc)?, &message)?;
        if push_commit(&git, nexus_url, &branch, &commit, &tip)? {
            return Ok(Outcome::Registered {
                territory: territory_id,
                created,
                commit,
                branch,
                attempts: attempt,
            });
        }
        if attempt < PUSH_ATTEMPTS {
            on_retry();
        }
    }
    Err(format!(
        "error: the nexus's {} moved between each fetch and push, {} times; re-run tsk nexus register-repo",
        branch, PUSH_ATTEMPTS
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url_entry(id: &str, url: &str) -> NewEntry {
        NewEntry {
            id: id.to_string(),
            url: Some(url.to_string()),
            local: None,
            ledger: "nexus".to_string(),
        }
    }

    fn local_entry(id: &str, machine: &str) -> NewEntry {
        NewEntry {
            id: id.to_string(),
            url: None,
            local: Some(machine.to_string()),
            ledger: "nexus".to_string(),
        }
    }

    fn any_territory() -> TerritoryChoice {
        TerritoryChoice {
            id: None,
            name: None,
        }
    }

    fn territory(id: &str) -> TerritoryChoice {
        TerritoryChoice {
            id: Some(id.to_string()),
            name: None,
        }
    }

    fn one_territory() -> Value {
        serde_json::from_str(
            r#"{"version":1,"owner":"me","territories":[
              {"id":"work","name":"Work","colour":"blue","repos":[
                {"id":"billing","url":"git@example.test:acme/billing.git","ledger":"nexus","note":"keep"},
                {"id":"scratch","local":"laptop","ledger":"nexus"}
              ]}
            ]}"#,
        )
        .unwrap()
    }

    fn two_territories() -> Value {
        serde_json::from_str(
            r#"{"version":1,"territories":[{"id":"work","repos":[]},{"id":"home"}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn derive_repo_id_lowercases_and_hyphenates() {
        assert_eq!(derive_repo_id("Work-API").as_deref(), Some("work-api"));
        assert_eq!(derive_repo_id("my_repo.v2").as_deref(), Some("my-repo-v2"));
        assert_eq!(derive_repo_id("  a  b ").as_deref(), Some("a-b"));
        assert_eq!(derive_repo_id("__x__").as_deref(), Some("x"));
        assert_eq!(derive_repo_id("___"), None);
        assert_eq!(derive_repo_id("café"), Some("caf".to_string()));
    }

    #[test]
    fn repo_name_from_url_is_the_last_path_segment() {
        assert_eq!(
            repo_name_from_url("git@github.com:Acme/Work-API.git").as_deref(),
            Some("work-api")
        );
        assert_eq!(
            repo_name_from_url("https://gitlab.com/a/b/c").as_deref(),
            Some("c")
        );
        assert_eq!(repo_name_from_url("/tmp/origin.git"), None);
    }

    #[test]
    fn adds_to_the_only_territory_and_keeps_every_other_field() {
        let mut doc = one_territory();
        let edit = add_entry(
            &mut doc,
            &url_entry("work-api", "https://example.test/acme/work-api"),
            &any_territory(),
        )
        .unwrap();
        assert_eq!(
            edit,
            Edit::Added {
                territory: "work".to_string(),
                created: false
            }
        );
        assert_eq!(doc["owner"], "me");
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["territories"][0]["colour"], "blue");
        assert_eq!(doc["territories"][0]["repos"][0]["note"], "keep");
        assert_eq!(
            doc["territories"][0]["repos"][2],
            json!({"id": "work-api", "url": "https://example.test/acme/work-api", "ledger": "nexus"})
        );
    }

    #[test]
    fn the_same_url_and_id_is_already_registered() {
        let mut doc = one_territory();
        let before = doc.clone();
        let edit = add_entry(
            &mut doc,
            &url_entry("billing", "https://example.test/Acme/billing"),
            &territory("elsewhere"),
        )
        .unwrap();
        assert!(
            matches!(edit, Edit::AlreadyRegistered { ref territory, .. } if territory == "work")
        );
        assert_eq!(doc, before);
    }

    #[test]
    fn the_same_url_under_another_id_is_an_error() {
        let mut doc = one_territory();
        let err = add_entry(
            &mut doc,
            &url_entry("billing-2", "https://example.test/acme/billing"),
            &any_territory(),
        )
        .unwrap_err();
        assert!(err.contains("under the id \"billing\""), "{}", err);
    }

    #[test]
    fn the_same_id_with_another_url_is_an_error() {
        let mut doc = one_territory();
        let err = add_entry(
            &mut doc,
            &url_entry("billing", "https://example.test/acme/other"),
            &any_territory(),
        )
        .unwrap_err();
        assert!(err.contains("entry with the id \"billing\""), "{}", err);
        assert!(err.contains("git@example.test:acme/billing.git"), "{}", err);
    }

    #[test]
    fn a_local_entry_matches_on_id_and_machine() {
        let mut doc = one_territory();
        let edit = add_entry(
            &mut doc,
            &local_entry("scratch", "laptop"),
            &any_territory(),
        )
        .unwrap();
        assert!(matches!(edit, Edit::AlreadyRegistered { .. }));
        let err = add_entry(
            &mut doc,
            &local_entry("scratch", "desktop"),
            &any_territory(),
        )
        .unwrap_err();
        assert!(err.contains("local: laptop"), "{}", err);
        let added = add_entry(&mut doc, &local_entry("notes", "laptop"), &any_territory()).unwrap();
        assert!(matches!(added, Edit::Added { .. }));
        assert_eq!(
            doc["territories"][0]["repos"][2],
            json!({"id": "notes", "local": "laptop", "ledger": "nexus"})
        );
    }

    #[test]
    fn more_than_one_territory_needs_a_choice() {
        let mut doc = two_territories();
        let entry = url_entry("x", "https://example.test/o/x");
        let err = add_entry(&mut doc, &entry, &any_territory()).unwrap_err();
        assert!(err.contains("2 territories (work, home)"), "{}", err);
        let err = add_entry(&mut doc, &entry, &territory("play")).unwrap_err();
        assert!(err.contains("no territory \"play\""), "{}", err);
        let edit = add_entry(&mut doc, &entry, &territory("home")).unwrap();
        assert_eq!(
            edit,
            Edit::Added {
                territory: "home".to_string(),
                created: false
            }
        );
        assert_eq!(doc["territories"][1]["repos"][0]["id"], "x");
    }

    #[test]
    fn no_territories_needs_one_named_and_creates_it() {
        let mut doc = json!({"version": 1, "territories": []});
        let entry = url_entry("x", "https://example.test/o/x");
        let err = add_entry(&mut doc, &entry, &any_territory()).unwrap_err();
        assert!(err.contains("has no territories"), "{}", err);
        let err = add_entry(&mut doc, &entry, &territory("Bad Id")).unwrap_err();
        assert!(err.contains("not a valid territory id"), "{}", err);
        let edit = add_entry(
            &mut doc,
            &entry,
            &TerritoryChoice {
                id: Some("work".to_string()),
                name: Some("Work".to_string()),
            },
        )
        .unwrap();
        assert_eq!(
            edit,
            Edit::Added {
                territory: "work".to_string(),
                created: true
            }
        );
        assert_eq!(doc["territories"][0]["name"], "Work");
        assert_eq!(doc["territories"][0]["repos"][0]["id"], "x");
    }

    #[test]
    fn a_missing_territories_key_counts_as_none() {
        let mut doc = json!({"version": 1});
        let edit = add_entry(
            &mut doc,
            &url_entry("x", "https://e.test/o/x"),
            &territory("t"),
        )
        .unwrap();
        assert!(matches!(edit, Edit::Added { created: true, .. }));
        assert_eq!(doc["territories"][0]["name"], "t");
    }

    #[test]
    fn a_document_that_is_not_an_object_is_an_error() {
        let mut doc = json!([1, 2]);
        assert!(add_entry(
            &mut doc,
            &url_entry("x", "https://e.test/o/x"),
            &any_territory()
        )
        .unwrap_err()
        .contains("not a JSON object"));
    }

    #[test]
    fn render_is_pretty_with_a_trailing_newline() {
        let text = render(&json!({"version": 1})).unwrap();
        assert_eq!(text, "{\n  \"version\": 1\n}\n");
    }

    #[test]
    fn parses_the_default_branch_from_ls_remote() {
        let listing =
            "ref: refs/heads/main\tHEAD\n1111111111111111111111111111111111111111\tHEAD\n";
        assert_eq!(
            parse_default_branch(listing).as_deref(),
            Some("refs/heads/main")
        );
        assert_eq!(parse_default_branch("1111\tHEAD\n"), None);
        assert_eq!(parse_default_branch(""), None);
    }
}
