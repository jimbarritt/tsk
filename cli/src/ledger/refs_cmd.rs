//! `tsk config ledger-refs`: show which refs hold this repo's ledger, or set the repo
//! config. See [`super::refs`] for the modes and the order they are read in.

use super::location::{state_root_from_env, Repo};
use super::nexus;
use super::refs::{self, LedgerRefs, RefsSource};
use crate::ledger::store::dolt::remote_tip;

fn repo_here() -> Result<Repo, String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    Repo::discover(&cwd)
}

/// Print the mode in force and where it came from. Exits with an error, after printing,
/// when the mode disagrees with a ledger that already exists.
pub fn show() -> Result<(), String> {
    let repo = repo_here()?;
    let state_root = state_root_from_env()?;
    let config_file = crate::config::config_file_from_env();
    let (placement, choice, unlisted) =
        nexus::choose_or_unlisted(&repo, &state_root, config_file.as_deref())?;
    let location = nexus::display_location(&repo, &state_root, &placement, &choice);
    println!("ledger refs: {}", choice.refs.as_str());
    match (choice.source, refs::repo_config_path(&repo)) {
        (RefsSource::RepoConfig, Some(file)) => {
            println!("source: {} ({})", choice.source.label(), file.display())
        }
        _ => println!("source: {}", choice.source.label()),
    }
    println!("ledger ref: {}", location.ref_name());
    println!("docs ref: {}", location.docs_ref_name());
    if unlisted {
        println!("note: the attached nexus has no entry for this repo yet");
    }
    refs::check_store(&repo, &state_root, &location)
}

/// One line for `tsk ledger status`: the mode in force, where it came from and the refs.
pub fn summary() -> Result<String, String> {
    let repo = repo_here()?;
    let state_root = state_root_from_env()?;
    let config_file = crate::config::config_file_from_env();
    let (placement, choice, _) =
        nexus::choose_or_unlisted(&repo, &state_root, config_file.as_deref())?;
    let location = nexus::display_location(&repo, &state_root, &placement, &choice);
    Ok(format!(
        "refs {} ({}): data {}, documents {}",
        choice.refs.as_str(),
        choice.source.label(),
        location.ref_name(),
        location.docs_ref_name()
    ))
}

pub fn warning() -> Option<String> {
    let repo = repo_here().ok()?;
    let state_root = state_root_from_env().ok()?;
    let config_file = crate::config::config_file_from_env();
    let (_, choice, _) =
        nexus::choose_or_unlisted(&repo, &state_root, config_file.as_deref()).ok()?;
    refs::cloud_custom_warning(choice.refs, choice.source)
}

/// Write the repo config, and move a ledger store that was never published to the refs of
/// the new mode. A ledger that origin holds cannot move.
pub fn set(mode: LedgerRefs) -> Result<(), String> {
    let repo = repo_here()?;
    let state_root = state_root_from_env()?;
    let config_file = crate::config::config_file_from_env();
    let file = refs::repo_config_path(&repo).ok_or_else(|| {
        "error: tsk config ledger-refs needs a repo with a working tree: it writes .tsk/config.toml at the top of it".to_string()
    })?;
    let placement = match nexus::place_or_unlisted(&repo, &state_root, config_file.as_deref())? {
        nexus::Placed::Found(placement) => placement,
        nexus::Placed::Unlisted(_) => nexus::Placement {
            base: super::location::LedgerLocation::in_repo(),
            entry_refs: None,
            entry_marked: false,
        },
    };
    let wanted = placement.base.with_refs(mode, RefsSource::RepoConfig);

    if let Some(held_ref) = refs::store_ref(&repo, &state_root) {
        if let Some(held) = LedgerRefs::of_ref(&held_ref).filter(|held| *held != mode) {
            let held_location = placement.base.with_refs(held, RefsSource::LedgerStore);
            // Under the current name, or under the name of tsk 0.69.0 and earlier.
            let mut names = vec![held_location.ref_name().to_string()];
            names.extend(held_location.legacy_ref_names().map(|(data, _)| data));
            let mut published = false;
            for name in names {
                let probe = super::location::LedgerLocation::named(&held_location, &name);
                published |= remote_tip(&repo.git, &probe)?.is_some();
            }
            if published {
                return Err(refs::cannot_switch(
                    held_location.ref_name(),
                    held_location.label(),
                    mode,
                ));
            }
            refs::repoint_store(&repo, &state_root, &wanted)?;
        }
    }

    refs::write_repo_refs(&file, mode)?;
    refs::forget_cache(&repo, &state_root);
    refs::memo_forget();

    if let Some(note) = overriding_note(&placement, mode) {
        eprintln!("{}", note);
    }
    println!("ledger refs: {}", mode.as_str());
    println!("ledger ref: {}", wanted.ref_name());
    println!("docs ref: {}", wanted.docs_ref_name());
    println!("wrote {}", file.display());
    Ok(())
}

/// A sentence when a source that outranks the repo config says something else.
fn overriding_note(placement: &nexus::Placement, mode: LedgerRefs) -> Option<String> {
    if let Some(value) = std::env::var(refs::ENV_VAR)
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        if LedgerRefs::parse(&value) != Some(mode) {
            return Some(format!(
                "note: {} is \"{}\" in the environment, which takes precedence over the repo config",
                refs::ENV_VAR,
                value.trim()
            ));
        }
    }
    let (id, value) = placement.entry_refs.as_ref()?;
    (refs::EntryRefs::parse(value).map(|found| found.refs) != Some(mode)).then(|| {
        format!(
            "note: the nexus entry \"{}\" sets ledger_refs to \"{}\", which takes precedence over the repo config",
            id,
            value.lines().next().unwrap_or("")
        )
    })
}
