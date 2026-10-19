use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::git::Git;
use super::location::{LedgerLocation, Repo};
use super::refs;
use crate::config;

pub const NEXUS_FILE: &str = "nexus.json";
pub const NEXUS_DIR_NAME: &str = "nexus";
pub const MACHINE_VAR: &str = "TSK_MACHINE_NAME";
pub const TRACKING_REF: &str = "refs/heads/nexus";

#[derive(Debug, Deserialize)]
pub struct Entry {
    pub id: String,
    pub url: Option<String>,
    pub local: Option<String>,
    pub ledger: Option<String>,
    /// `branches` or `custom`: which refs hold this repo's ledger. Absent means the repo
    /// config and discovery decide.
    #[serde(default)]
    pub ledger_refs: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Territory {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub repos: Vec<Entry>,
}

#[derive(Debug, Default, Deserialize)]
pub struct NexusIndex {
    #[serde(default)]
    territories: Vec<Territory>,
}

impl NexusIndex {
    pub fn parse(text: &str) -> Result<NexusIndex, String> {
        serde_json::from_str(text)
            .map_err(|e| format!("error: {} in the nexus is not valid: {}", NEXUS_FILE, e))
    }

    pub fn territories(&self) -> &[Territory] {
        &self.territories
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.territories.iter().flat_map(|t| t.repos.iter())
    }
}

pub fn valid_repo_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn normalise_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let (authority, path) = match raw.split_once("://") {
        Some((scheme, rest)) => {
            if scheme.eq_ignore_ascii_case("file") {
                return None;
            }
            let (authority, path) = rest.split_once('/')?;
            (strip_port(strip_user(authority)), path)
        }
        None => {
            let (host, path) = raw.split_once(':')?;
            if host.is_empty() || host.contains('/') {
                return None;
            }
            (strip_user(host), path)
        }
    };
    if authority.is_empty() {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path).trim_matches('/');
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 2 || segments.iter().any(|s| s.is_empty()) {
        return None;
    }
    Some(format!("{}/{}", authority, segments.join("/")).to_lowercase())
}

fn strip_user(authority: &str) -> &str {
    authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host)
}

fn strip_port(authority: &str) -> &str {
    match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => authority,
    }
}

pub fn select_entry<'a>(
    index: &'a NexusIndex,
    cached_id: Option<&str>,
    origin_url: Option<&str>,
    machine: Option<&str>,
) -> Option<&'a Entry> {
    let visible = |entry: &&Entry| match entry.local.as_deref() {
        None => true,
        Some(local) => machine == Some(local),
    };
    if let Some(id) = cached_id {
        if let Some(entry) = index.entries().filter(visible).find(|e| e.id == id) {
            return Some(entry);
        }
    }
    let normalised = origin_url.and_then(normalise_url)?;
    index.entries().filter(visible).find(|e| {
        e.url
            .as_deref()
            .and_then(normalise_url)
            .is_some_and(|url| url == normalised)
    })
}

pub fn entry_location(entry: &Entry, nexus_url: &str) -> Result<LedgerLocation, String> {
    match entry.ledger.as_deref() {
        None | Some("repo") => Ok(LedgerLocation::in_repo()),
        Some("nexus") => {
            if !valid_repo_id(&entry.id) {
                return Err(format!(
                    "error: the nexus entry id \"{}\" is not a valid repo id; use lowercase letters, digits and hyphens, starting with a letter or digit",
                    entry.id
                ));
            }
            Ok(LedgerLocation::nexus(nexus_url, &entry.id))
        }
        Some(other) => Err(format!(
            "error: the nexus entry \"{}\" has ledger \"{}\"; the values are \"repo\" and \"nexus\"",
            entry.id, other
        )),
    }
}

pub fn machine_name() -> Option<String> {
    if let Some(name) = std::env::var(MACHINE_VAR).ok().filter(|v| !v.is_empty()) {
        return Some(name);
    }
    let output = std::process::Command::new("hostname").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

pub fn nexus_clone_path(state_root: &Path) -> PathBuf {
    state_root.join(NEXUS_DIR_NAME)
}

pub fn open_clone(state_root: &Path) -> Result<Git, String> {
    let dir = nexus_clone_path(state_root);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
    let git = Git::new(&dir);
    if !dir.join("HEAD").is_file() {
        git.run(["init", "--quiet", "--bare"])?;
    }
    Ok(git)
}

pub fn load_index(state_root: &Path, nexus_url: &str) -> Result<NexusIndex, String> {
    let git = open_clone(state_root)?;
    let dir = git.dir().to_path_buf();
    let refspec = format!("+HEAD:{}", TRACKING_REF);
    if let Err(fetch_error) = git.run(["fetch", "--quiet", nexus_url, refspec.as_str()]) {
        let held = git
            .run_line(["rev-parse", "--verify", "--quiet", TRACKING_REF])
            .is_ok();
        if !held {
            return Err(format!(
                "error: could not fetch the nexus {}:\n{}",
                nexus_url,
                fetch_error.trim_start_matches("error: ")
            ));
        }
        eprintln!(
            "note: could not fetch the nexus {}; reading the copy held in {}",
            nexus_url,
            dir.display()
        );
    }
    let object = format!("{}:{}", TRACKING_REF, NEXUS_FILE);
    let text = git.run(["show", object.as_str()]).map_err(|_| {
        format!(
            "error: the nexus {} has no {} on its default branch",
            nexus_url, NEXUS_FILE
        )
    })?;
    NexusIndex::parse(&text)
}

/// Where a repo's ledger lives, before the refs mode is decided: the remote and the repo
/// id (the location on branches), and the `ledger_refs` setting of the nexus entry.
pub struct Placement {
    pub base: LedgerLocation,
    pub entry_refs: Option<refs::EntrySetting>,
}

pub fn place(
    repo: &Repo,
    state_root: &Path,
    config_file: Option<&Path>,
) -> Result<Placement, String> {
    let in_repo = || Placement {
        base: LedgerLocation::in_repo(),
        entry_refs: None,
    };
    let nexus_url = match config_file {
        Some(file) => config::read_nexus_url(file)?,
        None => None,
    };
    let Some(nexus_url) = nexus_url else {
        return Ok(in_repo());
    };
    let cached = repo.read_repo_id()?;
    let origin = repo.raw_origin_url()?;
    let index = load_index(state_root, &nexus_url)?;
    let machine = machine_name();
    let Some(entry) = select_entry(
        &index,
        cached.as_deref(),
        origin.as_deref(),
        machine.as_deref(),
    ) else {
        return Ok(in_repo());
    };
    let base = entry_location(entry, &nexus_url)?;
    if valid_repo_id(&entry.id) && cached.as_deref() != Some(entry.id.as_str()) {
        repo.write_repo_id(&entry.id)?;
    }
    Ok(Placement {
        base,
        entry_refs: entry
            .ledger_refs
            .as_ref()
            .map(|value| (entry.id.clone(), value.clone())),
    })
}

/// The refs mode in force for a repo, and where it came from, with no check against an
/// existing ledger.
pub fn choose(
    repo: &Repo,
    state_root: &Path,
    config_file: Option<&Path>,
) -> Result<(Placement, refs::RefsChoice), String> {
    let placement = place(repo, state_root, config_file)?;
    let choice = match refs::configured(repo, placement.entry_refs.as_ref())? {
        Some(choice) => choice,
        None => refs::discover(repo, state_root, &placement.base),
    };
    Ok((placement, choice))
}

pub fn resolve(
    repo: &Repo,
    state_root: &Path,
    config_file: Option<&Path>,
) -> Result<LedgerLocation, String> {
    let (placement, choice) = choose(repo, state_root, config_file)?;
    let location = placement.base.with_refs(choice.refs, choice.source);
    refs::check_store(repo, state_root, &location)?;
    Ok(location)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> NexusIndex {
        NexusIndex::parse(
            r#"{"version":1,"territories":[
              {"id":"a","name":"A","repos":[
                {"id":"tsk","url":"https://github.com/jimbarritt/tsk"},
                {"id":"work-api","url":"git@github.com:Acme/Work-API.git","ledger":"nexus"},
                {"id":"scratch","local":"laptop","ledger":"nexus"},
                {"id":"other-scratch","local":"desktop","ledger":"nexus"}
              ]},
              {"id":"b","name":"B"}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn normalises_every_documented_form_to_one_name() {
        for raw in [
            "https://github.com/jimbarritt/tsk.git",
            "https://jim@github.com/jimbarritt/tsk",
            "ssh://git@github.com/jimbarritt/tsk.git",
            "ssh://git@github.com:22/jimbarritt/tsk.git",
            "git@github.com:jimbarritt/tsk.git",
            "HTTPS://GitHub.com/JimBarritt/TSK/",
        ] {
            assert_eq!(
                normalise_url(raw).as_deref(),
                Some("github.com/jimbarritt/tsk"),
                "{}",
                raw
            );
        }
    }

    #[test]
    fn keeps_every_segment_of_a_subgroup_path() {
        assert_eq!(
            normalise_url("git@gitlab.com:a/b/c.git").as_deref(),
            Some("gitlab.com/a/b/c")
        );
    }

    #[test]
    fn local_paths_and_file_urls_do_not_normalise() {
        for raw in [
            "/tmp/origin.git",
            "file:///tmp/origin.git",
            "../origin.git",
            "origin.git",
            "",
            "https://github.com/onlyowner",
            "https://github.com",
        ] {
            assert_eq!(normalise_url(raw), None, "{}", raw);
        }
    }

    #[test]
    fn repo_ids_are_lowercase_alphanumeric_with_hyphens() {
        for good in ["tsk", "work-api", "9lives", "a-b-c"] {
            assert!(valid_repo_id(good), "{}", good);
        }
        for bad in ["", "-x", "Tsk", "a_b", "a/b", "a.b", "a b"] {
            assert!(!valid_repo_id(bad), "{}", bad);
        }
    }

    #[test]
    fn parse_tolerates_a_territory_with_no_repos() {
        assert_eq!(index().entries().count(), 4);
    }

    #[test]
    fn parse_keeps_territory_ids_and_names() {
        let index = index();
        let territories: Vec<(&str, &str, usize)> = index
            .territories()
            .iter()
            .map(|t| (t.id.as_str(), t.name.as_str(), t.repos.len()))
            .collect();
        assert_eq!(territories, vec![("a", "A", 4), ("b", "B", 0)]);
    }

    #[test]
    fn parse_tolerates_a_territory_with_no_id_or_name() {
        let index = NexusIndex::parse(r#"{"territories":[{"repos":[{"id":"x"}]}]}"#).unwrap();
        assert_eq!(index.territories()[0].id, "");
        assert_eq!(index.territories()[0].name, "");
        assert_eq!(index.entries().count(), 1);
    }

    #[test]
    fn an_entry_may_carry_ledger_refs_and_unknown_fields() {
        let index = NexusIndex::parse(
            r#"{"territories":[{"repos":[
              {"id":"a","ledger_refs":"custom","note":"kept"},
              {"id":"b","ledger_refs":"branches"},
              {"id":"c"}
            ]}]}"#,
        )
        .unwrap();
        let refs: Vec<Option<&str>> = index.entries().map(|e| e.ledger_refs.as_deref()).collect();
        assert_eq!(refs, vec![Some("custom"), Some("branches"), None]);
    }

    #[test]
    fn parse_rejects_malformed_json() {
        assert!(NexusIndex::parse("{").is_err());
    }

    #[test]
    fn selects_by_https_or_ssh_origin() {
        let index = index();
        let https = select_entry(&index, None, Some("https://github.com/acme/work-api"), None);
        assert_eq!(https.unwrap().id, "work-api");
        let ssh = select_entry(
            &index,
            None,
            Some("ssh://git@github.com/acme/work-api.git"),
            None,
        );
        assert_eq!(ssh.unwrap().id, "work-api");
    }

    #[test]
    fn the_cached_id_is_used_before_the_origin() {
        let index = index();
        let found = select_entry(
            &index,
            Some("tsk"),
            Some("https://github.com/acme/work-api"),
            None,
        );
        assert_eq!(found.unwrap().id, "tsk");
    }

    #[test]
    fn a_stale_cached_id_falls_back_to_the_origin() {
        let index = index();
        let found = select_entry(
            &index,
            Some("gone"),
            Some("https://github.com/acme/work-api"),
            None,
        );
        assert_eq!(found.unwrap().id, "work-api");
    }

    #[test]
    fn an_entry_with_no_url_is_found_only_by_its_cached_id_on_its_machine() {
        let index = index();
        let found = select_entry(&index, Some("scratch"), None, Some("laptop"));
        assert_eq!(found.unwrap().id, "scratch");
        assert!(select_entry(&index, None, None, Some("laptop")).is_none());
    }

    #[test]
    fn an_entry_local_to_another_machine_is_skipped() {
        let index = index();
        assert!(select_entry(&index, Some("other-scratch"), None, Some("laptop")).is_none());
        assert!(select_entry(&index, Some("scratch"), None, None).is_none());
    }

    #[test]
    fn a_local_path_origin_matches_nothing() {
        let index = index();
        assert!(select_entry(&index, None, Some("/tmp/origin.git"), None).is_none());
    }

    #[test]
    fn ledger_field_chooses_the_location() {
        let index = index();
        let nexus = "https://example.test/o/nexus";
        let by_id = |id: &str| index.entries().find(|e| e.id == id).unwrap();
        assert_eq!(
            entry_location(by_id("tsk"), nexus).unwrap(),
            LedgerLocation::in_repo()
        );
        assert_eq!(
            entry_location(by_id("work-api"), nexus).unwrap(),
            LedgerLocation::nexus(nexus, "work-api")
        );
    }

    #[test]
    fn explicit_repo_ledger_is_in_repo_and_unknown_values_are_errors() {
        let nexus = "n";
        let repo = Entry {
            id: "x".into(),
            url: None,
            local: None,
            ledger: Some("repo".into()),
            ledger_refs: None,
        };
        assert_eq!(
            entry_location(&repo, nexus).unwrap(),
            LedgerLocation::in_repo()
        );
        let odd = Entry {
            ledger: Some("cloud".into()),
            ..repo
        };
        assert!(entry_location(&odd, nexus)
            .unwrap_err()
            .contains("\"cloud\""));
    }

    #[test]
    fn a_nexus_entry_with_an_invalid_id_is_an_error() {
        let entry = Entry {
            id: "Bad_Id".into(),
            url: None,
            local: None,
            ledger: Some("nexus".into()),
            ledger_refs: None,
        };
        assert!(entry_location(&entry, "n")
            .unwrap_err()
            .contains("not a valid repo id"));
    }
}
