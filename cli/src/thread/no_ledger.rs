use std::path::Path;

use crate::ledger::absent::{self, Next, NexusState};
use crate::ledger::location::{state_root_from_env, Repo};
use crate::ledger::{nexus, refs};

pub const OFFER_SWITCH_VAR: &str = "TSK_NEXUS_OFFER";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    Normal,
    NotUsed,
    InitOrSkip,
    NexusOffer,
}

pub fn not_used_context() -> String {
    "This repo is marked as not under tsk, in its nexus entry or in .git/tsk-not-used. Work without tsk and do not ask about it again. To undo the mark, run `tsk ledger not-used --forget`.".to_string()
}

pub fn init_or_skip_prompt() -> String {
    "This repo has no ledger. Use the AskUserQuestion tool with two options, in this order: \"Initialise a ledger\" (recommended), then run /tsk:init-ledger; \"Work without tsk\", then ask with the AskUserQuestion tool \"Remember this choice for this repo?\" with the options \"Yes\" and \"No\", and run `tsk ledger not-used` only on \"Yes\" (it records the choice in .git/tsk-not-used, since the nexus has no entry for this repo, so session start asks no more); on \"No\" record nothing. Do not offer to register the repo in the nexus: initialising does that. To undo the second choice later, run `tsk ledger not-used --forget`.".to_string()
}

pub fn no_nexus_offer(cloud: bool) -> String {
    let local = if cloud {
        ""
    } else {
        "\"Create a local nexus\" (recommended), then run `tsk nexus init-local --yes`, which makes ~/.local/share/tsk/nexus.git and attaches it; the folder holds the only copy of every ledger in it, so back it up; "
    };
    format!(
        "No nexus is attached, so tsk is not set up for this repo. Use the AskUserQuestion tool with these options, in this order: {}\"Attach a nexus by URL\", then ask for the clone URL and run `tsk nexus add <url>`; \"Work without tsk\", then ask with the AskUserQuestion tool \"Remember this choice for this repo?\" with the options \"Yes\" and \"No\", and run `tsk ledger not-used` only on \"Yes\" (it records the choice in .git/tsk-not-used, so session start asks no more); on \"No\" record nothing. After a nexus is attached, run `tsk ledger status` and do what its Next line says, which is /tsk:init-ledger for a repo with no ledger. To undo the last choice later, run `tsk ledger not-used --forget`.",
        local
    )
}

pub fn offer_applies(
    nexus_attached: bool,
    has_store: bool,
    origin: Option<bool>,
    explicit_config: bool,
) -> bool {
    !nexus_attached && !has_store && !explicit_config && origin == Some(false)
}

fn switched_off() -> bool {
    std::env::var(OFFER_SWITCH_VAR).is_ok_and(|v| v == "off")
}

pub fn detect(cwd: &Path) -> Start {
    let Ok(repo) = Repo::discover(cwd) else {
        return Start::Normal;
    };
    if nexus::marked_not_used(&repo) {
        return Start::NotUsed;
    }
    if switched_off() || super::session_start::unattended_start() {
        return Start::Normal;
    }
    let Ok(state_root) = state_root_from_env() else {
        return Start::Normal;
    };
    if nexus::has_local_store(&repo, &state_root).unwrap_or(true) {
        return Start::Normal;
    }
    let attached = crate::config::config_file_from_env()
        .and_then(|file| crate::config::read_nexus_url(&file).ok().flatten());
    if attached.is_some() {
        return match absent::report(cwd, false) {
            Ok(Some(report)) => match report.nexus {
                NexusState::Entry { not_used: true, .. } => Start::NotUsed,
                NexusState::NoEntry if report.next == Next::Init => Start::InitOrSkip,
                _ => Start::Normal,
            },
            _ => Start::Normal,
        };
    }
    let explicit = refs::configured(&repo, None).ok().flatten().is_some();
    let origin = (!explicit)
        .then(|| nexus::origin_holds_ledger(&repo))
        .flatten();
    if offer_applies(false, false, origin, explicit) {
        Start::NexusOffer
    } else {
        Start::Normal
    }
}

pub fn start_context(cwd: &Path, cloud: bool) -> Option<String> {
    match detect(cwd) {
        Start::Normal => None,
        Start::NotUsed => Some(not_used_context()),
        Start::InitOrSkip => Some(init_or_skip_prompt()),
        Start::NexusOffer => Some(no_nexus_offer(cloud)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offer_applies_only_with_no_nexus_no_store_no_choice_and_an_empty_origin() {
        assert!(offer_applies(false, false, Some(false), false));
        assert!(!offer_applies(true, false, Some(false), false));
        assert!(!offer_applies(false, true, Some(false), false));
        assert!(!offer_applies(false, false, Some(true), false));
        assert!(!offer_applies(false, false, None, false));
        assert!(!offer_applies(false, false, Some(false), true));
    }

    #[test]
    fn the_nexus_offer_names_the_options_and_omits_a_local_nexus_in_the_cloud() {
        let terminal = no_nexus_offer(false);
        assert!(terminal.contains("tsk nexus init-local --yes"));
        assert!(terminal.contains("tsk nexus add <url>"));
        assert!(terminal.contains("tsk ledger not-used"));
        let cloud = no_nexus_offer(true);
        assert!(!cloud.contains("init-local"));
        assert!(cloud.contains("tsk nexus add <url>"));
        for text in [&terminal, &cloud] {
            assert!(text.contains("Remember this choice for this repo?"));
            assert!(!text.contains("private"));
            assert!(!text.contains('\u{2014}'));
        }
    }

    #[test]
    fn the_init_prompt_has_two_options_and_does_not_offer_to_register() {
        let text = init_or_skip_prompt();
        assert!(text.contains("two options"));
        let first = text.find("Initialise a ledger").unwrap();
        let second = text.find("Work without tsk").unwrap();
        assert!(first < second);
        assert!(text.contains("Do not offer to register"));
        assert!(text.contains("tsk ledger not-used"));
        assert!(text.contains("Remember this choice for this repo?"));
        assert!(text.contains("only on"));
    }

    #[test]
    fn the_not_used_context_says_how_to_undo() {
        assert!(not_used_context().contains("tsk ledger not-used --forget"));
    }
}
