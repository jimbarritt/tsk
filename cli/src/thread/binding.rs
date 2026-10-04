use std::fmt;
use std::path::Path;

use super::clock::utc_now;
use super::lookup::{lookup_path, CloudBinding, Lookup};
use super::session::Session;

pub const UNBOUND_PROMPT: &str = "No thread binding was found for this session or worktree. Use the AskUserQuestion tool to ask which mission to work: offer your best-inferred candidate (from index.md, the mission tree, or anything already said this session) as one selectable option, one or two other unblocked missions as alternatives, and leave free text open for anything else. Once answered, run /start-thread for it, or /resume-thread <thread-id> if an existing thread is named instead.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    Cloud(String),
    Worktree(String),
}

impl Binding {
    pub fn thread_id(&self) -> &str {
        match self {
            Binding::Cloud(id) | Binding::Worktree(id) => id,
        }
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Binding::Cloud(id) => write!(f, "cloud:{}", id),
            Binding::Worktree(id) => write!(f, "worktree:{}", id),
        }
    }
}

pub fn resolve_at(session: &Session, wt: &Path) -> Result<Option<Binding>, String> {
    match &session.cloud_session {
        Some(cloud) => Ok(Lookup::read(&lookup_path(wt))?
            .and_then(|lookup| lookup.thread_for(cloud).map(str::to_string))
            .map(Binding::Cloud)),
        None => Ok(read_marker(&session.marker_path())?.map(Binding::Worktree)),
    }
}

pub fn resolve_local(session: &Session) -> Result<Option<Binding>, String> {
    match session.existing_worktree()? {
        Some(wt) => resolve_at(session, &wt),
        None => Ok(None),
    }
}

pub fn read_marker(marker: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(marker) {
        Ok(text) => {
            let id: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            Ok(Some(id).filter(|id| !id.is_empty()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("error: could not read {}: {}", marker.display(), e)),
    }
}

pub fn write_marker(session: &Session, thread_id: &str) -> Result<(), String> {
    let marker = session.marker_path();
    std::fs::write(&marker, format!("{}\n", thread_id))
        .map_err(|e| format!("error: could not write {}: {}", marker.display(), e))
}

pub fn remove_marker(session: &Session) -> Result<(), String> {
    let marker = session.marker_path();
    match std::fs::remove_file(&marker) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!(
            "error: could not remove {}: {}",
            marker.display(),
            e
        )),
    }
}

pub fn bind_cloud(cloud: &str, thread_id: &str, wt: &Path) -> Result<(), String> {
    let path = lookup_path(wt);
    let mut lookup = Lookup::read(&path)?.unwrap_or_default();
    lookup.set(cloud, CloudBinding::new(thread_id, &utc_now()));
    lookup.write(&path)
}

pub fn bind_current(session: &Session, thread_id: &str, wt: &Path) -> Result<(), String> {
    match &session.cloud_session {
        Some(cloud) => bind_cloud(cloud, thread_id, wt),
        None => write_marker(session, thread_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_renders_with_its_channel_prefix() {
        assert_eq!(
            Binding::Cloud("1234abcd".into()).to_string(),
            "cloud:1234abcd"
        );
        assert_eq!(
            Binding::Worktree("1234abcd".into()).to_string(),
            "worktree:1234abcd"
        );
        assert_eq!(Binding::Worktree("1234abcd".into()).thread_id(), "1234abcd");
    }

    #[test]
    fn read_marker_strips_whitespace_and_treats_empty_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("tsk-thread-id");
        assert_eq!(read_marker(&marker).unwrap(), None);
        std::fs::write(&marker, " \n").unwrap();
        assert_eq!(read_marker(&marker).unwrap(), None);
        std::fs::write(&marker, "1234abcd\n").unwrap();
        assert_eq!(read_marker(&marker).unwrap(), Some("1234abcd".to_string()));
    }

    #[test]
    fn bind_cloud_creates_the_lookup_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        bind_cloud("cse_a", "1234abcd", dir.path()).unwrap();
        let lookup = Lookup::read(&lookup_path(dir.path())).unwrap().unwrap();
        assert_eq!(lookup.thread_for("cse_a"), Some("1234abcd"));
    }
}
