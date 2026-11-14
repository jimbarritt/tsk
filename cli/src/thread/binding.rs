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
    /// Who holds the thread, as a person reads it, when a session holds it. `None` when the
    /// thread is paused (ADR 0032).
    pub held_by: Option<String>,
    /// The name of the robot that holds the thread, when a session holds it and its actor is known.
    pub holder_robot: Option<String>,
}

/// The thread a robot holds: a start or resume of it after its latest pause (ADR 0032).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holds {
    pub thread: String,
    pub mission: Option<String>,
}

/// A registered robot and its threads, latest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Robot {
    pub name: String,
    pub threads: Vec<RobotThread>,
    /// The harness instances that ran as the robot: its own registrations, and the
    /// instances of the sessions it was the primary actor of.
    pub instances: Vec<String>,
    /// The thread it holds. A robot that holds a thread is busy.
    pub holds: Option<Holds>,
}

impl Robot {
    /// Busy while it holds a thread, as `busy (holds <thread id> <mission>)`. Free otherwise.
    pub fn state(&self) -> RobotState {
        match &self.holds {
            Some(holds) => RobotState::Busy(holds.render()),
            None => RobotState::Free,
        }
    }

    /// The missions of its threads: the held thread's mission first, then the others latest
    /// first, each once. A thread with no mission adds none.
    pub fn missions(&self) -> Vec<String> {
        let held = self.holds.as_ref().and_then(|h| h.mission.clone());
        let mut missions: Vec<String> = Vec::new();
        for mission in held
            .into_iter()
            .chain(self.threads.iter().filter_map(|t| t.mission.clone()))
        {
            if !missions.contains(&mission) {
                missions.push(mission);
            }
        }
        missions
    }
}

impl Holds {
    fn render(&self) -> String {
        format!(
            "holds {} {}",
            self.thread,
            self.mission.as_deref().unwrap_or("no-mission")
        )
    }
}

/// The part of a session or instance key that a cloud session ID and its harness instance
/// share: `session_X`, `cse_X` and `cloud:cse_X` all give `X`.
pub fn session_key(id: &str) -> &str {
    let id = id.rsplit(':').next().unwrap_or(id);
    id.strip_prefix("session_")
        .or_else(|| id.strip_prefix("cse_"))
        .unwrap_or(id)
}

/// Whether a robot is busy, as `tsk actor list` shows it. A robot is busy while it holds a
/// thread, which the ledger alone says (ADR 0032).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RobotState {
    Free,
    /// Busy, with what it holds: `holds <thread id> <mission or no-mission>`.
    Busy(String),
}

impl RobotState {
    pub fn render(&self) -> String {
        match self {
            RobotState::Free => "free".to_string(),
            RobotState::Busy(holds) => format!("busy ({})", holds),
        }
    }
}

/// One robot as `tsk actor list` shows it: its state and its missions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotStatus {
    pub name: String,
    pub state: RobotState,
    pub missions: Vec<String>,
}

/// The state and missions of each robot, from the ledger.
pub fn robot_states(
    store: &dyn crate::ledger::store::LedgerStore,
) -> Result<Vec<RobotStatus>, String> {
    Ok(robots(store)?
        .iter()
        .map(|robot| RobotStatus {
            name: robot.name.clone(),
            state: robot.state(),
            missions: robot.missions(),
        })
        .collect())
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
                    held_by: row.holder.as_ref().map(super::ops::Holder::label),
                    holder_robot: row.holder.as_ref().and_then(|h| h.robot.clone()),
                })
                .collect();
            threads.sort_by(|a, b| b.latest.cmp(&a.latest));
            let mut instances: Vec<String> = registrations
                .iter()
                .filter(|row| row.actor_name == actor.name)
                .map(|row| row.instance.clone())
                .collect();
            for row in session_actors.iter().filter(|row| {
                Some(row.actor_uid.as_str()) == actor.uid.as_deref()
                    && row.role == crate::ledger::store::ActorRole::Primary
            }) {
                if let Some(session) = sessions.iter().find(|s| s.session == row.session) {
                    if !instances.contains(&session.instance) {
                        instances.push(session.instance.clone());
                    }
                }
            }
            let holds = threads
                .iter()
                .find(|t| t.holder_robot.as_deref() == Some(actor.name.as_str()))
                .map(|t| Holds {
                    thread: t.id.clone(),
                    mission: t.mission.clone(),
                });
            Robot {
                name: actor.name,
                threads,
                instances,
                holds,
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
    let held = match &thread.held_by {
        Some(holder) => format!(" held by {}", holder),
        None => String::new(),
    };
    format!(
        "{} {} {}{}",
        thread.id,
        thread.mission.as_deref().unwrap_or("no-mission"),
        date,
        held
    )
}

/// One line of the robot list: name, free or `busy (holds <thread id> <mission>)`, then its
/// threads, with the state of each. The columns are tab-separated.
fn robot_line(robot: &Robot) -> String {
    let threads: Vec<String> = robot.threads.iter().map(thread_text).collect();
    format!(
        "{}\t{}\t{}",
        robot.name,
        robot.state().render(),
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

/// Every robot, the free robots first, then the busy robots, each group in the order of
/// `robots` (the latest thread first). A busy robot stays in the list: it takes a new thread
/// or no thread (ADR 0032).
fn ordered_robots(robots: &[Robot]) -> Vec<&Robot> {
    let (free, busy): (Vec<_>, Vec<_>) = robots.iter().partition(|robot| robot.holds.is_none());
    free.into_iter().chain(busy).collect()
}

/// The prompt for a session that is bound to a thread and has no robot: which robot this
/// session is. The free robots come first, then the busy robots, each marked.
pub fn robot_only_prompt(thread_id: &str, robots: &[Robot]) -> String {
    let names: Vec<String> = ordered_robots(robots)
        .iter()
        .map(|robot| match robot.state() {
            RobotState::Free => robot.name.clone(),
            busy => format!("{} [{}]", robot.name, busy.render()),
        })
        .collect();
    format!(
        "This session is bound to thread {} and has no robot. The robots, free robots first, then busy robots: {}. A robot works in any number of sessions, so a busy robot is marked and not removed. Use the AskUserQuestion tool to ask which robot this session is: up to three of them and \"Construct a new robot\", with free text open. Then run `tsk actor register \"<name>\" --kind robot`, or /tsk:construct-robot for a new one, and call set_session_title with the Session title line it prints.",
        thread_id,
        if names.is_empty() {
            "none".to_string()
        } else {
            names.join(", ")
        }
    )
}

fn choose_robot_prompt(robots: &[Robot], mission_lines: &[String]) -> String {
    let lines: Vec<String> = ordered_robots(robots).into_iter().map(robot_line).collect();
    format!(
        "No thread binding was found for this session or worktree, and no robot is registered for it. Choose the robot first. Every robot is listed below, one per line: name, free or busy (holds <thread id> <mission>), then its threads (thread id, mission, date of the latest pause, and held by <robot> when a session holds the thread), tab-separated. A robot is busy while it holds a thread. The free robots come first, the robot with the latest thread first, then the busy robots.\n{}\nRules. A busy robot takes a new thread or no thread. Any other robot can resume a paused thread, whichever robot worked it last. A held thread is not offered: do not offer a thread marked held by <robot>.\nAsk both choices in one AskUserQuestion call with two questions, and leave free text open on each. Question 1, which robot this session is: up to three free robots from the list, the one with the latest thread first, then \"Construct a new robot\", then a busy robot only when no free robot suits. Question 2, what to work on: the latest paused thread of each free robot offered, labelled with the robot (\"<robot>: resume <thread id> <mission>\"), then missions from the list to start a new thread, and \"No thread\", four options at most. A busy robot gets missions and \"No thread\" only. Once answered: run `tsk actor register \"<name>\" --kind robot` for a robot from the list, or /tsk:construct-robot for a new one. Then run /tsk:resume-thread <thread-id> for a paused thread, /tsk:start-thread <mission-id> for a mission, or `tsk thread skip` for no thread. Each of these commands prints the session title line in a cloud session: call set_session_title with it. {}\n{}",
        lines.join("\n"),
        missions_part(mission_lines),
        SKIP_SENTENCE
    )
}

/// The prompt for an unbound session, robot first. With no robot registered for this
/// session and robots in the ledger, it asks which robot this session is, from every robot,
/// the free ones first. With a robot that has threads, it offers them. Otherwise it is
/// [`unbound_prompt`].
pub fn robot_first_prompt(
    current: Option<&str>,
    robots: &[Robot],
    mission_lines: &[String],
) -> String {
    match current {
        None if !robots.is_empty() => choose_robot_prompt(robots, mission_lines),
        Some(name) => match robots
            .iter()
            .find(|robot| robot.name == name && !robot.threads.is_empty())
        {
            Some(robot) => {
                let threads: Vec<String> = robot.threads.iter().map(thread_text).collect();
                format!(
                    "No thread binding was found for this session or worktree. This session's robot is {}. Its threads are listed below, one per line: thread id, mission, date of the latest pause, and held by <robot> when a session holds the thread, latest first. A held thread is not offered: do not offer a thread marked held by <robot>.\n{}\nUse the AskUserQuestion tool to ask which thread to resume: offer the latest paused threads, up to three, for /tsk:resume-thread <thread-id>, and a new thread on a mission with /tsk:start-thread <mission-id>, and leave free text open. {}\n{}",
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

/// [`robot_only_prompt`] for a bound session with no robot, or `None` when it has a
/// robot or the ledger has none.
pub fn bound_robot_prompt(session: &Session, thread_id: &str) -> Option<String> {
    if crate::actor::current(session).ok().flatten().is_some() {
        return None;
    }
    let robots = robots(session.store.as_ref()).ok()?;
    if robots.is_empty() {
        return None;
    }
    Some(robot_only_prompt(thread_id, &robots))
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
    bind_current_at(session, thread_id, utc_now())
}

/// [`bind_current`] with the time the binding is registered at.
pub fn bind_current_at(
    session: &Session,
    thread_id: &str,
    registered_at: String,
) -> Result<(), String> {
    let (actor, kind) = session.actor_key()?.ok_or_else(no_clone_id)?;
    session.store.binding_set(&BindingRow {
        instance: actor,
        kind,
        thread_id: thread_id.to_string(),
        registered_at,
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
            instances: vec![format!("cse_{}", name.len())],
            holds: None,
            threads: threads
                .iter()
                .map(|(id, mission, latest)| RobotThread {
                    id: id.to_string(),
                    mission: Some(mission.to_string()),
                    latest: Some(latest.to_string()),
                    held_by: None,
                    holder_robot: None,
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
        assert!(text.contains("Credit Where It's Due\tfree\te6b3c55c M-ENV 2026-11-13\n"));
        assert!(text.contains("Noted, Not Expanded\tfree\tno threads"));
        assert!(text.contains("tsk actor register \"<name>\" --kind robot"));
        assert!(text.contains("/tsk:construct-robot"));
        assert!(text.contains("in one AskUserQuestion call with two questions"));
        assert!(text.contains("M-ENV\tin-progress"));
        assert!(text.ends_with(SKIP_SENTENCE));
    }

    /// A robot that holds the thread `id` of mission `mission`.
    fn holding(mut robot: Robot, id: &str, mission: &str) -> Robot {
        robot.holds = Some(Holds {
            thread: id.to_string(),
            mission: Some(mission.to_string()),
        });
        robot
    }

    #[test]
    fn a_robot_that_holds_a_thread_is_listed_after_the_free_robots_and_marked_busy() {
        // The busy robot has the latest thread, so it would come first by date.
        let busy = holding(
            robot("Busy", &[("aaaaaaaa", "M-A", "2026-11-13T10:00:00Z")]),
            "aaaaaaaa",
            "M-A",
        );
        let free = robot("Free", &[("bbbbbbbb", "M-B", "2026-11-12T10:00:00Z")]);
        let robots = vec![busy, free];

        let text = robot_first_prompt(None, &robots, &[]);
        let free_at = text.find("Free\tfree\tbbbbbbbb M-B 2026-11-12").unwrap();
        let busy_at = text
            .find("Busy\tbusy (holds aaaaaaaa M-A)\taaaaaaaa M-A 2026-11-13")
            .unwrap();
        assert!(free_at < busy_at, "{}", text);
        assert!(!text.contains("not offered: Busy"), "{}", text);
        assert!(
            text.contains("A busy robot takes a new thread or no thread."),
            "{}",
            text
        );
        assert!(
            text.contains("Any other robot can resume a paused thread"),
            "{}",
            text
        );
        assert!(
            text.contains("do not offer a thread marked held by"),
            "{}",
            text
        );
        assert!(!text.contains("list_sessions"), "{}", text);
        assert!(!text.contains("--open"), "{}", text);

        // With every robot busy the list stays whole and a new robot is still offered.
        let all = vec![
            holding(robot("Free", &[]), "cccccccc", "M-C"),
            holding(robot("Busy", &[]), "dddddddd", "M-D"),
        ];
        let none = robot_first_prompt(None, &all, &[]);
        assert!(none.contains("Free\tbusy (holds cccccccc M-C)"), "{}", none);
        assert!(none.contains("Busy\tbusy (holds dddddddd M-D)"), "{}", none);
        assert!(none.contains("\"Construct a new robot\""), "{}", none);
        assert!(none.contains("/tsk:construct-robot"), "{}", none);
    }

    #[test]
    fn a_held_thread_is_marked_and_the_prompt_says_not_to_offer_it() {
        let mut one = robot("One", &[("aaaaaaaa", "M-A", "2026-11-13T10:00:00Z")]);
        one.threads[0].held_by = Some("Two (cse_01AB)".to_string());
        let text = robot_first_prompt(None, &[one.clone()], &[]);
        assert!(
            text.contains("One\tfree\taaaaaaaa M-A 2026-11-13 held by Two (cse_01AB)"),
            "{}",
            text
        );
        assert!(text.contains("A held thread is not offered"), "{}", text);

        let own = robot_first_prompt(Some("One"), &[one], &[]);
        assert!(
            own.contains("aaaaaaaa M-A 2026-11-13 held by Two (cse_01AB)"),
            "{}",
            own
        );
        assert!(own.contains("A held thread is not offered"), "{}", own);
    }

    #[test]
    fn a_bound_session_with_no_robot_is_offered_every_robot_free_ones_first() {
        let busy = holding(robot("Busy", &[]), "aaaaaaaa", "M-A");
        let robots = vec![busy, robot("Free", &[])];
        let text = robot_only_prompt("e6b3c55c", &robots);
        assert!(text.starts_with("This session is bound to thread e6b3c55c and has no robot."));
        assert!(
            text.contains("then busy robots: Free, Busy [busy (holds aaaaaaaa M-A)]."),
            "{}",
            text
        );
        assert!(!text.contains("--open"), "{}", text);
    }

    #[test]
    fn a_robot_states_are_free_or_busy_and_its_missions_list_the_held_one_first() {
        let mut one = holding(
            robot(
                "One",
                &[
                    ("aaaaaaaa", "M-A", "2026-11-13T10:00:00Z"),
                    ("bbbbbbbb", "M-B", "2026-11-12T10:00:00Z"),
                    ("cccccccc", "M-A", "2026-11-11T10:00:00Z"),
                ],
            ),
            "bbbbbbbb",
            "M-B",
        );
        assert_eq!(one.state().render(), "busy (holds bbbbbbbb M-B)");
        assert_eq!(one.missions(), vec!["M-B".to_string(), "M-A".to_string()]);
        one.holds = Some(Holds {
            thread: "dddddddd".to_string(),
            mission: None,
        });
        assert_eq!(one.state().render(), "busy (holds dddddddd no-mission)");
        one.holds = None;
        assert_eq!(one.state().render(), "free");
        assert_eq!(one.missions(), vec!["M-A".to_string(), "M-B".to_string()]);
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
        let text = robot_first_prompt(Some("Credit Where It's Due"), &robots, &[]);
        assert!(text.contains("This session's robot is Credit Where It's Due."));
        assert!(text.contains("e6b3c55c M-ENV 2026-11-13\n01d143d5 M-ADHOC 2026-11-01"));
        assert!(text.contains("/tsk:resume-thread <thread-id>"));
        assert!(text.contains("do not offer a thread marked held by"));
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
