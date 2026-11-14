use std::path::Path;

use serde::Serialize;

use crate::ledger::git::Git;
use crate::ledger::location::mint_token;
use crate::ledger::store::{BindingKind, BindingRow, LedgerStore, ThreadInfo};
use crate::mission::model::Mission;
use crate::mission::records::{resolve_task, task_label};

use super::binding::{
    bind_current, bind_current_at, clear_skip, remove_marker, resolve, unbind_current, Binding,
};
use super::clock::{format_utc, now_secs, parse_utc};
use super::entry::{self, parse_store, CodeState, GitState, LedgerState, NewEntry};
use super::session::Session;
use super::writers::Writers;

pub const THREAD_ID_LEN: usize = 8;
pub const MINT_ATTEMPTS: usize = 100;

pub enum StartOutcome {
    Started(String),
    ResumeRequired(String),
}

pub struct ResumeOutcome {
    pub thread_id: String,
    pub latest: String,
    pub warning: String,
    /// The thread's mission as `(id, name)`: its name comes from the mission record.
    pub mission: Option<(String, String)>,
    /// The task of the latest entry as `(id, description)`, when the mission has it.
    pub task: Option<(String, String)>,
    /// The cloud session ID in the `session_` form `set_session_title` takes, and the
    /// title, in a cloud session with a title.
    pub session_title: Option<(String, String)>,
    /// A note when the latest entry was written by an older tsk.
    pub version_note: Option<String>,
}

/// The cloud session ID that the session tools take: `CLAUDE_CODE_REMOTE_SESSION_ID`
/// holds `cse_X`, and `set_session_title` takes `session_X`.
pub fn cloud_session_id(cloud: &str) -> String {
    match cloud.strip_prefix("cse_") {
        Some(rest) => format!("session_{}", rest),
        None => cloud.to_string(),
    }
}

/// What a bound thread's mission and latest entry say, resolved against the mission
/// records, for the resume output and the session start context.
pub struct Facts {
    pub mission: Option<(String, String)>,
    pub task: Option<(String, String)>,
}

/// The mission name and the task description of a thread's latest entry. The mission is
/// the thread's own, else the one the entry names. The task is looked up in the mission the
/// entry names, else in the thread's own, and its label names that mission when it is not
/// the thread's own (a pause written before the thread changed mission). Either is None when
/// no record has it.
pub fn facts_of(info: &ThreadInfo, latest: Option<&entry::Entry>, missions: &[Mission]) -> Facts {
    let entry_mission = latest.and_then(|e| e.mission_id.clone());
    let mission_id = thread_mission(info).or_else(|| entry_mission.clone());
    let find = |id: &str| missions.iter().find(|m| m.id == id);
    let record = mission_id.as_deref().and_then(find);
    let task_record = entry_mission.as_deref().and_then(find).or(record);
    let task = task_record.and_then(|mission| {
        let wanted = latest?.task_id.as_deref().filter(|t| !t.is_empty())?;
        let found = mission
            .tasks
            .iter()
            .find(|t| task_label(t.seq).eq_ignore_ascii_case(wanted))?;
        let label = if record.is_some_and(|own| own.id == mission.id) {
            task_label(found.seq)
        } else {
            format!("{} {}", mission.id, task_label(found.seq))
        };
        Some((label, found.description.clone()))
    });
    Facts {
        mission: record.map(|m| (m.id.clone(), m.name.clone())),
        task,
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ListRow {
    pub id: String,
    /// `urn:tsk:thread:<ledger-id>:<thread-id>`; null on a ledger below layout 12.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urn: Option<String>,
    pub mission_id: Option<String>,
    pub mission_link: String,
    pub latest_whats_next: Option<String>,
    pub latest_timestamp: Option<String>,
    /// The actors bound to the thread, oldest binding first. Empty when none is.
    pub actors: Vec<ListActor>,
    /// `paused` or `held` (ADR 0032, decision 4).
    pub state: &'static str,
    /// The session that holds the thread, or null when it is paused.
    pub holder: Option<Holder>,
}

/// The harness instance that holds a thread: the one whose binding is later than the
/// latest pause of the thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Holder {
    /// The instance key of the holder.
    pub instance: String,
    /// The name of the actor behind the instance, or null when none is known.
    pub robot: Option<String>,
    /// The time the holder took the thread: the `registered_at` of its binding.
    pub since: String,
}

impl Holder {
    /// How a person reads the holder: `<robot> (<short instance>)`, or the short instance
    /// alone when no actor is known.
    pub fn label(&self) -> String {
        match &self.robot {
            Some(robot) => format!("{} ({})", robot, short_instance(&self.instance)),
            None => short_instance(&self.instance),
        }
    }
}

/// An instance key cut short for a person: a cloud session ID keeps its first eight
/// characters after the `cse_` prefix, and a worktree key (`<clone-id>/<name>`) stays as it is.
pub fn short_instance(instance: &str) -> String {
    if instance.contains('/') {
        return instance.to_string();
    }
    super::binding::session_key(instance)
        .chars()
        .take(8)
        .collect()
}

/// Whether a thread is paused or held (ADR 0032, decision 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadState {
    Paused,
    Held(Holder),
}

impl ThreadState {
    pub fn name(&self) -> &'static str {
        match self {
            ThreadState::Paused => "paused",
            ThreadState::Held(_) => "held",
        }
    }

    pub fn holder(&self) -> Option<&Holder> {
        match self {
            ThreadState::Paused => None,
            ThreadState::Held(holder) => Some(holder),
        }
    }
}

/// True when the time `a` is later than the time `b`. Two UTC timestamps compare as
/// instants. A value that is not a timestamp compares as text.
fn is_later(a: &str, b: &str) -> bool {
    match (parse_utc(a), parse_utc(b)) {
        (Some(a), Some(b)) => a > b,
        _ => a > b,
    }
}

/// The time given to a pause entry that has no timestamp. Such an entry was written before
/// timestamps and before ADR 0032, so it is a real pause, at a time no later than the day the
/// ADR took effect. A binding registered up to that time is not a holder.
pub const LEGACY_PAUSE_TIME: &str = "2026-11-14T00:00:00Z";

/// The time of the latest pause of a thread, or `None` when it has no entry. An entry with no
/// timestamp counts as a pause at [`LEGACY_PAUSE_TIME`].
pub fn latest_pause(entries: &[entry::StoredEntry]) -> Option<String> {
    entries
        .iter()
        .map(|stored| {
            stored
                .entry
                .timestamp
                .as_deref()
                .filter(|time| !time.is_empty())
                .unwrap_or(LEGACY_PAUSE_TIME)
        })
        .fold(None::<&str>, |latest, time| match latest {
            Some(latest) if !is_later(time, latest) => Some(latest),
            _ => Some(time),
        })
        .map(str::to_string)
}

/// The state of a thread from its bindings and the time of its latest pause. The holder is
/// the instance of the binding with the latest `registered_at`, when that time is later than
/// the latest pause (a thread with no pause and a binding is held). Otherwise the thread is
/// paused, and so is a thread with no binding. `robot_of` names the actor behind an instance.
pub fn thread_state(
    thread_id: &str,
    latest_pause: Option<&str>,
    bindings: &[BindingRow],
    robot_of: &dyn Fn(&str) -> Option<String>,
) -> ThreadState {
    let mut newest: Option<&BindingRow> = None;
    for row in bindings.iter().filter(|row| row.thread_id == thread_id) {
        if newest.is_none_or(|n| !is_later(&n.registered_at, &row.registered_at)) {
            newest = Some(row);
        }
    }
    match newest {
        Some(row) if latest_pause.is_none_or(|pause| is_later(&row.registered_at, pause)) => {
            ThreadState::Held(Holder {
                instance: row.instance.clone(),
                robot: robot_of(&row.instance),
                since: row.registered_at.clone(),
            })
        }
        _ => ThreadState::Paused,
    }
}

/// The state of a thread as the ledger holds it: see [`thread_state`].
fn thread_state_of(
    id: &str,
    entries: &[entry::StoredEntry],
    bindings: &[BindingRow],
    writers: &Writers,
) -> ThreadState {
    thread_state(
        id,
        latest_pause(entries).as_deref(),
        bindings,
        &|instance| {
            let writer = writers.resolve(instance);
            writer.actor_uid.map(|_| writer.label)
        },
    )
}

/// One actor bound to a thread, as `tsk thread list` prints it.
#[derive(Debug, PartialEq, Serialize)]
pub struct ListActor {
    pub actor: String,
    pub kind: &'static str,
    /// The name of the actor this harness instance registered as (`tsk actor register`),
    /// or null when it registered none.
    pub name: Option<String>,
}

fn check_thread_id(id: &str) -> Result<(), String> {
    if id.len() == THREAD_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
    {
        Ok(())
    } else {
        Err(format!(
            "error: invalid thread id '{}': expected {} characters from [0-9a-z]",
            id, THREAD_ID_LEN
        ))
    }
}

fn mint_id(
    seed: &str,
    location: &str,
    exists: &dyn Fn(&str) -> Result<bool, String>,
) -> Result<String, String> {
    for _ in 0..MINT_ATTEMPTS {
        let id = mint_token(THREAD_ID_LEN, seed);
        if !exists(&id)? {
            return Ok(id);
        }
    }
    Err(format!(
        "error: no free thread id after {} attempts; is {} intact?",
        MINT_ATTEMPTS, location
    ))
}

pub fn entries_of(store: &dyn LedgerStore, id: &str) -> Result<Vec<entry::StoredEntry>, String> {
    let lines = store.continuation_list(id)?;
    parse_store(&lines.join("\n"), &store.describe_thread(id))
}

/// The mission ID at the start of a briefing's file name: `M-` and the groups of capitals
/// and digits that follow, up to the slug. `M-BOOT-04-official-ledger.md` is `M-BOOT-04`.
pub fn mission_id_of_file(path: &str) -> Option<String> {
    let name = Path::new(path).file_name()?.to_str()?;
    let stem = name.strip_suffix(".md").unwrap_or(name);
    let groups: Vec<&str> = stem
        .split('-')
        .take_while(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
        .collect();
    (groups.len() >= 2 && groups[0] == "M").then(|| groups.join("-"))
}

/// The mission a thread works: the stored ID, else the ID in the file name of its
/// briefing path. A thread made before layout version 5 stores only the briefing path.
/// The derived ID is never written back.
pub fn thread_mission(info: &ThreadInfo) -> Option<String> {
    info.mission_id
        .clone()
        .or_else(|| info.briefing_path.as_deref().and_then(mission_id_of_file))
}

/// The session title for an actor and the mission its thread works: `<mission>: <name>`,
/// `NO-MISSION: <name>` with no mission, the mission alone with no actor, and `None` with
/// neither (GitHub issue #7).
/// The mission part of the title of a session whose thread has no mission, or that has no
/// thread.
pub const NO_MISSION_TITLE: &str = "NO-MISSION";

pub fn session_title(actor: Option<&str>, mission: Option<&str>) -> Option<String> {
    match (actor, mission) {
        (Some(name), Some(mission)) => Some(format!("{}: {}", mission, name)),
        (Some(name), None) => Some(format!("{}: {}", NO_MISSION_TITLE, name)),
        (None, Some(mission)) => Some(mission.to_string()),
        (None, None) => None,
    }
}

/// The briefing document of a mission that has no record: a document at
/// `missions/<category>/<ID>-<slug>.md` or `missions/<category>/<ID>.md`. The first in
/// path order when several match.
pub fn find_briefing(store: &dyn LedgerStore, mission_id: &str) -> Result<Option<String>, String> {
    let found = store.doc_list("missions/")?.into_iter().find(|path| {
        let parts: Vec<&str> = path.split('/').collect();
        parts.len() == 3 && mission_id_of_file(parts[2]).as_deref() == Some(mission_id)
    });
    // A ledger below layout 14 names the document as the upgrade will, so a thread that
    // stores the path keeps a path that exists once the files are renamed.
    match found {
        Some(path) => Ok(Some(store.doc_resolve(&path)?.unwrap_or(path))),
        None => Ok(None),
    }
}

pub fn no_such_mission(mission_id: &str) -> String {
    format!(
        "error: no such mission: {} (it has no mission record and no briefing document at missions/<category>/{}-<slug>.md)",
        mission_id, mission_id
    )
}

/// Check that a mission exists. A mission exists when it has a record or, in a ledger not
/// yet moved to records, a briefing document. Returns the briefing path of a mission with
/// no record, and `None` for a mission with a record.
fn checked_mission(store: &dyn LedgerStore, mission_id: &str) -> Result<Option<String>, String> {
    if store.missions_load()?.iter().any(|m| m.id == mission_id) {
        return Ok(None);
    }
    Ok(Some(
        find_briefing(store, mission_id)?.ok_or_else(|| no_such_mission(mission_id))?,
    ))
}

/// Mint a thread and bind this session or worktree to it. With a mission ID the thread
/// works that mission (an unknown ID is refused). With none the thread has no mission.
pub fn start(session: &Session, mission_id: Option<&str>) -> Result<StartOutcome, String> {
    let store = session.store.as_ref();
    // A start mints a thread of its own. It does not ask origin first: the record of the
    // last fetch stands in for what origin holds, and a push that finds origin moved is
    // rejected and retried after a fetch. When the checks fail on that older view, they
    // run again on a fresh one, so an error is always about what origin holds.
    store.trust_last_fetch();
    let checked = match checked_start(session, mission_id) {
        Ok(checked) => checked,
        Err(_) => {
            store.distrust_last_fetch();
            checked_start(session, mission_id)?
        }
    };
    let briefing = match checked {
        Started::Bound(id) => return Ok(StartOutcome::ResumeRequired(id)),
        Started::Free(briefing) => briefing,
    };
    let id = mint_id(
        &store.describe_threads(),
        &store.describe_threads(),
        &|id| store.thread_exists(id),
    )?;
    store.thread_create(&id, mission_id, briefing.as_deref())?;
    bind_current(session, &id)?;
    clear_skip(session)?;
    session.push(&match mission_id {
        Some(mission_id) => format!("Start thread {} for mission {}", id, mission_id),
        None => format!("Start thread {} with no mission", id),
    })?;
    Ok(StartOutcome::Started(id))
}

/// What the checks of a start found.
enum Started {
    /// This session or worktree is bound to the thread already.
    Bound(String),
    /// Free to start, with the briefing path of a mission that has no record.
    Free(Option<String>),
}

/// The refresh of a start, then its checks: no binding yet, and a mission that exists.
fn checked_start(session: &Session, mission_id: Option<&str>) -> Result<Started, String> {
    session.refresh()?;
    let store = session.store.as_ref();
    if let Some(binding) = resolve(session)? {
        return Ok(Started::Bound(binding.thread_id().to_string()));
    }
    let briefing = match mission_id {
        Some(mission_id) => checked_mission(store, mission_id)?,
        None => None,
    };
    Ok(Started::Free(briefing))
}

/// What `tsk thread set mission` did: the thread, and the mission it now works.
pub struct MissionSet {
    pub thread_id: String,
    pub mission_id: Option<String>,
}

/// Attach a mission to a thread, change it, or detach it (`mission_id` of `None`). The
/// thread is the one given, else the one this session or worktree is bound to. Earlier
/// continuation entries keep the mission they were written with.
pub fn set_mission(
    session: &Session,
    mission_id: Option<&str>,
    target: Option<&str>,
) -> Result<MissionSet, String> {
    session.refresh()?;
    let store = session.store.as_ref();
    let id = match target {
        Some(id) => id.to_string(),
        None => resolve(session)?
            .map(|b| b.thread_id().to_string())
            .ok_or_else(|| {
                "error: no thread binding found for this session or worktree, and no thread id given; nothing to change"
                    .to_string()
            })?,
    };
    check_thread_id(&id)?;
    if !store.thread_exists(&id)? {
        return Err(format!("error: no such thread: {}", id));
    }
    let briefing = match mission_id {
        Some(mission_id) => checked_mission(store, mission_id)?,
        None => None,
    };
    // A local write. It goes out with the next push (ADR 0019).
    store.thread_set_mission(&id, mission_id, briefing.as_deref())?;
    eprintln!("{}", NOT_PUBLISHED_NOTE);
    Ok(MissionSet {
        thread_id: id,
        mission_id: mission_id.map(str::to_string),
    })
}

/// The error for a task ID on a thread that has no mission.
pub fn no_mission_for_task(thread_id: &str) -> String {
    format!(
        "error: thread {} has no mission, so it has no tasks. Attach a mission with tsk thread set mission <mission-id>, or pass an empty task ID",
        thread_id
    )
}

/// The latest pause entry's what's next for a thread, if it has one.
pub fn latest_whats_next(store: &dyn LedgerStore, id: &str) -> Result<Option<String>, String> {
    Ok(entries_of(store, id)?
        .last()
        .and_then(|stored| stored.entry.whats_next.clone())
        .filter(|text| !text.trim().is_empty()))
}

/// The task to record: empty stays empty. A thread with no mission has no tasks, so a
/// task ID on it is refused. When the mission has a record, a task ID must
/// name one of its tasks, and `T-1` is written as `T-01`. A mission with no record has no
/// tasks to check against.
fn checked_task(
    missions: &[Mission],
    thread_id: &str,
    mission_id: Option<&str>,
    task_id: &str,
) -> Result<String, String> {
    if mission_id.is_none() && !task_id.is_empty() {
        return Err(no_mission_for_task(thread_id));
    }
    let record = mission_id.and_then(|id| missions.iter().find(|m| m.id == id));
    match record {
        Some(mission) if !task_id.is_empty() => resolve_task(mission, task_id),
        _ => Ok(task_id.to_string()),
    }
}

/// The time a pause is stamped with: now, or the latest binding time of the thread when that
/// is later than now. A resume registers its binding up to one second after the clock (see
/// [`binding_time`]), and a pause in that second must not be earlier than the binding, or
/// the thread would stay held after the pause.
pub fn pause_time(now: u64, thread_id: &str, bindings: &[BindingRow]) -> String {
    let latest_binding = bindings
        .iter()
        .filter(|row| row.thread_id == thread_id)
        .filter_map(|row| parse_utc(&row.registered_at))
        .max()
        .unwrap_or(0);
    format_utc(now.max(latest_binding))
}

pub fn pause(session: &Session, id: &str, task_id: &str, whats_next: &str) -> Result<(), String> {
    check_thread_id(id)?;
    let store = session.store.as_ref();
    // A pause writes a row of its own. It does not ask origin first: the record of the last
    // fetch stands in for what origin holds, and a push that finds origin moved is rejected
    // and retried after a fetch. When the checks below fail on that older view, they run
    // again on a fresh one, so an error is always about what origin holds.
    store.trust_last_fetch();
    let (info, task_id) = match checked_pause(session, id, task_id) {
        Ok(checked) => checked,
        Err(_) => {
            store.distrust_last_fetch();
            checked_pause(session, id, task_id)?
        }
    };
    let mission_id = thread_mission(&info);
    let ledger_commit = store.head_id()?;
    let code_git = &session.repo.git;
    let code_commit = code_git.run_line(["rev-parse", "--verify", "HEAD^{commit}"])?;
    let code_ref = match current_branch_ref(code_git) {
        Ok(code_ref) => code_ref,
        Err(e) if is_detached(code_git) => detached_ref(code_git, &code_commit).ok_or(e)?,
        Err(e) => return Err(e),
    };
    ensure_on_an_origin_branch(code_git, &code_ref, &code_commit)?;
    let timestamp = pause_time(now_secs(), id, &store.bindings_list()?);
    let written_by = session.written_by()?;
    // The write is made with the commit of the push, in one process.
    store.continuation_append_deferred(
        id,
        &NewEntry {
            mission_id: mission_id.as_deref(),
            mission_link: info.briefing_path.as_deref(),
            task_id: &task_id,
            whats_next,
            git: GitState {
                ledger: LedgerState {
                    commit: &ledger_commit,
                },
                code: CodeState {
                    r#ref: &code_ref,
                    commit: &code_commit,
                },
            },
            timestamp: &timestamp,
            written_by: &written_by,
            tsk_version: Some(env!("CARGO_PKG_VERSION")),
        }
        .to_json(),
    )?;
    session.push(&format!("Pause thread {}: {}", id, task_id))
}

/// The refresh of a pause, then the checks of the thread and the task against it: what the
/// thread stores, and the task to record.
fn checked_pause(
    session: &Session,
    id: &str,
    task_id: &str,
) -> Result<(ThreadInfo, String), String> {
    session.refresh()?;
    let store = session.store.as_ref();
    if !store.thread_exists(id)? {
        return Err(format!(
            "error: no such thread: {} (expected {})",
            id,
            store.describe_thread(id)
        ));
    }
    let info = store.thread_info(id)?;
    let mission_id = thread_mission(&info);
    let task_id = checked_task(&store.missions_load()?, id, mission_id.as_deref(), task_id)?;
    Ok((info, task_id))
}

const BRANCH_PREFIX: &str = "refs/heads/";
const ORIGIN_HEADS_REFSPEC: &str = "+refs/heads/*:refs/remotes/origin/*";
const ORIGIN_REMOTE_PREFIX: &str = "refs/remotes/origin/";
const ORIGIN_REMOTE_HEAD: &str = "refs/remotes/origin/HEAD";

fn current_branch_ref(git: &Git) -> Result<String, String> {
    let (_, output) = git.output(["symbolic-ref", "--quiet", "HEAD"])?;
    match output.status.code() {
        Some(0) => {
            let text = String::from_utf8_lossy(&output.stdout);
            let name = text.trim_end_matches(['\n', '\r']);
            if is_branch_ref(name) {
                Ok(name.to_string())
            } else {
                Err(format!(
                    "error: HEAD points at '{}', which is not a branch.\n       \
                     Check out a branch before pausing.",
                    name
                ))
            }
        }
        Some(1) => Err(
            "error: HEAD is detached, so the branch the work is on cannot be recorded.\n       \
             Check out a branch, and push it to origin, before pausing."
                .to_string(),
        ),
        _ => Err(crate::ledger::git::failure_message(
            &[
                "symbolic-ref".to_string(),
                "--quiet".to_string(),
                "HEAD".to_string(),
            ],
            &output,
        )),
    }
}

/// True when HEAD is detached: it names a commit and no branch.
fn is_detached(git: &Git) -> bool {
    git.output(["symbolic-ref", "--quiet", "HEAD"])
        .is_ok_and(|(_, output)| output.status.code() == Some(1))
}

/// The branch a pause records for a detached HEAD (a harness worktree starts detached): the
/// origin branch that holds the commit, origin's default branch first, as `refs/heads/<name>`.
/// `None` when no origin branch holds it, after one fetch.
fn detached_ref(git: &Git, commit: &str) -> Option<String> {
    let holders = |git: &Git| -> Vec<String> {
        git.run([
            "for-each-ref",
            "--contains",
            commit,
            "--format=%(refname)",
            ORIGIN_REMOTE_PREFIX,
        ])
        .map(|listing| origin_branches(&listing))
        .unwrap_or_default()
    };
    let mut found = holders(git);
    if found.is_empty() {
        let _ = git.run([
            "fetch",
            "--quiet",
            "--prune",
            "origin",
            ORIGIN_HEADS_REFSPEC,
        ]);
        found = holders(git);
    }
    let default = git
        .run_line(["symbolic-ref", "--quiet", ORIGIN_REMOTE_HEAD])
        .ok();
    let pick = default
        .filter(|d| found.contains(d))
        .or_else(|| found.first().cloned())?;
    pick.strip_prefix(ORIGIN_REMOTE_PREFIX)
        .map(|name| format!("{}{}", BRANCH_PREFIX, name))
}

fn ensure_on_an_origin_branch(git: &Git, code_ref: &str, commit: &str) -> Result<(), String> {
    // The remote-tracking branches the last fetch left say whether a branch on origin
    // holds the commit. Only when they do not is origin asked.
    if on_an_origin_branch(git, commit)? {
        return Ok(());
    }
    git.run([
        "fetch",
        "--quiet",
        "--prune",
        "origin",
        ORIGIN_HEADS_REFSPEC,
    ])?;
    if !on_an_origin_branch(git, commit)? {
        return Err(format!(
            "error: HEAD ({}) is not reachable from any branch on origin.\n       \
             A different actor resuming this thread elsewhere would not be able\n       \
             to see this commit. Push {} before pausing.",
            commit,
            code_ref.strip_prefix(BRANCH_PREFIX).unwrap_or(code_ref)
        ));
    }
    Ok(())
}

/// True when a remote-tracking branch of origin holds `commit`, as the clone last fetched.
fn on_an_origin_branch(git: &Git, commit: &str) -> Result<bool, String> {
    let listing = git.run([
        "for-each-ref",
        "--contains",
        commit,
        "--format=%(refname)",
        ORIGIN_REMOTE_PREFIX,
    ])?;
    Ok(!origin_branches(&listing).is_empty())
}

fn origin_branches(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter(|line| line.starts_with(ORIGIN_REMOTE_PREFIX) && *line != ORIGIN_REMOTE_HEAD)
        .map(str::to_string)
        .collect()
}

fn is_branch_ref(target: &str) -> bool {
    target
        .strip_prefix(BRANCH_PREFIX)
        .is_some_and(|name| !name.is_empty() && !name.contains(char::is_whitespace))
}

/// The time a resume registers its binding at: now, or the second after the latest pause or
/// the latest binding of another instance, when that time is not before now. Times have a
/// resolution of one second, so a resume in the second of a pause would otherwise leave the
/// thread paused, and a take-over in the second of the holder's binding would tie with it.
pub fn binding_time(
    now: u64,
    latest_pause: Option<&str>,
    latest_other_binding: Option<&str>,
) -> String {
    let after = |time: Option<&str>| time.and_then(parse_utc).map(|t| t + 1).unwrap_or(0);
    format_utc(
        now.max(after(latest_pause))
            .max(after(latest_other_binding)),
    )
}

/// The error for a resume of a thread that another session holds.
pub fn held_error(id: &str, holder: &Holder) -> String {
    format!(
        "error: thread {} is held by {} since {}. A thread has one open session at a time (ADR 0032). If that session no longer runs, take the thread over: tsk thread resume {} --take-over",
        id,
        holder.label(),
        holder.since,
        id
    )
}

/// The holder of a thread when it is another harness instance than this session or worktree.
/// `None` when the thread is paused, when this instance holds it, or when the ledger cannot
/// say.
pub fn other_holder(session: &Session, id: &str) -> Option<Holder> {
    let store = session.store.as_ref();
    let entries = entries_of(store, id).ok()?;
    let writers = Writers::load(store).ok()?;
    let bindings = store.bindings_list().ok()?;
    let this_instance = session.actor_key().ok()??.0;
    match thread_state_of(id, &entries, &bindings, &writers) {
        ThreadState::Held(holder) if holder.instance != this_instance => Some(holder),
        _ => None,
    }
}

/// What a session that is bound to a thread, which another session holds, is told: not to
/// work the thread, and the choices.
pub fn held_note(id: &str, holder: &Holder) -> String {
    format!(
        "Thread {} is held by {} since {}, so this session does not hold it. Do not work the thread. Tell the person. If that session no longer runs, and the person confirms, run: tsk thread resume {} --take-over. Otherwise work with no thread (tsk thread skip) or start another thread.",
        id,
        holder.label(),
        holder.since,
        id
    )
}

/// Bind this session or worktree to a thread and read its latest entry. A thread that
/// another harness instance holds is refused, unless `take_over` is set. Every resume
/// writes the binding again, so its `registered_at` is later than the latest pause and the
/// thread is held by this instance.
pub fn resume(session: &Session, id: &str, take_over: bool) -> Result<ResumeOutcome, String> {
    check_thread_id(id)?;
    refresh_for_binding(session)?;
    let store = session.store.as_ref();
    if !store.thread_exists(id)? {
        return Err(format!("error: no such thread: {}", id));
    }
    let entries = entries_of(store, id)?;
    let writers = Writers::load(store)?;
    let warning = writers.takeover_warning(id, &entries, &session.written_by()?);

    if !take_over {
        let this_instance = session.actor_key()?.map(|(instance, _)| instance);
        let state = thread_state_of(id, &entries, &store.bindings_list()?, &writers);
        if let ThreadState::Held(holder) = state {
            if Some(&holder.instance) != this_instance.as_ref() {
                return Err(held_error(id, &holder));
            }
        }
    }

    // The binding is a local write. It goes out with the next push (ADR 0019).
    let this_instance = session.actor_key()?.map(|(instance, _)| instance);
    let latest_other = store
        .bindings_list()?
        .into_iter()
        .filter(|row| row.thread_id == id && Some(&row.instance) != this_instance.as_ref())
        .map(|row| row.registered_at)
        .filter(|time| parse_utc(time).is_some())
        .max_by_key(|time| parse_utc(time));
    let at = binding_time(
        now_secs(),
        latest_pause(&entries).as_deref(),
        latest_other.as_deref(),
    );
    bind_current_at(session, id, at)?;
    eprintln!("{}", NOT_PUBLISHED_NOTE);
    clear_skip(session)?;

    let missions = store.missions_load().unwrap_or_default();
    let facts = match store.thread_info(id) {
        Ok(info) => facts_of(&info, entries.last().map(|s| &s.entry), &missions),
        Err(_) => Facts {
            mission: None,
            task: None,
        },
    };
    let actor = crate::actor::current(session)
        .ok()
        .flatten()
        .map(|actor| actor.name);
    let mission_id = facts.mission.as_ref().map(|(id, _)| id.clone());
    let session_title = session
        .cloud_session
        .as_deref()
        .zip(session_title(actor.as_deref(), mission_id.as_deref()))
        .map(|(cloud, title)| (cloud_session_id(cloud), title));
    let version_note = entries
        .last()
        .and_then(|stored| super::entry::version_note(&stored.entry, env!("CARGO_PKG_VERSION")));
    Ok(ResumeOutcome {
        session_title,
        version_note,
        mission: facts.mission,
        task: facts.task,
        thread_id: id.to_string(),
        latest: entries
            .last()
            .map(|stored| stored.raw.clone())
            .unwrap_or_else(|| "{}".to_string()),
        warning,
    })
}

/// The resume JSON: `thread_id`, `latest` (as stored), `warning`, then `mission`
/// (`{"id","name"}` or null) and `task` (`{"id","description"}` or null).
pub fn render_resume(outcome: &ResumeOutcome) -> String {
    #[derive(Serialize)]
    struct Mission<'a> {
        id: &'a str,
        name: &'a str,
    }
    #[derive(Serialize)]
    struct Task<'a> {
        id: &'a str,
        description: &'a str,
    }
    let mission = serde_json::to_string(
        &outcome
            .mission
            .as_ref()
            .map(|(id, name)| Mission { id, name }),
    )
    .unwrap_or_default();
    let task = serde_json::to_string(
        &outcome
            .task
            .as_ref()
            .map(|(id, description)| Task { id, description }),
    )
    .unwrap_or_default();
    #[derive(Serialize)]
    struct Title<'a> {
        session_id: &'a str,
        title: &'a str,
    }
    let title = serde_json::to_string(
        &outcome
            .session_title
            .as_ref()
            .map(|(session_id, title)| Title { session_id, title }),
    )
    .unwrap_or_default();
    let note = serde_json::to_string(&outcome.version_note).unwrap_or_default();
    format!(
        "{{\"thread_id\":{},\"latest\":{},\"warning\":{},\"mission\":{},\"task\":{},\"session_title\":{},\"version_note\":{}}}",
        serde_json::Value::String(outcome.thread_id.clone()),
        outcome.latest,
        serde_json::Value::String(outcome.warning.clone()),
        mission,
        task,
        title,
        note
    )
}

/// The lines a pause prints after `paused:<thread-id>`, which stand in for the checks a
/// person would run by hand: the code worktree, the code commit on origin, and the ledger.
/// No line says it is safe when the check failed or could not run.
pub fn pause_report(session: &Session) -> Vec<String> {
    let code_git = &session.repo.git;
    let dirty = code_git
        .run(["status", "--porcelain"])
        .map(|out| out.lines().filter(|l| !l.trim().is_empty()).count());
    let code = match dirty {
        Ok(0) => "code: clean".to_string(),
        Ok(n) => format!(
            "code: dirty ({} {})",
            n,
            if n == 1 { "file" } else { "files" }
        ),
        Err(e) => format!("code: unknown ({})", e.replace('\n', " ")),
    };
    let pushed = code_git
        .run_line(["rev-parse", "--verify", "HEAD^{commit}"])
        .and_then(|commit| on_an_origin_branch(code_git, &commit));
    let code_pushed = match pushed {
        Ok(true) => "code pushed: yes".to_string(),
        Ok(false) => "code pushed: no".to_string(),
        Err(e) => format!("code pushed: unknown ({})", e.replace('\n', " ")),
    };
    let ledger = match session.store.status_local() {
        Ok(status) if !status.has_unpushed() => "ledger: pushed".to_string(),
        Ok(status) => {
            let n = status.unpushed_count();
            format!(
                "ledger: {} {} not pushed",
                n,
                if n == 1 { "change" } else { "changes" }
            )
        }
        Err(e) => format!("ledger: unknown ({})", e.replace('\n', " ")),
    };
    vec![code, code_pushed, ledger]
}

pub fn detach(session: &Session) -> Result<String, String> {
    refresh_for_binding(session)?;
    let binding = resolve(session)?.ok_or_else(|| {
        "error: no thread binding found for this session or worktree; nothing to detach".to_string()
    })?;
    // A local write. It goes out with the next push (ADR 0019).
    unbind_current(session)?;
    eprintln!("{}", NOT_PUBLISHED_NOTE);
    Ok(binding.thread_id().to_string())
}

/// The note a thread command prints on stderr after a local write that does not push:
/// resume, detach and set mission. The publish rule is ADR 0019.
pub const NOT_PUBLISHED_NOTE: &str =
    "note: saved in the local ledger store; it goes out with the next push (tsk thread start, pause or stop, or tsk ledger push)";

/// Refresh before a bind or a detach. A worktree command with a local ledger store goes on
/// when the refresh fails (origin away): its binding is valid locally.
fn refresh_for_binding(session: &Session) -> Result<(), String> {
    match session.refresh() {
        Ok(_) => Ok(()),
        Err(_) if session.cloud_session.is_none() && session.store.exists_locally()? => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn stop(session: &Session, target: Option<&str>) -> Result<String, String> {
    session.refresh()?;
    let store = session.store.as_ref();
    let current = resolve(session)?;
    let id = match target {
        Some(id) => id.to_string(),
        None => current
            .as_ref()
            .map(|b| b.thread_id().to_string())
            .ok_or_else(|| {
                "error: no thread binding found for this session or worktree, and no thread id given; nothing to stop"
                    .to_string()
            })?,
    };
    check_thread_id(&id)?;
    if !store.thread_exists(&id)? {
        return Err(format!("error: no such thread: {}", id));
    }
    if current == Some(Binding::Worktree(id.clone())) {
        remove_marker(session)?;
    }
    store.binding_remove_thread(&id)?;
    store.thread_delete(&id)?;
    session.push(&format!("Stop thread {}", id))?;
    Ok(id)
}

pub fn list_rows(store: &dyn LedgerStore) -> Result<Vec<ListRow>, String> {
    let bindings = store.bindings_list()?;
    // The actor of each bound harness instance: the primary actor of its latest session,
    // else the actor it registered as (ADR 0030).
    let writers = Writers::load(store)?;
    let ledger_id = store.ledger_id()?;
    let mut rows = Vec::new();
    let names = store.thread_ids()?;
    // Every thread's lines are read together, not one process per thread.
    let lines = store.continuation_lists(&names)?;
    for (name, lines) in names.into_iter().zip(lines) {
        let info = store.thread_info(&name)?;
        let entries = parse_store(&lines.join("\n"), &store.describe_thread(&name))?;
        let latest = entries.last().map(|stored| &stored.entry);
        let state = thread_state_of(&name, &entries, &bindings, &writers);
        rows.push(ListRow {
            state: state.name(),
            holder: state.holder().cloned(),
            urn: ledger_id
                .as_deref()
                .map(|ledger| crate::urn::thread_urn(ledger, &name)),
            mission_id: thread_mission(&info),
            mission_link: info.briefing_path.unwrap_or_default(),
            latest_whats_next: latest.and_then(|e| e.whats_next.clone()),
            latest_timestamp: latest.and_then(|e| e.timestamp.clone()),
            actors: bindings
                .iter()
                .filter(|binding| binding.thread_id == name)
                .map(|binding| {
                    let writer = writers.resolve(&binding.instance);
                    ListActor {
                        actor: binding.instance.clone(),
                        kind: binding.kind.as_str(),
                        name: writer.actor_uid.map(|_| writer.label),
                    }
                })
                .collect(),
            id: name,
        });
    }
    Ok(sort_rows(rows))
}

pub fn sort_rows(mut rows: Vec<ListRow>) -> Vec<ListRow> {
    rows.sort_by(|a, b| {
        a.latest_timestamp
            .as_deref()
            .unwrap_or("")
            .cmp(b.latest_timestamp.as_deref().unwrap_or(""))
    });
    rows.reverse();
    rows
}

/// The age, in days, past which `tsk thread prune-bindings` removes a cloud binding.
pub const DEFAULT_PRUNE_DAYS: u64 = 30;

/// The bindings `tsk thread prune-bindings` removes, in the order given: cloud rows
/// registered more than `days` days before `now` (seconds since the epoch), except the
/// row of `current`. A worktree row is never chosen, and neither is a row whose
/// `registered_at` is not a timestamp: its age is unknown.
pub fn prunable_bindings(
    rows: &[BindingRow],
    now: u64,
    days: u64,
    current: Option<&str>,
) -> Vec<BindingRow> {
    let limit = days.saturating_mul(86_400);
    rows.iter()
        .filter(|row| row.kind == BindingKind::Cloud)
        .filter(|row| Some(row.instance.as_str()) != current)
        .filter(|row| {
            parse_utc(&row.registered_at).is_some_and(|at| now.saturating_sub(at) > limit)
        })
        .cloned()
        .collect()
}

/// Remove the cloud bindings of sessions that ended: see [`prunable_bindings`]. With
/// `dry_run` nothing is changed. The removal is a local write, which the next
/// `tsk ledger push` publishes. Returns the rows removed, or that would be.
pub fn prune_bindings(
    session: &Session,
    days: u64,
    dry_run: bool,
) -> Result<Vec<BindingRow>, String> {
    let store = session.store.as_ref();
    let current = session.actor_key()?.map(|(actor, _)| actor);
    let rows = prunable_bindings(
        &store.bindings_list()?,
        now_secs(),
        days,
        current.as_deref(),
    );
    if !dry_run {
        let actors: Vec<String> = rows.iter().map(|row| row.instance.clone()).collect();
        store.bindings_remove(&actors)?;
    }
    Ok(rows)
}

/// The lines `tsk thread prune-bindings` prints: one per row, then the count.
pub fn render_prune(rows: &[BindingRow], dry_run: bool) -> Vec<String> {
    let verb = if dry_run { "would remove" } else { "removed" };
    let mut lines: Vec<String> = rows
        .iter()
        .map(|row| format!("{} {} {}", verb, row.instance, row.thread_id))
        .collect();
    let noun = if rows.len() == 1 {
        "binding"
    } else {
        "bindings"
    };
    lines.push(if dry_run {
        format!("{} {} would be removed", rows.len(), noun)
    } else {
        format!("{} {} removed", rows.len(), noun)
    });
    lines
}

pub fn list(session: &Session) -> Result<Vec<ListRow>, String> {
    session.refresh()?;
    list_rows(session.store.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_title_is_the_actor_and_the_mission_or_whichever_there_is() {
        assert_eq!(
            session_title(Some("Plain Statement Of Fact"), Some("M-ADMIN-02")).as_deref(),
            Some("M-ADMIN-02: Plain Statement Of Fact")
        );
        assert_eq!(
            session_title(Some("alice"), None).as_deref(),
            Some("NO-MISSION: alice")
        );
        assert_eq!(session_title(None, Some("M-X")).as_deref(), Some("M-X"));
        assert_eq!(session_title(None, None), None);
    }

    #[test]
    fn thread_id_must_be_eight_lowercase_base36_characters() {
        assert!(check_thread_id("1234abcd").is_ok());
        assert!(check_thread_id("4onylfsg").is_ok());
        assert!(check_thread_id("1234abc").is_err());
        assert!(check_thread_id("1234ABCD").is_err());
        assert!(check_thread_id("../../xy").is_err());
        assert!(check_thread_id("").is_err());
    }

    #[test]
    fn is_branch_ref_accepts_only_named_branch_refs() {
        assert!(is_branch_ref("refs/heads/main"));
        assert!(is_branch_ref("refs/heads/feature/x"));
        assert!(!is_branch_ref("refs/heads/"));
        assert!(!is_branch_ref("refs/tags/v1"));
        assert!(!is_branch_ref("refs/heads/a b"));
        assert!(!is_branch_ref(""));
    }

    #[test]
    fn origin_branches_skips_the_symbolic_head_and_other_refs() {
        let listing = "refs/remotes/origin/HEAD\nrefs/remotes/origin/main\nrefs/remotes/origin/feature/x\nrefs/remotes/upstream/main\n";
        assert_eq!(
            origin_branches(listing),
            vec![
                "refs/remotes/origin/main".to_string(),
                "refs/remotes/origin/feature/x".to_string()
            ]
        );
        assert!(origin_branches("refs/remotes/origin/HEAD\n").is_empty());
        assert!(origin_branches("").is_empty());
    }

    #[test]
    fn mission_id_comes_from_the_file_name_before_the_slug() {
        let id = |p: &str| mission_id_of_file(p);
        assert_eq!(
            id("missions/operational/M-BOOT-04-official-ledger.md").as_deref(),
            Some("M-BOOT-04")
        );
        assert_eq!(
            id("missions/operational/M-CONF-01.md").as_deref(),
            Some("M-CONF-01")
        );
        assert_eq!(
            id("missions/operational/M-BOOT-02-01-ledger-store.md").as_deref(),
            Some("M-BOOT-02-01")
        );
        assert_eq!(
            id("M-TSK-SCHEMA-tsk-schema.md").as_deref(),
            Some("M-TSK-SCHEMA")
        );
        assert_eq!(id("missions/operational/notes.md"), None);
        assert_eq!(id("missions/operational/M-lower.md"), None);
        assert_eq!(id("base.md"), None);
        assert_eq!(id(""), None);
    }

    #[test]
    fn a_thread_mission_is_stored_else_derived_from_the_briefing_path() {
        let info = |mission: Option<&str>, briefing: Option<&str>| ThreadInfo {
            mission_id: mission.map(str::to_string),
            briefing_path: briefing.map(str::to_string),
        };
        assert_eq!(
            thread_mission(&info(Some("M-A"), Some("missions/operational/M-B-x.md"))).as_deref(),
            Some("M-A")
        );
        assert_eq!(
            thread_mission(&info(None, Some("missions/operational/M-B-x.md"))).as_deref(),
            Some("M-B")
        );
        assert_eq!(thread_mission(&info(None, Some("base.md"))), None);
        assert_eq!(thread_mission(&info(None, None)), None);
    }

    fn mission_with_task() -> Mission {
        let missions = crate::mission::model::create(
            &[],
            "M-X",
            "Mission X",
            crate::mission::model::Category::Operational,
            None,
        )
        .unwrap();
        crate::mission::records::task_add(&[missions], "M-X", "First").unwrap()
    }

    #[test]
    fn a_task_is_checked_against_a_mission_with_a_record_only() {
        let missions = vec![mission_with_task()];
        assert_eq!(
            checked_task(&missions, "1234abcd", Some("M-X"), "T-1").unwrap(),
            "T-01"
        );
        assert_eq!(
            checked_task(&missions, "1234abcd", Some("M-X"), "T-01").unwrap(),
            "T-01"
        );
        assert_eq!(
            checked_task(&missions, "1234abcd", Some("M-X"), "").unwrap(),
            ""
        );
        let err = checked_task(&missions, "1234abcd", Some("M-X"), "T-09").unwrap_err();
        assert!(err.contains("T-09"), "{}", err);
        assert!(checked_task(&missions, "1234abcd", Some("M-X"), "later").is_err());
        assert_eq!(
            checked_task(&missions, "1234abcd", Some("M-Y"), "T-09").unwrap(),
            "T-09"
        );
        // A thread with no mission has no tasks.
        assert_eq!(checked_task(&missions, "1234abcd", None, "").unwrap(), "");
        assert_eq!(
            checked_task(&missions, "1234abcd", None, "anything").unwrap_err(),
            "error: thread 1234abcd has no mission, so it has no tasks. Attach a mission with tsk thread set mission <mission-id>, or pass an empty task ID"
        );
    }

    #[test]
    fn mint_id_is_eight_hex_characters_not_already_taken() {
        let taken = std::cell::RefCell::new(Vec::new());
        let id = mint_id("seed", "threads", &|id| {
            taken.borrow_mut().push(id.to_string());
            Ok(taken.borrow().len() < 3)
        })
        .unwrap();
        assert!(check_thread_id(&id).is_ok());
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(taken.borrow().len(), 3);
        assert_eq!(taken.borrow().last(), Some(&id));
    }

    #[test]
    fn mint_id_gives_up_when_every_id_is_taken() {
        let err = mint_id("seed", "/s/threads", &|_| Ok(true)).unwrap_err();
        assert_eq!(
            err,
            "error: no free thread id after 100 attempts; is /s/threads intact?"
        );
    }

    fn bound(actor: &str, kind: BindingKind, at: &str) -> BindingRow {
        BindingRow {
            instance: actor.to_string(),
            kind,
            thread_id: "1234abcd".to_string(),
            registered_at: at.to_string(),
        }
    }

    // 2026-10-19T00:00:00Z
    const NOW: u64 = 1_792_368_000;

    #[test]
    fn prune_selects_old_cloud_rows_only() {
        let rows = vec![
            bound("old", BindingKind::Cloud, "2026-09-01T00:00:00Z"),
            bound("new", BindingKind::Cloud, "2026-10-10T00:00:00Z"),
            bound(
                "old-worktree",
                BindingKind::Worktree,
                "2026-01-01T00:00:00Z",
            ),
            bound("odd", BindingKind::Cloud, "not a time"),
        ];
        let chosen = prunable_bindings(&rows, NOW, 30, None);
        assert_eq!(
            chosen
                .iter()
                .map(|r| r.instance.as_str())
                .collect::<Vec<_>>(),
            vec!["old"]
        );
    }

    #[test]
    fn prune_keeps_the_current_session_and_honours_the_age_limit() {
        let rows = vec![
            bound("mine", BindingKind::Cloud, "2026-01-01T00:00:00Z"),
            bound("exactly", BindingKind::Cloud, "2026-09-19T00:00:00Z"),
            bound("just over", BindingKind::Cloud, "2026-09-18T23:59:59Z"),
        ];
        let chosen = prunable_bindings(&rows, NOW, 30, Some("mine"));
        assert_eq!(
            chosen
                .iter()
                .map(|r| r.instance.as_str())
                .collect::<Vec<_>>(),
            vec!["just over"]
        );
        assert_eq!(prunable_bindings(&rows, NOW, 0, Some("mine")).len(), 2);
    }

    #[test]
    fn prune_output_has_a_line_per_row_and_a_count() {
        let rows = vec![
            bound("a", BindingKind::Cloud, "2026-01-01T00:00:00Z"),
            bound("b", BindingKind::Cloud, "2026-01-02T00:00:00Z"),
        ];
        assert_eq!(
            render_prune(&rows, false),
            vec![
                "removed a 1234abcd",
                "removed b 1234abcd",
                "2 bindings removed"
            ]
        );
        assert_eq!(
            render_prune(&rows[..1], true),
            vec!["would remove a 1234abcd", "1 binding would be removed"]
        );
        assert_eq!(render_prune(&[], false), vec!["0 bindings removed"]);
    }

    #[test]
    fn render_resume_passes_the_latest_entry_through_as_stored() {
        let outcome = ResumeOutcome {
            thread_id: "1234abcd".to_string(),
            latest: "{\"whats_next\":\"x\",\"git\":{\"ledger\":{\"commit\":\"b\"}}}".to_string(),
            warning: String::new(),
            mission: Some(("M-X".to_string(), "Example".to_string())),
            task: None,
            session_title: Some(("session_01AB".to_string(), "M-X: Robot Bee".to_string())),
            version_note: None,
        };
        assert_eq!(
            render_resume(&outcome),
            "{\"thread_id\":\"1234abcd\",\"latest\":{\"whats_next\":\"x\",\"git\":{\"ledger\":{\"commit\":\"b\"}}},\"warning\":\"\",\"mission\":{\"id\":\"M-X\",\"name\":\"Example\"},\"task\":null,\"session_title\":{\"session_id\":\"session_01AB\",\"title\":\"M-X: Robot Bee\"},\"version_note\":null}"
        );
        assert_eq!(cloud_session_id("cse_01AB"), "session_01AB");
        assert_eq!(cloud_session_id("session_01AB"), "session_01AB");
    }

    fn row(id: &str, ts: Option<&str>) -> ListRow {
        ListRow {
            id: id.to_string(),
            urn: None,
            mission_id: None,
            mission_link: String::new(),
            latest_whats_next: None,
            latest_timestamp: ts.map(str::to_string),
            actors: Vec::new(),
            state: "paused",
            holder: None,
        }
    }

    #[test]
    fn sort_rows_puts_newest_first_and_threads_without_entries_last() {
        let sorted = sort_rows(vec![
            row("aaaaaaaa", None),
            row("bbbbbbbb", Some("2026-09-01T00:00:00Z")),
            row("cccccccc", None),
            row("dddddddd", Some("2026-10-01T00:00:00Z")),
        ]);
        let ids: Vec<_> = sorted.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["dddddddd", "bbbbbbbb", "cccccccc", "aaaaaaaa"]);
    }

    #[test]
    fn list_row_serialises_in_the_script_field_order() {
        let r = ListRow {
            id: "1234abcd".to_string(),
            urn: None,
            mission_id: Some("M-X".to_string()),
            mission_link: "m.md".to_string(),
            latest_whats_next: None,
            latest_timestamp: None,
            actors: vec![
                ListActor {
                    actor: "cse_1".to_string(),
                    kind: "cloud",
                    name: Some("alice".to_string()),
                },
                ListActor {
                    actor: "tsk-1a2b/main".to_string(),
                    kind: "worktree",
                    name: None,
                },
            ],
            state: "held",
            holder: Some(Holder {
                instance: "cse_1".to_string(),
                robot: Some("alice".to_string()),
                since: "2026-10-01T00:00:00Z".to_string(),
            }),
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            "{\"id\":\"1234abcd\",\"mission_id\":\"M-X\",\"mission_link\":\"m.md\",\"latest_whats_next\":null,\"latest_timestamp\":null,\"actors\":[{\"actor\":\"cse_1\",\"kind\":\"cloud\",\"name\":\"alice\"},{\"actor\":\"tsk-1a2b/main\",\"kind\":\"worktree\",\"name\":null}],\"state\":\"held\",\"holder\":{\"instance\":\"cse_1\",\"robot\":\"alice\",\"since\":\"2026-10-01T00:00:00Z\"}}"
        );
        assert!(serde_json::to_string(&row("1234abcd", None))
            .unwrap()
            .ends_with("\"actors\":[],\"state\":\"paused\",\"holder\":null}"));
    }

    fn binding_at(instance: &str, thread: &str, at: &str) -> BindingRow {
        BindingRow {
            instance: instance.to_string(),
            kind: BindingKind::Cloud,
            thread_id: thread.to_string(),
            registered_at: at.to_string(),
        }
    }

    fn robot_of(instance: &str) -> Option<String> {
        (instance == "cse_01ROBOTAAAAAA").then(|| "Robot A".to_string())
    }

    #[test]
    fn a_thread_with_no_binding_is_paused() {
        let rows = vec![binding_at("cse_1", "other000", "2026-11-01T00:00:00Z")];
        assert_eq!(
            thread_state("1234abcd", None, &rows, &robot_of),
            ThreadState::Paused
        );
        assert_eq!(
            thread_state("1234abcd", Some("2026-10-01T00:00:00Z"), &[], &robot_of),
            ThreadState::Paused
        );
    }

    #[test]
    fn a_binding_with_no_pause_holds_the_thread() {
        let rows = vec![binding_at(
            "cse_01ROBOTAAAAAA",
            "1234abcd",
            "2026-11-01T00:00:00Z",
        )];
        let state = thread_state("1234abcd", None, &rows, &robot_of);
        assert_eq!(state.name(), "held");
        let holder = state.holder().unwrap();
        assert_eq!(holder.instance, "cse_01ROBOTAAAAAA");
        assert_eq!(holder.robot.as_deref(), Some("Robot A"));
        assert_eq!(holder.since, "2026-11-01T00:00:00Z");
        assert_eq!(holder.label(), "Robot A (01ROBOTA)");
    }

    #[test]
    fn a_binding_later_than_the_latest_pause_holds_and_an_earlier_or_equal_one_does_not() {
        let rows = vec![binding_at("cse_2", "1234abcd", "2026-11-01T10:00:00Z")];
        let later = thread_state("1234abcd", Some("2026-11-01T09:59:59Z"), &rows, &robot_of);
        assert_eq!(later.holder().map(|h| h.instance.as_str()), Some("cse_2"));
        assert_eq!(later.holder().unwrap().robot, None);
        for pause in ["2026-11-01T10:00:00Z", "2026-11-01T10:00:01Z"] {
            assert_eq!(
                thread_state("1234abcd", Some(pause), &rows, &robot_of),
                ThreadState::Paused,
                "{}",
                pause
            );
        }
    }

    #[test]
    fn the_holder_is_the_instance_with_the_latest_binding() {
        let rows = vec![
            binding_at("cse_old", "1234abcd", "2026-11-01T10:00:00Z"),
            binding_at("cse_new", "1234abcd", "2026-11-02T10:00:00Z"),
            binding_at("cse_mid", "1234abcd", "2026-11-01T20:00:00Z"),
        ];
        let state = thread_state("1234abcd", Some("2026-11-01T12:00:00Z"), &rows, &robot_of);
        assert_eq!(state.holder().unwrap().instance, "cse_new");
        // A pause after every binding frees the thread, whoever bound it.
        assert_eq!(
            thread_state("1234abcd", Some("2026-11-03T00:00:00Z"), &rows, &robot_of),
            ThreadState::Paused
        );
    }

    #[test]
    fn the_latest_pause_is_the_latest_timestamp_of_the_entries() {
        let entries = parse_store(
            "{\"whats_next\":\"a\",\"timestamp\":\"2026-10-02T00:00:00Z\"}\n{\"whats_next\":\"c\",\"timestamp\":\"2026-10-01T00:00:00Z\"}\n",
            "test",
        )
        .unwrap();
        assert_eq!(
            latest_pause(&entries).as_deref(),
            Some("2026-10-02T00:00:00Z")
        );
        assert_eq!(latest_pause(&[]), None);
    }

    #[test]
    fn an_entry_with_no_timestamp_is_a_pause_at_the_legacy_time() {
        let legacy = parse_store(
            "{\"whats_next\":\"a\"}\n{\"whats_next\":\"b\",\"timestamp\":\"\"}\n",
            "test",
        )
        .unwrap();
        assert_eq!(latest_pause(&legacy).as_deref(), Some(LEGACY_PAUSE_TIME));
        // A binding copied by an old migration is older than the legacy time: not a holder.
        let old = vec![binding_at("cse_1", "1234abcd", "2026-09-01T00:00:00Z")];
        assert_eq!(
            thread_state(
                "1234abcd",
                latest_pause(&legacy).as_deref(),
                &old,
                &robot_of
            ),
            ThreadState::Paused
        );
        // A resume after the ADR took effect holds the thread, though no entry has a time.
        let new = vec![binding_at("cse_1", "1234abcd", "2026-11-14T09:00:00Z")];
        let state = thread_state(
            "1234abcd",
            latest_pause(&legacy).as_deref(),
            &new,
            &robot_of,
        );
        assert_eq!(state.holder().map(|h| h.instance.as_str()), Some("cse_1"));
        // A timestamp later than the legacy time wins over an entry with none.
        let mixed = parse_store(
            "{\"whats_next\":\"a\"}\n{\"whats_next\":\"b\",\"timestamp\":\"2026-11-20T00:00:00Z\"}\n",
            "test",
        )
        .unwrap();
        assert_eq!(
            latest_pause(&mixed).as_deref(),
            Some("2026-11-20T00:00:00Z")
        );
    }

    #[test]
    fn a_pause_is_never_earlier_than_a_binding_of_its_thread() {
        // 2026-11-14T10:00:00Z
        let now = 1_794_650_400;
        let rows = vec![
            binding_at("cse_1", "1234abcd", "2026-11-14T10:00:01Z"),
            binding_at("cse_2", "other000", "2026-11-14T11:00:00Z"),
        ];
        // A resume stamped its binding a second ahead of the clock: the pause matches it.
        let pause = pause_time(now, "1234abcd", &rows);
        assert_eq!(pause, "2026-11-14T10:00:01Z");
        assert_eq!(
            thread_state("1234abcd", Some(&pause), &rows, &robot_of),
            ThreadState::Paused
        );
        // A binding of another thread, or an older one, leaves the clock alone.
        assert_eq!(pause_time(now, "third000", &rows), "2026-11-14T10:00:00Z");
        assert_eq!(pause_time(now, "1234abcd", &[]), "2026-11-14T10:00:00Z");
        let older = vec![binding_at("cse_1", "1234abcd", "2026-11-14T09:00:00Z")];
        assert_eq!(pause_time(now, "1234abcd", &older), "2026-11-14T10:00:00Z");
    }

    #[test]
    fn a_resume_registers_its_binding_after_the_latest_pause_and_other_bindings() {
        // 2026-11-14T10:00:00Z
        let now = 1_794_650_400;
        assert_eq!(binding_time(now, None, None), "2026-11-14T10:00:00Z");
        assert_eq!(
            binding_time(now, Some("2026-11-14T09:00:00Z"), None),
            "2026-11-14T10:00:00Z"
        );
        assert_eq!(
            binding_time(now, Some("2026-11-14T10:00:00Z"), None),
            "2026-11-14T10:00:01Z"
        );
        assert_eq!(
            binding_time(now, Some("2026-11-14T10:05:00Z"), None),
            "2026-11-14T10:05:01Z"
        );
        assert_eq!(
            binding_time(now, Some("not a time"), None),
            "2026-11-14T10:00:00Z"
        );
        // A take-over in the second of the holder's binding is strictly later than it.
        assert_eq!(
            binding_time(now, None, Some("2026-11-14T10:00:00Z")),
            "2026-11-14T10:00:01Z"
        );
        assert_eq!(
            binding_time(
                now,
                Some("2026-11-14T10:00:00Z"),
                Some("2026-11-14T10:00:03Z")
            ),
            "2026-11-14T10:00:04Z"
        );
    }

    #[test]
    fn an_instance_is_cut_short_for_a_person() {
        assert_eq!(short_instance("cse_01Sw1xdtf5tWD95jCuHqkK75"), "01Sw1xdt");
        assert_eq!(short_instance("tsk-180ae86e/main"), "tsk-180ae86e/main");
        assert_eq!(short_instance("abc"), "abc");
    }

    #[test]
    fn the_refusal_names_the_holder_and_the_take_over_flag() {
        let holder = Holder {
            instance: "cse_01ROBOTAAAAAA".to_string(),
            robot: Some("Robot A".to_string()),
            since: "2026-11-01T00:00:00Z".to_string(),
        };
        let text = held_error("1234abcd", &holder);
        assert!(text.starts_with(
            "error: thread 1234abcd is held by Robot A (01ROBOTA) since 2026-11-01T00:00:00Z."
        ));
        assert!(text.ends_with("tsk thread resume 1234abcd --take-over"));
    }
}
