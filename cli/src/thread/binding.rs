use std::fmt;
use std::path::{Path, PathBuf};

use crate::ledger::store::{BindingKind, BindingRow};
use crate::mission::model::{self, Mission, Status};

use super::clock::utc_now;
use super::session::Session;

pub const MARKER_SKIP_FILE: &str = "tsk-thread-skip";

const SKIP_SENTENCE: &str =
    "If the user wants to work without a thread, run `tsk thread skip` instead.";

/// The prompt for a ledger with no mission records.
pub const UNBOUND_PROMPT: &str = "No thread binding was found for this session or worktree. Use the AskUserQuestion tool to ask which mission to work: offer your best-inferred candidate (from `tsk mission list`, or anything already said this session) as one selectable option, one or two other unblocked missions as alternatives, and leave free text open for anything else. Once answered, run /tsk:start-thread for it, or /tsk:resume-thread <thread-id> if an existing thread is named instead. If the user wants to work without a thread, run `tsk thread skip` instead.";

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

/// One thread a robot is bound to, as the unbound prompt lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotThread {
    pub id: String,
    pub mission: Option<String>,
    /// The timestamp of the latest continuation entry.
    pub latest: Option<String>,
}

/// A registered robot and its threads, latest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Robot {
    pub name: String,
    pub threads: Vec<RobotThread>,
}

/// The registered robots, the one with the latest thread first and robots with no thread
/// last, from the actors and the thread bindings.
pub fn robots(store: &dyn crate::ledger::store::LedgerStore) -> Result<Vec<Robot>, String> {
    let rows = super::ops::list_rows(store)?;
    let mut robots: Vec<Robot> = store
        .actors_list()?
        .into_iter()
        .filter(|actor| actor.kind == crate::ledger::store::ActorKind::Robot)
        .map(|actor| {
            let mut threads: Vec<RobotThread> = rows
                .iter()
                .filter(|row| {
                    row.actors
                        .iter()
                        .any(|bound| bound.name.as_deref() == Some(actor.name.as_str()))
                })
                .map(|row| RobotThread {
                    id: row.id.clone(),
                    mission: row.mission_id.clone(),
                    latest: row.latest_timestamp.clone(),
                })
                .collect();
            threads.sort_by(|a, b| b.latest.cmp(&a.latest));
            Robot {
                name: actor.name,
                threads,
            }
        })
        .collect();
    robots.sort_by(|a, b| {
        let latest = |r: &Robot| r.threads.first().and_then(|t| t.latest.clone());
        latest(b).cmp(&latest(a))
    });
    Ok(robots)
}

fn thread_text(thread: &RobotThread) -> String {
    let date = thread
        .latest
        .as_deref()
        .map(|t| t.get(..10).unwrap_or(t))
        .unwrap_or("never paused");
    format!(
        "{} {} {}",
        thread.id,
        thread.mission.as_deref().unwrap_or("no-mission"),
        date
    )
}

fn robot_line(robot: &Robot) -> String {
    let threads: Vec<String> = robot.threads.iter().map(thread_text).collect();
    format!(
        "{}\t{}",
        robot.name,
        if threads.is_empty() {
            "no threads".to_string()
        } else {
            threads.join(", ")
        }
    )
}

fn missions_part(mission_lines: &[String]) -> String {
    if mission_lines.is_empty() {
        "Offer the missions from `tsk mission list`.".to_string()
    } else {
        format!(
            "The missions not done are listed below, one per line: id, status, blocked-by, parent, name, tab-separated.\n{}",
            mission_lines.join("\n")
        )
    }
}

/// The prompt for an unbound session, robot first. With no robot registered for this
/// session and robots in the ledger, it asks which robot this session is. With a robot that
/// has threads, it offers them. Otherwise it is [`unbound_prompt`].
pub fn robot_first_prompt(
    current: Option<&str>,
    robots: &[Robot],
    mission_lines: &[String],
) -> String {
    match current {
        None if !robots.is_empty() => {
            let lines: Vec<String> = robots.iter().map(robot_line).collect();
            format!(
                "No thread binding was found for this session or worktree, and no robot is registered for it. Choose the robot first. The registered robots are listed below, one per line: name, then its threads (thread id, mission, date of the latest pause), tab-separated, the robot with the latest thread first.\n{}\nUse the AskUserQuestion tool to ask which robot this session is: offer up to three robots from the list, the one with the latest thread first, and \"Construct a new robot\", and leave free text open. Once answered, run `tsk actor register \"<name>\" --kind robot` for a robot from the list, or /tsk:construct-robot for a new one. Then use the AskUserQuestion tool again: offer that robot's threads, latest first, to resume with /tsk:resume-thread <thread-id>, and a new thread on a mission with /tsk:start-thread <mission-id>. {}\n{}",
                lines.join("\n"),
                missions_part(mission_lines),
                SKIP_SENTENCE
            )
        }
        Some(name) => match robots
            .iter()
            .find(|robot| robot.name == name && !robot.threads.is_empty())
        {
            Some(robot) => {
                let threads: Vec<String> = robot.threads.iter().map(thread_text).collect();
                format!(
                    "No thread binding was found for this session or worktree. This session's robot is {}. Its threads are listed below, one per line: thread id, mission, date of the latest pause, latest first.\n{}\nUse the AskUserQuestion tool to ask which thread to resume: offer the latest threads, up to three, for /tsk:resume-thread <thread-id>, and a new thread on a mission with /tsk:start-thread <mission-id>, and leave free text open. {}\n{}",
                    name,
                    threads.join("\n"),
                    missions_part(mission_lines),
                    SKIP_SENTENCE
                )
            }
            None => unbound_prompt(mission_lines),
        },
        None => unbound_prompt(mission_lines),
    }
}

/// [`robot_first_prompt`] for this session, read from its ledger store.
pub fn session_unbound_prompt(session: &Session, mission_lines: &[String]) -> String {
    let current = crate::actor::current(session)
        .ok()
        .flatten()
        .map(|actor| actor.name);
    match robots(session.store.as_ref()) {
        Ok(robots) => robot_first_prompt(current.as_deref(), &robots, mission_lines),
        Err(_) => unbound_prompt(mission_lines),
    }
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

/// This session's binding. A cloud session is looked up by its session ID. A local code
/// worktree is looked up by its actor key. A worktree that has no row but still has the
/// older marker file gets a row from the file, and the file is removed: a local write,
/// pushed with the next push. With no ledger store yet the marker is read as it is.
pub fn resolve(session: &Session) -> Result<Option<Binding>, String> {
    resolve_with(session, true)
}

/// [`resolve`] for a command that reads the ledger store as it is and never fetches or
/// writes: it reads a marker file but does not migrate it. `None` when the store does not
/// exist.
pub fn resolve_local(session: &Session) -> Result<Option<Binding>, String> {
    if session.store.exists_locally()? {
        resolve_with(session, false)
    } else {
        Ok(None)
    }
}

fn binding_of(row: &BindingRow) -> Option<Binding> {
    if row.thread_id.is_empty() {
        return None;
    }
    Some(match row.kind {
        BindingKind::Cloud => Binding::Cloud(row.thread_id.clone()),
        BindingKind::Worktree => Binding::Worktree(row.thread_id.clone()),
    })
}

fn resolve_with(session: &Session, migrate: bool) -> Result<Option<Binding>, String> {
    if session.cloud_session.is_some() {
        let Some((actor, _)) = session.actor_key()? else {
            return Ok(None);
        };
        return Ok(session
            .store
            .bindings_list()?
            .iter()
            .find(|row| row.instance == actor)
            .and_then(binding_of));
    }
    let marker = read_marker(&session.marker_path())?;
    let from_marker = || marker.clone().map(Binding::Worktree);
    let actor = match session.actor_key()? {
        Some((actor, _)) if session.store.exists_locally()? => actor,
        _ => return Ok(from_marker()),
    };
    let rows = match session.store.bindings_list() {
        Ok(rows) => rows,
        Err(_) if marker.is_some() => return Ok(from_marker()),
        Err(e) => return Err(e),
    };
    if let Some(row) = rows.iter().find(|row| row.instance == actor) {
        if marker.is_some() && migrate {
            // The row is the binding. A marker beside it is left over from before.
            let _ = remove_marker(session);
        }
        return Ok(binding_of(row));
    }
    if let (Some(id), true) = (&marker, migrate) {
        let row = BindingRow {
            instance: actor,
            kind: BindingKind::Worktree,
            thread_id: id.clone(),
            registered_at: utc_now(),
        };
        // A ledger that refuses the write keeps the marker, and the binding still reads.
        if session.store.binding_set(&row).is_ok() {
            let _ = remove_marker(session);
        }
    }
    Ok(from_marker())
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

/// Bind this session or worktree to a thread, replacing its earlier binding. A local
/// bind is a ledger write: the caller pushes it. A marker file left over from before is
/// removed.
pub fn bind_current(session: &Session, thread_id: &str) -> Result<(), String> {
    let (actor, kind) = session.actor_key()?.ok_or_else(no_clone_id)?;
    session.store.binding_set(&BindingRow {
        instance: actor,
        kind,
        thread_id: thread_id.to_string(),
        registered_at: utc_now(),
    })?;
    if kind == BindingKind::Worktree {
        remove_marker(session)?;
    }
    Ok(())
}

/// Remove this session's or worktree's binding, and a marker file left over from before.
/// Returns the kind it removed.
pub fn unbind_current(session: &Session) -> Result<BindingKind, String> {
    let (actor, kind) = session.actor_key()?.ok_or_else(no_clone_id)?;
    session.store.binding_remove(&actor)?;
    if kind == BindingKind::Worktree {
        remove_marker(session)?;
    }
    Ok(kind)
}

pub fn no_clone_id() -> String {
    "error: this clone has no clone id yet, so it has no ledger store to hold a binding. Run tsk ledger fetch first"
        .to_string()
}

/// The missions not done, as `tsk mission list --porcelain` prints them. The lines stay
/// tab-separated: they are context for the agent, not for a terminal. Do not align them.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn robot(name: &str, threads: &[(&str, &str, &str)]) -> Robot {
        Robot {
            name: name.to_string(),
            threads: threads
                .iter()
                .map(|(id, mission, latest)| RobotThread {
                    id: id.to_string(),
                    mission: Some(mission.to_string()),
                    latest: Some(latest.to_string()),
                })
                .collect(),
        }
    }

    #[test]
    fn with_no_robot_for_the_session_the_prompt_asks_for_the_robot_first() {
        let robots = vec![
            robot(
                "Credit Where It's Due",
                &[("e6b3c55c", "M-ENV", "2026-11-13T18:32:32Z")],
            ),
            robot("Noted, Not Expanded", &[]),
        ];
        let missions = vec!["M-ENV\tin-progress\t-\t-\tEnvironment".to_string()];
        let text = robot_first_prompt(None, &robots, &missions);
        assert!(text.contains("no robot is registered for it. Choose the robot first."));
        assert!(text.contains("Credit Where It's Due\te6b3c55c M-ENV 2026-11-13\n"));
        assert!(text.contains("Noted, Not Expanded\tno threads"));
        assert!(text.contains("tsk actor register \"<name>\" --kind robot"));
        assert!(text.contains("/tsk:construct-robot"));
        assert!(text.contains("M-ENV\tin-progress"));
        assert!(text.ends_with(SKIP_SENTENCE));
    }

    #[test]
    fn a_robot_with_threads_is_offered_its_threads() {
        let robots = vec![robot(
            "Credit Where It's Due",
            &[
                ("e6b3c55c", "M-ENV", "2026-11-13T18:32:32Z"),
                ("01d143d5", "M-ADHOC", "2026-11-01T10:00:00Z"),
            ],
        )];
        let text = robot_first_prompt(Some("Credit Where It's Due"), &robots, &[]);
        assert!(text.contains("This session's robot is Credit Where It's Due."));
        assert!(text.contains("e6b3c55c M-ENV 2026-11-13\n01d143d5 M-ADHOC 2026-11-01"));
        assert!(text.contains("/tsk:resume-thread <thread-id>"));
    }

    #[test]
    fn with_no_robots_or_no_threads_the_prompt_offers_the_missions() {
        let missions = vec!["M-ENV\tin-progress\t-\t-\tEnvironment".to_string()];
        assert_eq!(
            robot_first_prompt(None, &[], &missions),
            unbound_prompt(&missions)
        );
        let robots = vec![robot("Lone", &[])];
        assert_eq!(
            robot_first_prompt(Some("Lone"), &robots, &missions),
            unbound_prompt(&missions)
        );
    }

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
