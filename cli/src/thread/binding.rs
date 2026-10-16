use std::fmt;
use std::path::{Path, PathBuf};

use crate::mission::model::{self, Mission, Status};

use super::clock::utc_now;
use super::session::Session;

pub const MARKER_SKIP_FILE: &str = "tsk-thread-skip";

const SKIP_SENTENCE: &str =
    "If the user wants to work without a thread, run `tsk thread skip` instead.";

/// The prompt for a ledger with no mission records.
pub const UNBOUND_PROMPT: &str = "No thread binding was found for this session or worktree. Use the AskUserQuestion tool to ask which mission to work: offer your best-inferred candidate (from index.md, the mission tree, or anything already said this session) as one selectable option, one or two other unblocked missions as alternatives, and leave free text open for anything else. Once answered, run /tsk:start-thread for it, or /tsk:resume-thread <thread-id> if an existing thread is named instead. If the user wants to work without a thread, run `tsk thread skip` instead.";

/// The prompt for a ledger with mission records: it offers the missions not done, as
/// `tsk mission list` prints them. With no lines it is [`UNBOUND_PROMPT`].
pub fn unbound_prompt(mission_lines: &[String]) -> String {
    if mission_lines.is_empty() {
        return UNBOUND_PROMPT.to_string();
    }
    format!(
        "No thread binding was found for this session or worktree. The missions not done are listed below, one per line: id, status, blocked-by, parent, name, tab-separated.\n{}\nUse the AskUserQuestion tool to ask which mission to work: offer your best-inferred candidate (from this list or anything already said this session) as one selectable option, one or two other unblocked missions from the list as alternatives, and leave free text open for anything else. Once answered, run /tsk:start-thread <mission-id> for it, or /tsk:resume-thread <thread-id> if an existing thread is named instead. {}",
        mission_lines.join("\n"),
        SKIP_SENTENCE
    )
}

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

pub fn resolve(session: &Session) -> Result<Option<Binding>, String> {
    match &session.cloud_session {
        Some(cloud) => Ok(session
            .store
            .cloud_lookup()?
            .thread_for(cloud)
            .map(str::to_string)
            .map(Binding::Cloud)),
        None => Ok(read_marker(&session.marker_path())?.map(Binding::Worktree)),
    }
}

pub fn resolve_local(session: &Session) -> Result<Option<Binding>, String> {
    if session.store.exists_locally()? {
        resolve(session)
    } else {
        Ok(None)
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

pub fn bind_cloud(session: &Session, cloud: &str, thread_id: &str) -> Result<(), String> {
    session.store.cloud_bind(cloud, thread_id, &utc_now())
}

/// The missions not done, as `tsk mission list` prints them.
pub fn open_mission_lines(missions: &[Mission]) -> Vec<String> {
    missions
        .iter()
        .filter(|m| m.status != Status::Done)
        .map(|m| model::list_line(missions, m))
        .collect()
}

/// Where this session's or worktree's skip is recorded: a file in the git directory,
/// beside the worktree marker, so it is local and never pushed. A cloud session has a
/// file of its own, named for its session ID.
pub fn skip_path(session: &Session) -> PathBuf {
    match &session.cloud_session {
        Some(cloud) => {
            let safe: String = cloud
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            session
                .git_dir
                .join(format!("{}-{}", MARKER_SKIP_FILE, safe))
        }
        None => session.git_dir.join(MARKER_SKIP_FILE),
    }
}

pub fn is_skipped(session: &Session) -> bool {
    skip_path(session).is_file()
}

pub fn write_skip(session: &Session) -> Result<(), String> {
    let path = skip_path(session);
    std::fs::write(&path, "skipped\n")
        .map_err(|e| format!("error: could not write {}: {}", path.display(), e))
}

pub fn clear_skip(session: &Session) -> Result<(), String> {
    let path = skip_path(session);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("error: could not remove {}: {}", path.display(), e)),
    }
}

pub fn bind_current(session: &Session, thread_id: &str) -> Result<(), String> {
    match &session.cloud_session {
        Some(cloud) => bind_cloud(session, cloud, thread_id),
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
}
