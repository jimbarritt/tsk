//! What `tsk ledger status` prints for a clone with no ledger store: where the ledger is,
//! as far as the attached nexus and origin say, and the next action. A fresh clone, and a
//! fresh cloud session above all, starts here.

use std::path::Path;

use super::location::{state_root_from_env, Repo};
use super::nexus;

/// What the attached nexus says about this repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NexusState {
    /// No nexus URL in the user config.
    None,
    /// The nexus could not be fetched, with the last line of the error.
    Unreachable(String),
    /// The nexus lists this repo: its id, and whether its ledger is in the nexus.
    Entry { id: String, in_nexus: bool },
    /// The nexus does not list this repo.
    NoEntry,
}

/// The one action that moves a clone with no ledger on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// In a cloud session: attach the nexus repository to the session.
    AttachNexus,
    /// Outside a cloud session: the machine's git cannot fetch a remote.
    CheckAccess,
    /// A ledger exists: fetch it.
    Fetch,
    /// No ledger exists: create one.
    Init,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub nexus_url: Option<String>,
    pub nexus: NexusState,
    /// Whether origin holds a ledger: `None` when origin was not asked or cannot be.
    pub origin: Option<bool>,
    /// True when origin was asked.
    pub origin_asked: bool,
    pub next: Next,
    pub cloud: bool,
}

/// The report for the clone at `dir`, or `None` when the clone has a ledger store. With
/// `local`, nothing on the network is asked, and only the Local line is printed.
pub fn report(dir: &Path, local: bool) -> Result<Option<Report>, String> {
    let repo = Repo::discover(dir)?;
    let state_root = state_root_from_env()?;
    if nexus::has_local_store(&repo, &state_root)? {
        return Ok(None);
    }
    let cloud = crate::thread::session_start::is_remote();
    let nexus_url = match crate::config::config_file_from_env() {
        Some(file) => crate::config::read_nexus_url(&file)?,
        None => None,
    };
    if local {
        return Ok(Some(Report {
            nexus_url,
            nexus: NexusState::None,
            origin: None,
            origin_asked: false,
            next: Next::Fetch,
            cloud,
        }));
    }
    let nexus = match &nexus_url {
        None => NexusState::None,
        Some(url) => match nexus::load_index(&state_root, url) {
            Err(error) => NexusState::Unreachable(last_line(&error)),
            Ok(index) => {
                let cached = repo.read_repo_id()?;
                let origin = repo.raw_origin_url()?;
                let machine = nexus::machine_name();
                match nexus::select_entry(
                    &index,
                    cached.as_deref(),
                    origin.as_deref(),
                    machine.as_deref(),
                ) {
                    Some(entry) => NexusState::Entry {
                        id: entry.id.clone(),
                        in_nexus: entry.ledger.as_deref() == Some("nexus"),
                    },
                    None => NexusState::NoEntry,
                }
            }
        },
    };
    let (origin, origin_asked) = match nexus {
        NexusState::Unreachable(_) | NexusState::Entry { in_nexus: true, .. } => (None, false),
        _ => (nexus::origin_holds_ledger(&repo), true),
    };
    let next = next_action(&nexus, origin, cloud);
    Ok(Some(Report {
        nexus_url,
        nexus,
        origin,
        origin_asked,
        next,
        cloud,
    }))
}

/// The next action, from what the nexus and origin say.
pub fn next_action(nexus: &NexusState, origin: Option<bool>, cloud: bool) -> Next {
    match nexus {
        NexusState::Unreachable(_) if cloud => Next::AttachNexus,
        NexusState::Unreachable(_) => Next::CheckAccess,
        NexusState::Entry { in_nexus: true, .. } => Next::Fetch,
        _ => match origin {
            Some(true) => Next::Fetch,
            Some(false) => Next::Init,
            None => Next::CheckAccess,
        },
    }
}

/// The line of a fetch error that names the cause: git's `fatal:` line, else the last line.
fn last_line(error: &str) -> String {
    let lines: Vec<&str> = error
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    lines
        .iter()
        .find(|line| line.starts_with("fatal: "))
        .or(lines.last())
        .copied()
        .unwrap_or("")
        .trim_start_matches("error: ")
        .trim_start_matches("fatal: ")
        .to_string()
}

/// `owner` and `repo` of a GitHub URL, for the add_repo tool.
pub fn github_owner_repo(url: &str) -> Option<(String, String)> {
    let normal = nexus::normalise_url(url)?;
    let path = normal.strip_prefix("github.com/")?;
    let (owner, repo) = path.split_once('/')?;
    if repo.contains('/') {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

impl Report {
    fn nexus_text(&self) -> String {
        let url = self.nexus_url.as_deref().unwrap_or("");
        match &self.nexus {
            NexusState::None if self.nexus_url.is_some() => url.to_string(),
            NexusState::None => "none attached".to_string(),
            NexusState::Unreachable(reason) => {
                format!("{}: cannot be fetched: {}", url, reason)
            }
            NexusState::Entry { id, in_nexus: true } => {
                format!("{}: entry {}, ledger in the nexus", url, id)
            }
            NexusState::Entry {
                id,
                in_nexus: false,
            } => {
                format!("{}: entry {}, ledger in the repo", url, id)
            }
            NexusState::NoEntry => format!("{}: no entry for this repo", url),
        }
    }

    fn origin_text(&self) -> Option<&'static str> {
        if !self.origin_asked {
            return None;
        }
        Some(match self.origin {
            Some(true) => "holds a ledger",
            Some(false) => "holds no ledger",
            None => "cannot be reached",
        })
    }

    fn next_text(&self) -> String {
        let url = self.nexus_url.as_deref().unwrap_or("");
        match self.next {
            Next::AttachNexus => match github_owner_repo(url) {
                Some((owner, repo)) => format!(
                    "attach the nexus to this cloud session with the add_repo tool (owner {}, repo {}, access push), do not clone it (tsk fetches it itself), then run tsk ledger fetch",
                    owner, repo
                ),
                None => format!(
                    "attach the nexus repository {} to this cloud session, then run tsk ledger fetch",
                    url
                ),
            },
            Next::CheckAccess => match &self.nexus {
                NexusState::Unreachable(_) => format!(
                    "check that git on this machine can fetch {}, then run tsk ledger fetch",
                    url
                ),
                _ => "check that git on this machine can fetch origin, then run tsk ledger status again".to_string(),
            },
            Next::Fetch => "run tsk ledger fetch".to_string(),
            Next::Init if self.nexus_url.is_some() => "ask the person where the new ledger goes, then run tsk ledger init --nexus (in the nexus) or tsk ledger init --repo (in this repo)".to_string(),
            Next::Init => "run tsk ledger init --repo, or attach a nexus with tsk nexus add <url> and run tsk ledger init --nexus".to_string(),
        }
    }

    /// The lines for a person, with `local` as `tsk ledger status --local` prints them.
    pub fn human_lines(&self, local: bool) -> Vec<String> {
        let mut lines = vec!["Local   no ledger in this clone".to_string()];
        if local {
            return lines;
        }
        lines.push(format!("Nexus   {}", self.nexus_text()));
        if let Some(origin) = self.origin_text() {
            lines.push(format!("Origin  {}", origin));
        }
        lines.push(format!("Next    {}", self.next_text()));
        lines
    }

    /// The tab-separated facts, one per line.
    pub fn porcelain_lines(&self, local: bool) -> Vec<String> {
        let mut lines = vec!["local\tnone".to_string()];
        if local {
            return lines;
        }
        lines.push(format!(
            "nexus\t{}",
            self.nexus_url.as_deref().unwrap_or("-")
        ));
        let (state, entry) = match &self.nexus {
            NexusState::None => ("none", None),
            NexusState::Unreachable(_) => ("unreachable", None),
            NexusState::Entry { id, in_nexus } => (
                "entry",
                Some(format!(
                    "entry\t{}\t{}",
                    id,
                    if *in_nexus { "nexus" } else { "repo" }
                )),
            ),
            NexusState::NoEntry => ("no-entry", None),
        };
        lines.push(format!("nexus-state\t{}", state));
        lines.extend(entry);
        if self.origin_asked {
            lines.push(format!(
                "origin\t{}",
                match self.origin {
                    Some(true) => "ledger",
                    Some(false) => "no-ledger",
                    None => "unknown",
                }
            ));
        }
        lines.push(format!(
            "next\t{}",
            match self.next {
                Next::AttachNexus => "attach-nexus",
                Next::CheckAccess => "check-access",
                Next::Fetch => "fetch",
                Next::Init => "init",
            }
        ));
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(nexus: NexusState, origin: Option<bool>, cloud: bool) -> Report {
        let next = next_action(&nexus, origin, cloud);
        Report {
            nexus_url: Some("https://github.com/acme/nexus".to_string()),
            origin_asked: !matches!(
                nexus,
                NexusState::Unreachable(_) | NexusState::Entry { in_nexus: true, .. }
            ),
            nexus,
            origin,
            next,
            cloud,
        }
    }

    #[test]
    fn an_unreachable_nexus_in_a_cloud_session_names_add_repo() {
        let r = report(NexusState::Unreachable("no access".into()), None, true);
        assert_eq!(r.next, Next::AttachNexus);
        let lines = r.human_lines(false);
        assert_eq!(lines[0], "Local   no ledger in this clone");
        assert_eq!(
            lines[1],
            "Nexus   https://github.com/acme/nexus: cannot be fetched: no access"
        );
        assert!(lines[2].contains("add_repo tool (owner acme, repo nexus, access push)"));
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn an_unreachable_nexus_in_a_terminal_asks_to_check_access() {
        let r = report(NexusState::Unreachable("no access".into()), None, false);
        assert_eq!(r.next, Next::CheckAccess);
        assert!(!r.human_lines(false).join("\n").contains("add_repo"));
    }

    #[test]
    fn an_entry_in_the_nexus_is_fetched() {
        let r = report(
            NexusState::Entry {
                id: "tsk".into(),
                in_nexus: true,
            },
            None,
            true,
        );
        assert_eq!(r.next, Next::Fetch);
        assert_eq!(
            r.porcelain_lines(false),
            vec![
                "local\tnone",
                "nexus\thttps://github.com/acme/nexus",
                "nexus-state\tentry",
                "entry\ttsk\tnexus",
                "next\tfetch"
            ]
        );
    }

    #[test]
    fn no_entry_and_no_ledger_on_origin_asks_where_to_create_one() {
        let r = report(NexusState::NoEntry, Some(false), true);
        assert_eq!(r.next, Next::Init);
        let text = r.human_lines(false).join("\n");
        assert!(text.contains("Origin  holds no ledger"));
        assert!(text.contains("tsk ledger init --nexus"));
        assert!(text.contains("tsk ledger init --repo"));
    }

    #[test]
    fn no_entry_with_a_ledger_on_origin_is_fetched() {
        let r = report(NexusState::NoEntry, Some(true), false);
        assert_eq!(r.next, Next::Fetch);
    }

    #[test]
    fn local_prints_only_the_local_line() {
        let r = report(NexusState::NoEntry, Some(true), false);
        assert_eq!(r.human_lines(true), vec!["Local   no ledger in this clone"]);
        assert_eq!(r.porcelain_lines(true), vec!["local\tnone"]);
    }

    #[test]
    fn the_reason_is_the_fatal_line_of_the_git_error() {
        let error = "error: could not fetch the nexus x:\ngit fetch failed (exit status 128):\nfatal: 'x' does not appear to be a git repository\nfatal: Could not read from remote repository.\n\nPlease make sure you have the correct access rights\nand the repository exists.";
        assert_eq!(
            last_line(error),
            "'x' does not appear to be a git repository"
        );
        assert_eq!(last_line("error: plain"), "plain");
    }

    #[test]
    fn owner_and_repo_come_from_a_github_url_only() {
        assert_eq!(
            github_owner_repo("git@github.com:acme/nexus.git"),
            Some(("acme".to_string(), "nexus".to_string()))
        );
        assert_eq!(github_owner_repo("https://example.test/acme/nexus"), None);
    }
}
