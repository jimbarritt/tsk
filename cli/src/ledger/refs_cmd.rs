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
    let (placement, choice) = nexus::choose(&repo, &state_root, config_file.as_deref())?;
    let location = placement.base.with_refs(choice.refs, choice.source);
    println!("ledger refs: {}", choice.refs.as_str());
    match (choice.source, refs::repo_config_path(&repo)) {
        (RefsSource::RepoConfig, Some(file)) => {
            println!("source: {} ({})", choice.source.label(), file.display())
        }
        _ => println!("source: {}", choice.source.label()),
    }
    println!("ledger ref: {}", location.ref_name());
    println!("docs ref: {}", location.docs_ref_name());
    refs::check_store(&repo, &state_root, &location)
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
    let placement = nexus::place(&repo, &state_root, config_file.as_deref())?;
    let wanted = placement.base.with_refs(mode, RefsSource::RepoConfig);

    if let Some(held_ref) = refs::store_ref(&repo, &state_root) {
        if let Some(held) = LedgerRefs::of_ref(&held_ref).filter(|held| *held != mode) {
            let held_location = placement.base.with_refs(held, RefsSource::LedgerStore);
            if remote_tip(&repo.git, &held_location)?.is_some() {
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
    (LedgerRefs::parse(value) != Some(mode)).then(|| {
        format!(
            "note: the nexus entry \"{}\" sets ledger_refs to \"{}\", which takes precedence over the repo config",
            id, value
        )
    })
}
