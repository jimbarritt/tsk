use std::path::Path;

use serde_json::{json, Value};

use super::nexus::{normalise_url, valid_repo_id, NEXUS_FILE};
use super::register::{prefix_holder, prefix_taken, same_url, update_nexus, Edit, TerritoryChoice};
use crate::mission::prefix::valid_prefix;

pub const WORKFLOWS: [&str; 2] = ["main", "pull-request"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Done(String),
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    Unchanged,
    Changed {
        summary: String,
        commit: String,
        branch: String,
    },
}

impl Published {
    pub fn text(&self, nexus_url: &str) -> String {
        match self {
            Published::Unchanged => "unchanged\n".to_string(),
            Published::Changed {
                summary,
                commit,
                branch,
            } => format!(
                "{}\npushed {} to {} of {}\n",
                summary, commit, branch, nexus_url
            ),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoFields {
    pub workflow: Option<String>,
    pub prefix: Option<String>,
    pub url: Option<String>,
}

fn invalid() -> String {
    format!(
        "error: {} in the nexus is not a JSON object with a territories array",
        NEXUS_FILE
    )
}

fn territories_mut(doc: &mut Value) -> Result<&mut Vec<Value>, String> {
    doc.as_object_mut()
        .ok_or_else(invalid)?
        .entry("territories")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(invalid)
}

fn territory_ids(territories: &[Value]) -> Vec<String> {
    territories
        .iter()
        .map(|t| t["id"].as_str().unwrap_or("").to_string())
        .collect()
}

fn find_territory(territories: &[Value], id: &str) -> Result<usize, String> {
    territories
        .iter()
        .position(|t| t["id"].as_str() == Some(id))
        .ok_or_else(|| {
            format!(
                "error: the nexus has no territory \"{}\"; its territories are: {}",
                id,
                territory_ids(territories).join(", ")
            )
        })
}

fn find_repo(territories: &[Value], repo_id: &str) -> Result<(usize, usize), String> {
    for (t, territory) in territories.iter().enumerate() {
        let Some(repos) = territory.get("repos").and_then(Value::as_array) else {
            continue;
        };
        if let Some(r) = repos.iter().position(|e| e["id"].as_str() == Some(repo_id)) {
            return Ok((t, r));
        }
    }
    Err(format!(
        "error: the nexus has no repo \"{}\" in {}",
        repo_id, NEXUS_FILE
    ))
}

fn repos_mut(territory: &mut Value) -> Result<&mut Vec<Value>, String> {
    territory
        .as_object_mut()
        .ok_or_else(invalid)?
        .entry("repos")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(invalid)
}

fn territory_label(territory: &Value) -> String {
    territory["id"].as_str().unwrap_or("").to_string()
}

pub fn add_territory(doc: &mut Value, id: &str, name: Option<&str>) -> Result<Change, String> {
    if !valid_repo_id(id) {
        return Err(format!(
            "error: \"{}\" is not a valid territory id; use lowercase letters, digits and hyphens, starting with a letter or digit",
            id
        ));
    }
    let territories = territories_mut(doc)?;
    if territories.iter().any(|t| t["id"].as_str() == Some(id)) {
        return Ok(Change::Unchanged);
    }
    let name = name.unwrap_or(id);
    territories.push(json!({"id": id, "name": name, "repos": []}));
    Ok(Change::Done(format!(
        "added territory {} named \"{}\"",
        id, name
    )))
}

pub fn rename_territory(doc: &mut Value, id: &str, name: &str) -> Result<Change, String> {
    let territories = territories_mut(doc)?;
    let index = find_territory(territories, id)?;
    let object = territories[index].as_object_mut().ok_or_else(invalid)?;
    let old = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string);
    if old.as_deref() == Some(name) {
        return Ok(Change::Unchanged);
    }
    object.insert("name".to_string(), json!(name));
    Ok(Change::Done(format!(
        "renamed territory {} from \"{}\" to \"{}\"",
        id,
        old.unwrap_or_default(),
        name
    )))
}

pub fn remove_territory(doc: &mut Value, id: &str) -> Result<Change, String> {
    let territories = territories_mut(doc)?;
    let index = find_territory(territories, id)?;
    let held = territories[index]
        .get("repos")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if held > 0 {
        return Err(format!(
            "error: the territory \"{}\" holds {} {}; move or remove them first",
            id,
            held,
            if held == 1 { "repo" } else { "repos" }
        ));
    }
    territories.remove(index);
    Ok(Change::Done(format!("removed territory {}", id)))
}

pub fn set_repo(doc: &mut Value, repo_id: &str, fields: &RepoFields) -> Result<Change, String> {
    if fields == &RepoFields::default() {
        return Err(
            "error: pass at least one of --workflow, --prefix and --url to set on the repo"
                .to_string(),
        );
    }
    if let Some(workflow) = fields.workflow.as_deref() {
        if !WORKFLOWS.contains(&workflow) {
            return Err(format!(
                "error: \"{}\" is not a workflow; the values are main and pull-request",
                workflow
            ));
        }
    }
    if let Some(prefix) = fields.prefix.as_deref() {
        if !valid_prefix(prefix) {
            return Err(format!(
                "error: '{}' is not a mission prefix; a prefix is 3 or 4 capital letters, for example TSK",
                prefix
            ));
        }
    }
    if let Some(url) = fields.url.as_deref() {
        if normalise_url(url).is_none() {
            return Err(format!(
                "error: \"{}\" is not a host URL that other machines can match",
                url
            ));
        }
    }
    let (t, r) = find_repo(territories_mut(doc)?, repo_id)?;
    if let Some(prefix) = fields.prefix.as_deref() {
        if let Some(holder) = prefix_holder(doc, prefix, repo_id) {
            return Err(prefix_taken(prefix, &holder));
        }
    }
    if let Some(url) = fields.url.as_deref() {
        let territories = territories_mut(doc)?;
        for (ti, territory) in territories.iter().enumerate() {
            let repos = territory.get("repos").and_then(Value::as_array);
            for (ri, other) in repos.into_iter().flatten().enumerate() {
                let uses = other["url"]
                    .as_str()
                    .is_some_and(|held| same_url(held, url));
                if uses && (ti, ri) != (t, r) {
                    return Err(format!(
                        "error: the repo \"{}\" in the nexus already uses the URL {}",
                        other["id"].as_str().unwrap_or(""),
                        url
                    ));
                }
            }
        }
    }
    let entry = territories_mut(doc)?[t]
        .get_mut("repos")
        .and_then(Value::as_array_mut)
        .and_then(|repos| repos.get_mut(r))
        .and_then(Value::as_object_mut)
        .ok_or_else(invalid)?;
    let mut parts = Vec::new();
    let mut set = |key: &str, value: &Option<String>| {
        if let Some(value) = value {
            if entry.get(key).and_then(Value::as_str) != Some(value.as_str()) {
                entry.insert(key.to_string(), json!(value));
                parts.push(format!("{} {}", key, value));
            }
        }
    };
    set("workflow", &fields.workflow);
    set("prefix", &fields.prefix);
    set("url", &fields.url);
    if parts.is_empty() {
        return Ok(Change::Unchanged);
    }
    Ok(Change::Done(format!(
        "set {}: {}",
        repo_id,
        parts.join(", ")
    )))
}

pub fn move_repo(doc: &mut Value, repo_id: &str, territory_id: &str) -> Result<Change, String> {
    let territories = territories_mut(doc)?;
    let target = find_territory(territories, territory_id)?;
    let (from, at) = find_repo(territories, repo_id)?;
    if from == target {
        return Ok(Change::Unchanged);
    }
    let source_id = territory_label(&territories[from]);
    let entry = repos_mut(&mut territories[from])?.remove(at);
    repos_mut(&mut territories[target])?.push(entry);
    Ok(Change::Done(format!(
        "moved {} from territory {} to territory {}",
        repo_id, source_id, territory_id
    )))
}

pub fn remove_repo(doc: &mut Value, repo_id: &str, force: bool) -> Result<Change, String> {
    let territories = territories_mut(doc)?;
    let (from, at) = find_repo(territories, repo_id)?;
    let holds_ledger = territories[from]["repos"][at]["ledger"].as_str() == Some("nexus");
    if holds_ledger && !force {
        return Err(format!(
            "error: the entry \"{}\" holds its ledger in the nexus. The ledger branches stay in the nexus, and removing the entry hides them from every clone; run with --force to remove it anyway",
            repo_id
        ));
    }
    let source_id = territory_label(&territories[from]);
    repos_mut(&mut territories[from])?.remove(at);
    Ok(Change::Done(format!(
        "removed {} from territory {}",
        repo_id, source_id
    )))
}

pub fn publish(
    state_root: &Path,
    nexus_url: &str,
    command: &str,
    commit_message: &str,
    create_file: bool,
    on_retry: &mut dyn FnMut(),
    edit: &mut dyn FnMut(&mut Value) -> Result<Change, String>,
) -> Result<Published, String> {
    let mut summary = String::new();
    let choice = TerritoryChoice {
        id: create_file.then(|| "new".to_string()),
        name: None,
    };
    let outcome = update_nexus(
        state_root,
        nexus_url,
        &choice,
        command,
        on_retry,
        &mut |doc| match edit(doc)? {
            Change::Done(text) => {
                summary = text;
                Ok(Edit::Added {
                    territory: String::new(),
                    created: false,
                })
            }
            Change::Unchanged => Ok(Edit::AlreadyRegistered {
                territory: String::new(),
                entry: Value::Null,
            }),
        },
        &|_, _| commit_message.to_string(),
    )?;
    Ok(match outcome {
        super::register::Outcome::AlreadyRegistered { .. } => Published::Unchanged,
        super::register::Outcome::Registered { commit, branch, .. } => Published::Changed {
            summary,
            commit,
            branch,
        },
    })
}

#[allow(dead_code)]
pub fn set_repo_workflow(
    state_root: &Path,
    nexus_url: &str,
    repo_id: &str,
    workflow: &str,
    on_retry: &mut dyn FnMut(),
) -> Result<Published, String> {
    let fields = RepoFields {
        workflow: Some(workflow.to_string()),
        ..RepoFields::default()
    };
    publish(
        state_root,
        nexus_url,
        "tsk nexus set-repo",
        &format!("Set fields of {}", repo_id),
        false,
        on_retry,
        &mut |doc| set_repo(doc, repo_id, &fields),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Value {
        serde_json::from_str(
            r#"{"version":1,"territories":[
              {"id":"work","name":"Work","repos":[
                {"id":"billing","url":"https://example.test/acme/billing","ledger":"nexus","prefix":"BIL"},
                {"id":"scratch","local":"laptop","prefix":"SCR"}
              ]},
              {"id":"home","name":"Home","repos":[]}
            ]}"#,
        )
        .unwrap()
    }

    fn done(change: Result<Change, String>) -> String {
        match change.unwrap() {
            Change::Done(text) => text,
            Change::Unchanged => panic!("unchanged"),
        }
    }

    fn fields(workflow: Option<&str>, prefix: Option<&str>, url: Option<&str>) -> RepoFields {
        RepoFields {
            workflow: workflow.map(str::to_string),
            prefix: prefix.map(str::to_string),
            url: url.map(str::to_string),
        }
    }

    #[test]
    fn add_territory_appends_one_with_the_id_as_its_default_name() {
        let mut doc = doc();
        assert_eq!(
            done(add_territory(&mut doc, "play", None)),
            "added territory play named \"play\""
        );
        assert_eq!(
            doc["territories"][2],
            json!({"id": "play", "name": "play", "repos": []})
        );
        done(add_territory(&mut doc, "lab", Some("The Lab")));
        assert_eq!(doc["territories"][3]["name"], "The Lab");
    }

    #[test]
    fn add_territory_twice_is_unchanged_and_keeps_the_stored_name() {
        let mut doc = doc();
        let before = doc.clone();
        assert_eq!(
            add_territory(&mut doc, "work", Some("Other")).unwrap(),
            Change::Unchanged
        );
        assert_eq!(doc, before);
    }

    #[test]
    fn add_territory_refuses_an_invalid_id() {
        assert!(add_territory(&mut doc(), "Bad Id", None)
            .unwrap_err()
            .contains("not a valid territory id"));
    }

    #[test]
    fn add_territory_creates_the_array_in_an_empty_document() {
        let mut doc = json!({"version": 1});
        done(add_territory(&mut doc, "work", None));
        assert_eq!(doc["territories"][0]["id"], "work");
    }

    #[test]
    fn rename_territory_changes_the_name_only() {
        let mut doc = doc();
        assert_eq!(
            done(rename_territory(&mut doc, "work", "Team")),
            "renamed territory work from \"Work\" to \"Team\""
        );
        assert_eq!(doc["territories"][0]["name"], "Team");
        assert_eq!(doc["territories"][0]["repos"][0]["id"], "billing");
    }

    #[test]
    fn rename_territory_to_the_same_name_is_unchanged() {
        assert_eq!(
            rename_territory(&mut doc(), "work", "Work").unwrap(),
            Change::Unchanged
        );
    }

    #[test]
    fn rename_territory_refuses_an_unknown_id_and_lists_the_known() {
        let error = rename_territory(&mut doc(), "nope", "X").unwrap_err();
        assert!(error.contains("no territory \"nope\""), "{}", error);
        assert!(error.contains("work, home"), "{}", error);
    }

    #[test]
    fn remove_territory_drops_an_empty_one() {
        let mut doc = doc();
        assert_eq!(
            done(remove_territory(&mut doc, "home")),
            "removed territory home"
        );
        assert_eq!(doc["territories"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn remove_territory_refuses_one_with_repos_and_names_the_count() {
        let mut doc = doc();
        let before = doc.clone();
        let error = remove_territory(&mut doc, "work").unwrap_err();
        assert!(error.contains("holds 2 repos"), "{}", error);
        assert_eq!(doc, before);
    }

    #[test]
    fn remove_territory_refuses_an_unknown_id() {
        assert!(remove_territory(&mut doc(), "nope").is_err());
    }

    #[test]
    fn set_repo_writes_the_fields_and_keeps_the_others() {
        let mut doc = doc();
        let text = done(set_repo(
            &mut doc,
            "billing",
            &fields(Some("pull-request"), Some("BILL"), None),
        ));
        assert_eq!(text, "set billing: workflow pull-request, prefix BILL");
        assert_eq!(
            doc["territories"][0]["repos"][0],
            json!({
                "id": "billing",
                "url": "https://example.test/acme/billing",
                "ledger": "nexus",
                "prefix": "BILL",
                "workflow": "pull-request"
            })
        );
        let keys: Vec<&String> = doc["territories"][0]["repos"][0]
            .as_object()
            .unwrap()
            .keys()
            .collect();
        assert_eq!(keys, ["id", "url", "ledger", "prefix", "workflow"]);
    }

    #[test]
    fn set_repo_sets_the_url_of_a_local_entry() {
        let mut doc = doc();
        done(set_repo(
            &mut doc,
            "scratch",
            &fields(None, None, Some("git@example.test:acme/scratch.git")),
        ));
        assert_eq!(
            doc["territories"][0]["repos"][1]["url"],
            "git@example.test:acme/scratch.git"
        );
    }

    #[test]
    fn set_repo_with_the_stored_values_is_unchanged() {
        let mut doc = doc();
        let same = fields(None, Some("BIL"), Some("https://example.test/acme/billing"));
        assert_eq!(
            set_repo(&mut doc, "billing", &same).unwrap(),
            Change::Unchanged
        );
        done(set_repo(
            &mut doc,
            "billing",
            &fields(Some("main"), None, None),
        ));
        assert_eq!(
            set_repo(&mut doc, "billing", &fields(Some("main"), None, None)).unwrap(),
            Change::Unchanged
        );
    }

    #[test]
    fn set_repo_reports_only_the_fields_that_changed() {
        let mut doc = doc();
        let text = done(set_repo(
            &mut doc,
            "billing",
            &fields(Some("main"), Some("BIL"), None),
        ));
        assert_eq!(text, "set billing: workflow main");
    }

    #[test]
    fn set_repo_needs_an_option() {
        assert!(set_repo(&mut doc(), "billing", &RepoFields::default())
            .unwrap_err()
            .contains("at least one"));
    }

    #[test]
    fn set_repo_refuses_an_unknown_repo() {
        assert!(
            set_repo(&mut doc(), "nope", &fields(Some("main"), None, None))
                .unwrap_err()
                .contains("no repo \"nope\"")
        );
    }

    #[test]
    fn set_repo_refuses_a_workflow_that_is_not_main_or_pull_request() {
        assert!(
            set_repo(&mut doc(), "billing", &fields(Some("trunk"), None, None))
                .unwrap_err()
                .contains("main and pull-request")
        );
    }

    #[test]
    fn set_repo_refuses_a_malformed_prefix() {
        for bad in ["AB", "ABCDE", "abc", "AB1"] {
            assert!(
                set_repo(&mut doc(), "billing", &fields(None, Some(bad), None))
                    .unwrap_err()
                    .contains("3 or 4 capital letters"),
                "{}",
                bad
            );
        }
    }

    #[test]
    fn set_repo_refuses_a_prefix_another_repo_uses() {
        let mut doc = doc();
        let before = doc.clone();
        let error = set_repo(&mut doc, "billing", &fields(None, Some("SCR"), None)).unwrap_err();
        assert!(
            error.contains("already used by the repo \"scratch\""),
            "{}",
            error
        );
        assert_eq!(doc, before);
    }

    #[test]
    fn set_repo_refuses_a_url_another_repo_uses() {
        let error = set_repo(
            &mut doc(),
            "scratch",
            &fields(None, None, Some("git@example.test:Acme/Billing.git")),
        )
        .unwrap_err();
        assert!(error.contains("already uses the URL"), "{}", error);
    }

    #[test]
    fn set_repo_refuses_a_url_that_cannot_be_matched() {
        assert!(set_repo(
            &mut doc(),
            "billing",
            &fields(None, None, Some("/tmp/origin.git"))
        )
        .is_err());
    }

    #[test]
    fn move_repo_moves_the_entry_to_the_end_of_the_target() {
        let mut doc = doc();
        assert_eq!(
            done(move_repo(&mut doc, "billing", "home")),
            "moved billing from territory work to territory home"
        );
        assert_eq!(doc["territories"][0]["repos"].as_array().unwrap().len(), 1);
        assert_eq!(doc["territories"][1]["repos"][0]["id"], "billing");
        assert_eq!(doc["territories"][1]["repos"][0]["prefix"], "BIL");
    }

    #[test]
    fn move_repo_to_its_own_territory_is_unchanged() {
        assert_eq!(
            move_repo(&mut doc(), "billing", "work").unwrap(),
            Change::Unchanged
        );
    }

    #[test]
    fn move_repo_refuses_an_unknown_territory_or_repo() {
        assert!(move_repo(&mut doc(), "billing", "nope")
            .unwrap_err()
            .contains("no territory \"nope\""));
        assert!(move_repo(&mut doc(), "nope", "home")
            .unwrap_err()
            .contains("no repo \"nope\""));
    }

    #[test]
    fn remove_repo_drops_an_entry_that_keeps_its_ledger_in_the_repo() {
        let mut doc = doc();
        assert_eq!(
            done(remove_repo(&mut doc, "scratch", false)),
            "removed scratch from territory work"
        );
        assert_eq!(doc["territories"][0]["repos"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn remove_repo_refuses_a_nexus_ledger_without_force() {
        let mut doc = doc();
        let before = doc.clone();
        let error = remove_repo(&mut doc, "billing", false).unwrap_err();
        assert!(
            error.contains("ledger branches stay in the nexus"),
            "{}",
            error
        );
        assert!(error.contains("hides them"), "{}", error);
        assert_eq!(doc, before);
    }

    #[test]
    fn remove_repo_with_force_drops_a_nexus_ledger_entry() {
        let mut doc = doc();
        done(remove_repo(&mut doc, "billing", true));
        assert_eq!(doc["territories"][0]["repos"][0]["id"], "scratch");
    }

    #[test]
    fn remove_repo_refuses_an_unknown_repo() {
        assert!(remove_repo(&mut doc(), "nope", false).is_err());
    }

    #[test]
    fn published_text_is_one_line_for_unchanged_and_two_for_a_change() {
        assert_eq!(Published::Unchanged.text("n"), "unchanged\n");
        let changed = Published::Changed {
            summary: "removed territory home".to_string(),
            commit: "abc".to_string(),
            branch: "refs/heads/main".to_string(),
        };
        assert_eq!(
            changed.text("n"),
            "removed territory home\npushed abc to refs/heads/main of n\n"
        );
    }
}
