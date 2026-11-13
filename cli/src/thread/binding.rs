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
    /// The harness instances that ran as the robot: its own registrations, and the
    /// instances of the sessions it was the primary actor of.
    pub instances: Vec<String>,
    /// The URNs of the sessions it was the primary actor of.
    pub sessions: Vec<String>,
}

/// The part of a session or instance key that a cloud session ID and its harness instance
/// share: `session_X`, `cse_X` and `cloud:cse_X` all give `X`.
pub fn session_key(id: &str) -> &str {
    let id = id.rsplit(':').next().unwrap_or(id);
    id.strip_prefix("session_")
        .or_else(|| id.strip_prefix("cse_"))
        .unwrap_or(id)
}

impl Robot {
    /// True when one of the robot's instances or sessions is one of the `open` sessions.
    pub fn is_open(&self, open: &[String]) -> bool {
        self.instances
            .iter()
            .chain(self.sessions.iter())
            .any(|key| open.iter().any(|id| session_key(id) == session_key(key)))
    }
}

/// The registered robots, the one with the latest thread first and robots with no thread
/// last, from the actors and the thread bindings.
pub fn robots(store: &dyn crate::ledger::store::LedgerStore) -> Result<Vec<Robot>, String> {
    let rows = super::ops::list_rows(store)?;
    let registrations = store.instances_list()?;
    let sessions = store.sessions_list()?;
    let session_actors = store.session_actors_list()?;
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
            let mut sessions_of: Vec<String> = Vec::new();
            let mut instances: Vec<String> = registrations
                .iter()
                .filter(|row| row.actor_name == actor.name)
                .map(|row| row.instance.clone())
                .collect();
            for row in session_actors.iter().filter(|row| {
                Some(row.actor_uid.as_str()) == actor.uid.as_deref()
                    && row.role == crate::ledger::store::ActorRole::Primary
            }) {
                sessions_of.push(row.session.clone());
                if let Some(session) = sessions.iter().find(|s| s.session == row.session) {
                    if !instances.contains(&session.instance) {
                        instances.push(session.instance.clone());
                    }
                }
            }
            Robot {
                name: actor.name,
                threads,
                instances,
                sessions: sessions_of,
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

/// Which sessions are open, for the robot choice: a robot whose session is open is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenSessions<'a> {
    /// Outside a cloud session: the open sessions on this machine (crate::live), and the
    /// robots that tmux panes running claude show.
    Local {
        open: &'a [String],
        tmux: &'a [String],
    },
    /// A cloud session with no list yet: the prompt asks the agent for one.
    NotAsked,
    /// The IDs of the sessions that are not archived.
    Listed(&'a [String]),
}

/// The text that tells the agent to list the open cloud sessions and run
/// `tsk thread next --open` with them.
pub const OPEN_SESSIONS_STEP: &str = "No thread binding was found for this session, and no robot is registered for it. Choose the robot first, from the robots that no open session uses. Call the list_sessions tool of the claude-code-remote server (mine: true, limit: 30), take the id of every session whose session_status is not SESSION_STATUS_ARCHIVED, and run `tsk thread next --open <id>,<id>,...` with them. It prints the robots to offer.";

fn choose_robot_prompt(robots: &[Robot], mission_lines: &[String], open: OpenSessions) -> String {
    let (free, taken): (Vec<&Robot>, Vec<&Robot>) = match open {
        OpenSessions::NotAsked => return OPEN_SESSIONS_STEP.to_string(),
        OpenSessions::Listed(ids) => robots.iter().partition(|robot| !robot.is_open(ids)),
        OpenSessions::Local { open, tmux } => robots
            .iter()
            .partition(|robot| !robot.is_open(open) && !tmux.contains(&robot.name)),
    };
    let taken_part = if taken.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = taken.iter().map(|robot| robot.name.as_str()).collect();
        format!(
            " Taken by a session that is not archived, so not offered: {}.",
            names.join(", ")
        )
    };
    if free.is_empty() {
        return format!(
            "No thread binding was found for this session, and no robot is registered for it. Every registered robot is taken by a session that is not archived.{} Run /tsk:construct-robot to construct a new robot, then /tsk:start-thread <mission-id>. {}\n{}",
            taken_part,
            missions_part(mission_lines),
            SKIP_SENTENCE
        );
    }
    let lines: Vec<String> = free.iter().map(|robot| robot_line(robot)).collect();
    format!(
        "No thread binding was found for this session or worktree, and no robot is registered for it. Choose the robot first. The robots to offer are listed below, one per line: name, then its threads (thread id, mission, date of the latest pause), tab-separated, the robot with the latest thread first.\n{}\n{}Ask both choices in one AskUserQuestion call with two questions, and leave free text open on each. Question 1, which robot this session is: up to three robots from the list, the one with the latest thread first, and \"Construct a new robot\". Question 2, what to work on: the latest thread of each robot offered, labelled with the robot (\"<robot>: resume <thread id> <mission>\"), then missions from the list to start a new thread, and \"No thread\", four options at most. Once answered: run `tsk actor register \"<name>\" --kind robot` for a robot from the list, or /tsk:construct-robot for a new one. Then run /tsk:resume-thread <thread-id> for a thread, /tsk:start-thread <mission-id> for a mission, or `tsk thread skip` for no thread. If the thread chosen belongs to another robot than the one chosen, ask again which thread. Each of these commands prints the session title line in a cloud session: call set_session_title with it. {}\n{}",
        lines.join("\n"),
        if taken_part.is_empty() {
            String::new()
        } else {
            format!("{}\n", taken_part.trim_start())
        },
        missions_part(mission_lines),
        SKIP_SENTENCE
    )
}

/// The prompt for an unbound session, robot first. With no robot registered for this
/// session and robots in the ledger, it asks which robot this session is, from the robots
/// no open session uses. With a robot that has threads, it offers them. Otherwise it is
/// [`unbound_prompt`].
pub fn robot_first_prompt(
    current: Option<&str>,
    robots: &[Robot],
    mission_lines: &[String],
    open: OpenSessions,
) -> String {
    match current {
        None if !robots.is_empty() => choose_robot_prompt(robots, mission_lines, open),
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

/// [`robot_first_prompt`] for this session, read from its ledger store. `open` lists the
/// cloud sessions that are not archived, when the agent passed them.
pub fn session_unbound_prompt(
    session: &Session,
    mission_lines: &[String],
    open: Option<&[String]>,
) -> String {
    let current = crate::actor::current(session)
        .ok()
        .flatten()
        .map(|actor| actor.name);
    let (local_open, tmux) = if open.is_none() && !super::session_start::is_remote() {
        let root = crate::ledger::location::state_root_from_env().ok();
        (
            root.map(|r| crate::live::open_session_ids(&r))
                .unwrap_or_default(),
            crate::live::tmux_robots(),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let open = match open {
        Some(ids) => OpenSessions::Listed(ids),
        None if super::session_start::is_remote() => OpenSessions::NotAsked,
        None => OpenSessions::Local {
            open: &local_open,
            tmux: &tmux,
        },
    };
    match robots(session.store.as_ref()) {
        Ok(robots) => robot_first_prompt(current.as_deref(), &robots, mission_lines, open),
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

    const NONE_OPEN: OpenSessions<'static> = OpenSessions::Local {
        open: &[],
        tmux: &[],
    };

    fn robot(name: &str, threads: &[(&str, &str, &str)]) -> Robot {
        Robot {
            name: name.to_string(),
            instances: vec![format!("cse_{}", name.len())],
            sessions: Vec::new(),
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
        let text = robot_first_prompt(None, &robots, &missions, NONE_OPEN);
        assert!(text.contains("no robot is registered for it. Choose the robot first."));
        assert!(text.contains("Credit Where It's Due\te6b3c55c M-ENV 2026-11-13\n"));
        assert!(text.contains("Noted, Not Expanded\tno threads"));
        assert!(text.contains("tsk actor register \"<name>\" --kind robot"));
        assert!(text.contains("/tsk:construct-robot"));
        assert!(text.contains("in one AskUserQuestion call with two questions"));
        assert!(text.contains("M-ENV\tin-progress"));
        assert!(text.ends_with(SKIP_SENTENCE));
    }

    #[test]
    fn a_robot_with_an_open_session_is_not_offered() {
        let mut busy = robot("Busy", &[("aaaaaaaa", "M-A", "2026-11-13T10:00:00Z")]);
        busy.instances = vec!["cse_01OPEN".to_string()];
        let mut free = robot("Free", &[("bbbbbbbb", "M-B", "2026-11-12T10:00:00Z")]);
        free.instances = vec!["cse_01DONE".to_string()];
        let robots = vec![busy, free];
        let open = vec!["session_01OPEN".to_string()];

        let text = robot_first_prompt(None, &robots, &[], OpenSessions::Listed(&open));
        assert!(text.contains("Free\tbbbbbbbb M-B 2026-11-12"), "{}", text);
        assert!(!text.contains("Busy\t"), "{}", text);
        assert!(text.contains("not offered: Busy."), "{}", text);

        let all = vec!["session_01OPEN".to_string(), "session_01DONE".to_string()];
        let none = robot_first_prompt(None, &robots, &[], OpenSessions::Listed(&all));
        assert!(none.contains("Every registered robot is taken"), "{}", none);
        assert!(none.contains("/tsk:construct-robot"), "{}", none);

        assert_eq!(
            robot_first_prompt(None, &robots, &[], OpenSessions::NotAsked),
            OPEN_SESSIONS_STEP
        );
    }

    #[test]
    fn in_a_terminal_open_sessions_and_tmux_panes_take_robots() {
        let mut by_session = robot("Session Robot", &[]);
        by_session.sessions = vec!["urn:tsk:session:claude:5f2c-a1".to_string()];
        let tmux_robot = robot("Pane Robot", &[]);
        let free = robot("Free Robot", &[]);
        let robots = vec![by_session, tmux_robot, free];
        let open = vec!["5f2c-a1".to_string()];
        let tmux = vec!["Pane Robot".to_string()];

        let text = robot_first_prompt(
            None,
            &robots,
            &[],
            OpenSessions::Local {
                open: &open,
                tmux: &tmux,
            },
        );
        assert!(text.contains("Free Robot\tno threads"), "{}", text);
        assert!(!text.contains("Session Robot\t"), "{}", text);
        assert!(!text.contains("Pane Robot\t"), "{}", text);
        assert!(
            text.contains("not offered: Session Robot, Pane Robot."),
            "{}",
            text
        );
    }

    #[test]
    fn session_keys_match_across_the_id_forms() {
        assert_eq!(session_key("session_01ABC"), "01ABC");
        assert_eq!(session_key("cse_01ABC"), "01ABC");
        assert_eq!(session_key("urn:tsk:instance:cloud:cse_01ABC"), "01ABC");
        assert_eq!(session_key("laptop-tsk"), "laptop-tsk");
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
        let text = robot_first_prompt(Some("Credit Where It's Due"), &robots, &[], NONE_OPEN);
        assert!(text.contains("This session's robot is Credit Where It's Due."));
        assert!(text.contains("e6b3c55c M-ENV 2026-11-13\n01d143d5 M-ADHOC 2026-11-01"));
        assert!(text.contains("/tsk:resume-thread <thread-id>"));
    }

    #[test]
    fn with_no_robots_or_no_threads_the_prompt_offers_the_missions() {
        let missions = vec!["M-ENV\tin-progress\t-\t-\tEnvironment".to_string()];
        assert_eq!(
            robot_first_prompt(None, &[], &missions, NONE_OPEN),
            unbound_prompt(&missions)
        );
        let robots = vec![robot("Lone", &[])];
        assert_eq!(
            robot_first_prompt(Some("Lone"), &robots, &missions, NONE_OPEN),
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
