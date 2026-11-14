//! The state of the harnesses of a clone. One JSON file per harness, and one file of the
//! names ever used (ADR 0032 keeps the thread, the harness keeps the worktree).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What tsk keeps of one harness: a code worktree, a tmux session and a `claude` process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessState {
    pub name: String,
    pub worktree: PathBuf,
    pub branch: String,
    pub main_worktree: PathBuf,
    pub tmux_session: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub robot: Option<String>,
    #[serde(default)]
    pub mission: Option<String>,
    pub created_at: String,
}

/// The directory of the state files of a clone: `<state root>/harnesses/<clone-id>`.
pub fn harness_dir(state_root: &Path, clone_id: &str) -> PathBuf {
    state_root.join("harnesses").join(clone_id)
}

/// The file of the names a clone has used: `<state root>/repos/<clone-id>/harness-names`.
pub fn names_file(state_root: &Path, clone_id: &str) -> PathBuf {
    state_root
        .join("repos")
        .join(clone_id)
        .join("harness-names")
}

pub fn state_file(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{}.json", name))
}

/// Every harness of the directory, by name. A missing directory holds none.
pub fn load_all(dir: &Path) -> Result<Vec<HarnessState>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("error: could not read {}: {}", dir.display(), e)),
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("error: could not read {}: {}", path.display(), e))?;
        let state: HarnessState = serde_json::from_str(&text).map_err(|e| {
            format!(
                "error: {} is not a harness state file: {}",
                path.display(),
                e
            )
        })?;
        found.push(state);
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

pub fn save(dir: &Path, state: &HarnessState) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
    let path = state_file(dir, &state.name);
    let text = serde_json::to_string_pretty(state)
        .map_err(|e| format!("error: could not write the harness state: {}", e))?;
    std::fs::write(&path, format!("{}\n", text))
        .map_err(|e| format!("error: could not write {}: {}", path.display(), e))
}

pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
    let path = state_file(dir, name);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("error: could not remove {}: {}", path.display(), e)),
    }
}

/// The names the clone has used, live and deleted.
pub fn used_names(file: &Path) -> Result<BTreeSet<String>, String> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(super::names::parse_used(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(e) => Err(format!("error: could not read {}: {}", file.display(), e)),
    }
}

/// Record a name as used. A name stays recorded after its harness is deleted.
pub fn record_name(file: &Path, name: &str) -> Result<(), String> {
    use std::io::Write;
    if used_names(file)?.contains(name) {
        return Ok(());
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .map_err(|e| format!("error: could not open {}: {}", file.display(), e))?;
    writeln!(handle, "{}", name)
        .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> HarnessState {
        HarnessState {
            name: name.to_string(),
            worktree: PathBuf::from("/w/repo").join(name),
            branch: format!("harness/{}", name),
            main_worktree: PathBuf::from("/m/repo"),
            tmux_session: name.to_string(),
            thread_id: None,
            robot: Some("Marvin".to_string()),
            mission: None,
            created_at: "2026-11-14T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn a_state_file_round_trips_and_the_list_is_sorted_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().join("harnesses").join("c1");
        assert!(load_all(&dir).unwrap().is_empty());
        save(&dir, &sample("owl")).unwrap();
        save(&dir, &sample("fox")).unwrap();
        let all = load_all(&dir).unwrap();
        assert_eq!(all, vec![sample("fox"), sample("owl")]);
        remove(&dir, "fox").unwrap();
        remove(&dir, "fox").unwrap();
        assert_eq!(load_all(&dir).unwrap(), vec![sample("owl")]);
    }

    #[test]
    fn a_recorded_name_is_recorded_once() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("repos").join("c1").join("harness-names");
        record_name(&file, "fox").unwrap();
        record_name(&file, "fox").unwrap();
        record_name(&file, "owl").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fox\nowl\n");
        assert_eq!(used_names(&file).unwrap().len(), 2);
    }
}
