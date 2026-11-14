use std::path::Path;

use crate::config;
use crate::ledger::location::Repo;
use crate::ledger::nexus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushWorkflow {
    Main,
    PullRequest,
}

impl PushWorkflow {
    pub fn parse(text: &str) -> Option<PushWorkflow> {
        match text.trim().to_lowercase().as_str() {
            "m" | "main" => Some(PushWorkflow::Main),
            "p" | "pr" | "pull-request" | "pull request" => Some(PushWorkflow::PullRequest),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PushWorkflow::Main => "main",
            PushWorkflow::PullRequest => "pull-request",
        }
    }
}

pub fn parse_flag(text: &str) -> Result<PushWorkflow, String> {
    PushWorkflow::parse(text)
        .ok_or_else(|| format!("'{}' is not a push workflow: use main or pr", text))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushSource {
    Flag,
    Nexus,
    ConfigFile,
}

impl PushSource {
    pub fn label(self) -> &'static str {
        match self {
            PushSource::Flag => "the --push flag",
            PushSource::Nexus => "the nexus entry of this repo",
            PushSource::ConfigFile => ".tsk/config.toml",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusEntry {
    pub url: String,
    pub repo_id: Option<String>,
    pub workflow: Option<String>,
}

pub fn nexus_lookup(
    repo: &Repo,
    state_root: &Path,
    config_file: Option<&Path>,
) -> Result<Option<NexusEntry>, String> {
    let Some(url) = config_file
        .map(config::read_nexus_url)
        .transpose()?
        .flatten()
    else {
        return Ok(None);
    };
    let index = nexus::load_index(state_root, &url)?;
    let cached = repo.read_repo_id()?;
    let origin = repo.raw_origin_url()?;
    let machine = nexus::machine_name();
    let entry = nexus::select_entry(
        &index,
        cached.as_deref(),
        origin.as_deref(),
        machine.as_deref(),
    );
    Ok(Some(NexusEntry {
        url,
        repo_id: entry.map(|e| e.id.clone()),
        workflow: entry.and_then(|e| e.workflow.clone()),
    }))
}

fn invalid(what: &str, value: &str) -> String {
    format!(
        "error: {} is \"{}\"; the values are \"main\" and \"pull-request\"",
        what, value
    )
}

pub fn stored(
    nexus: Option<&NexusEntry>,
    config_value: Option<&str>,
) -> Result<Option<(PushWorkflow, PushSource)>, String> {
    if let Some(entry) = nexus {
        if let (Some(id), Some(value)) = (&entry.repo_id, &entry.workflow) {
            let workflow = PushWorkflow::parse(value).ok_or_else(|| {
                invalid(&format!("workflow on the nexus entry \"{}\"", id), value)
            })?;
            return Ok(Some((workflow, PushSource::Nexus)));
        }
    }
    if let Some(value) = config_value {
        let workflow = PushWorkflow::parse(value)
            .ok_or_else(|| invalid("harness.workflow in .tsk/config.toml", value))?;
        return Ok(Some((workflow, PushSource::ConfigFile)));
    }
    Ok(None)
}

pub fn save_target(lookup: Option<&NexusEntry>) -> Option<(&str, &str)> {
    let entry = lookup?;
    Some((entry.url.as_str(), entry.repo_id.as_deref()?))
}

pub fn unsaved_warning(lookup: Option<&NexusEntry>, workflow: PushWorkflow, why: &str) -> String {
    let repo_id = lookup
        .and_then(|entry| entry.repo_id.as_deref())
        .unwrap_or("<repo id>");
    format!(
        "warning: the answer was not saved ({}). Save it with: tsk nexus set-repo {} --workflow {}",
        why,
        repo_id,
        workflow.as_str()
    )
}

pub fn render_question() -> Vec<String> {
    let rows = [
        (
            "main",
            "rebase on origin's main and push with git push origin HEAD:main",
        ),
        ("pr", "push the branch and open a pull request"),
    ];
    let width = rows.iter().map(|(a, _)| a.len()).max().unwrap_or(0);
    let mut lines = vec!["Push to main or open pull requests?".to_string()];
    for (answer, note) in rows {
        lines.push(format!("  {:<width$}  {}", answer, note, width = width));
    }
    lines
}

const MAIN_RULE: &str = "This repo works on main: before you push, rebase on origin's main with `git rebase origin/main`, then push with `git push origin HEAD:main`. Open no pull request.";
const DETACHED: &str =
    "This worktree has no branch: its HEAD is detached at origin's default branch.";

pub fn step(branch: Option<&str>, push: Option<PushWorkflow>) -> String {
    match (branch, push) {
        (Some(branch), Some(PushWorkflow::Main)) => {
            format!("This worktree is on branch {}. {}", branch, MAIN_RULE)
        }
        (Some(branch), Some(PushWorkflow::PullRequest)) => format!("This worktree is on branch {}. This repo uses pull requests: push {} with `git push origin {}` and open a pull request from it. Push nothing to main.", branch, branch, branch),
        (Some(branch), None) => format!("This worktree is on branch {}. Ask me whether to push to main or to open a pull request before you push.", branch),
        (None, Some(PushWorkflow::Main)) => format!("{} {}", DETACHED, MAIN_RULE),
        (None, Some(PushWorkflow::PullRequest)) => format!("{} This repo uses pull requests: before you commit, create a branch named for the work, and a new one for each pull request. Push it with `git push origin <branch>` and open a pull request from it. Push nothing to main.", DETACHED),
        (None, None) => format!("{} Ask me whether to push to main or to open a pull request before you push. If I say pull request, create a branch named for the work before you commit, and a new one for each pull request.", DETACHED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: Option<&str>, workflow: Option<&str>) -> NexusEntry {
        NexusEntry {
            url: "nexus-url".to_string(),
            repo_id: id.map(str::to_string),
            workflow: workflow.map(str::to_string),
        }
    }

    #[test]
    fn answers_parse_as_letters_and_words() {
        for text in ["m", "main", "Main", " MAIN "] {
            assert_eq!(PushWorkflow::parse(text), Some(PushWorkflow::Main));
        }
        for text in ["p", "pr", "PR", "pull-request", "pull request"] {
            assert_eq!(PushWorkflow::parse(text), Some(PushWorkflow::PullRequest));
        }
        for text in ["", "x", "mai", "prs"] {
            assert_eq!(PushWorkflow::parse(text), None, "{}", text);
        }
        assert!(parse_flag("maybe").is_err());
        assert_eq!(parse_flag("pr").unwrap(), PushWorkflow::PullRequest);
    }

    #[test]
    fn the_nexus_entry_outranks_the_config_file() {
        let nexus = entry(Some("tsk"), Some("pull-request"));
        assert_eq!(
            stored(Some(&nexus), Some("main")).unwrap(),
            Some((PushWorkflow::PullRequest, PushSource::Nexus))
        );
        assert_eq!(
            stored(None, Some("main")).unwrap(),
            Some((PushWorkflow::Main, PushSource::ConfigFile))
        );
        let without = entry(Some("tsk"), None);
        assert_eq!(
            stored(Some(&without), Some("pull-request")).unwrap(),
            Some((PushWorkflow::PullRequest, PushSource::ConfigFile))
        );
        assert_eq!(stored(Some(&without), None).unwrap(), None);
        assert_eq!(stored(None, None).unwrap(), None);
    }

    #[test]
    fn a_stored_value_that_is_not_a_workflow_is_an_error() {
        let nexus = entry(Some("tsk"), Some("direct"));
        assert!(stored(Some(&nexus), None).unwrap_err().contains("direct"));
        assert!(stored(None, Some("x"))
            .unwrap_err()
            .contains("harness.workflow"));
    }

    #[test]
    fn an_answer_is_saved_only_to_an_entry_of_an_attached_nexus() {
        assert_eq!(save_target(None), None);
        assert_eq!(save_target(Some(&entry(None, None))), None);
        assert_eq!(
            save_target(Some(&entry(Some("tsk"), None))),
            Some(("nexus-url", "tsk"))
        );
    }

    #[test]
    fn the_warning_names_the_command_that_saves_the_answer() {
        let warning = unsaved_warning(
            Some(&entry(Some("tsk"), None)),
            PushWorkflow::PullRequest,
            "offline",
        );
        assert!(warning.starts_with("warning: the answer was not saved (offline)"));
        assert!(warning.ends_with("tsk nexus set-repo tsk --workflow pull-request"));
        assert!(unsaved_warning(None, PushWorkflow::Main, "x").contains("set-repo <repo id>"));
    }

    #[test]
    fn the_question_lists_the_answers_aligned() {
        assert_eq!(
            render_question(),
            vec![
                "Push to main or open pull requests?".to_string(),
                "  main  rebase on origin's main and push with git push origin HEAD:main"
                    .to_string(),
                "  pr    push the branch and open a pull request".to_string(),
            ]
        );
    }

    #[test]
    fn the_step_states_the_workflow_for_a_branch() {
        assert_eq!(
            step(Some("otter"), Some(PushWorkflow::Main)),
            "This worktree is on branch otter. This repo works on main: before you push, rebase on origin's main with `git rebase origin/main`, then push with `git push origin HEAD:main`. Open no pull request."
        );
        assert_eq!(
            step(Some("otter"), Some(PushWorkflow::PullRequest)),
            "This worktree is on branch otter. This repo uses pull requests: push otter with `git push origin otter` and open a pull request from it. Push nothing to main."
        );
        assert_eq!(
            step(Some("otter"), None),
            "This worktree is on branch otter. Ask me whether to push to main or to open a pull request before you push."
        );
    }

    #[test]
    fn the_step_states_the_workflow_for_a_detached_head() {
        for push in [
            None,
            Some(PushWorkflow::Main),
            Some(PushWorkflow::PullRequest),
        ] {
            assert!(step(None, push).starts_with(
                "This worktree has no branch: its HEAD is detached at origin's default branch."
            ));
        }
        assert!(step(None, Some(PushWorkflow::Main)).contains("git push origin HEAD:main"));
        assert!(step(None, Some(PushWorkflow::PullRequest))
            .contains("create a branch named for the work"));
        assert!(step(None, None).contains("Ask me whether to push to main"));
    }
}
