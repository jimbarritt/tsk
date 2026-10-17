use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::ledger::git::Git;
use crate::ledger::location::mint_token;
use crate::ledger::store::{BindingKind, LedgerStore, ThreadInfo};
use crate::mission::model::Mission;
use crate::mission::records::resolve_task;

use super::binding::{bind_current, clear_skip, remove_marker, resolve, unbind_current, Binding};
use super::clock::utc_now;
use super::entry::{self, parse_store, CodeState, GitState, LedgerState, NewEntry};
use super::session::Session;

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
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ListRow {
    pub id: String,
    pub mission_id: Option<String>,
    pub mission_link: String,
    pub latest_whats_next: Option<String>,
    pub latest_timestamp: Option<String>,
    /// The actors bound to the thread, oldest binding first. Empty when none is.
    pub actors: Vec<ListActor>,
}

/// One actor bound to a thread, as `tsk thread list` prints it.
#[derive(Debug, PartialEq, Serialize)]
pub struct ListActor {
    pub actor: String,
    pub kind: &'static str,
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

fn entries_of(store: &dyn LedgerStore, id: &str) -> Result<Vec<entry::StoredEntry>, String> {
    let lines = store.continuation_list(id)?;
    parse_store(&lines.join("\n"), &store.describe_thread(id))
}

/// The file ledger's `threads/<id>/index.md`. A thread names its mission on a `Mission:`
/// line. A thread for a mission with no record also links the mission's briefing. A
/// thread with no mission has neither line.
pub fn index_md(id: &str, mission_id: Option<&str>, briefing_path: Option<&str>) -> String {
    let mut text = format!("# Thread {}\n", id);
    if let Some(mission_id) = mission_id {
        text.push_str(&format!("\nMission: {}\n", mission_id));
    }
    if let (Some(_), Some(path)) = (mission_id, briefing_path) {
        text.push_str(&format!("Mission briefing: [{}]({})\n", path, path));
    }
    text
}

/// The mission ID on the `Mission:` line of a thread's `index.md`, if it has one.
pub fn index_mission(index_md: &str) -> Option<String> {
    index_md
        .lines()
        .find_map(|line| line.strip_prefix("Mission: "))
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
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

/// The briefing document of a mission that has no record: a document at
/// `missions/<category>/<ID>-<slug>.md` or `missions/<category>/<ID>.md`. The first in
/// path order when several match.
pub fn find_briefing(store: &dyn LedgerStore, mission_id: &str) -> Result<Option<String>, String> {
    Ok(store.doc_list("missions/")?.into_iter().find(|path| {
        let parts: Vec<&str> = path.split('/').collect();
        parts.len() == 3 && mission_id_of_file(parts[2]).as_deref() == Some(mission_id)
    }))
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
    session.refresh()?;
    let store = session.store.as_ref();
    if let Some(binding) = resolve(session)? {
        return Ok(StartOutcome::ResumeRequired(
            binding.thread_id().to_string(),
        ));
    }
    let briefing = match mission_id {
        Some(mission_id) => checked_mission(store, mission_id)?,
        None => None,
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
    store.thread_set_mission(&id, mission_id, briefing.as_deref())?;
    session.push(&match mission_id {
        Some(mission_id) => format!("Set mission of thread {} to {}", id, mission_id),
        None => format!("Detach thread {} from its mission", id),
    })?;
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

pub fn pause(session: &Session, id: &str, task_id: &str, whats_next: &str) -> Result<(), String> {
    check_thread_id(id)?;
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
    let ledger_commit = store.head_id()?;
    let code_git = &session.repo.git;
    let code_ref = current_branch_ref(code_git)?;
    let code_commit = code_git.run_line(["rev-parse", "--verify", "HEAD^{commit}"])?;
    ensure_on_an_origin_branch(code_git, &code_ref, &code_commit)?;
    let timestamp = utc_now();
    let written_by = session.actor_urn();
    store.continuation_append(
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
        }
        .to_json(),
    )?;
    session.push(&format!("Pause thread {}: {}", id, task_id))
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

fn ensure_on_an_origin_branch(git: &Git, code_ref: &str, commit: &str) -> Result<(), String> {
    git.run([
        "fetch",
        "--quiet",
        "--prune",
        "origin",
        ORIGIN_HEADS_REFSPEC,
    ])?;
    let listing = git.run([
        "for-each-ref",
        "--contains",
        commit,
        "--format=%(refname)",
        ORIGIN_REMOTE_PREFIX,
    ])?;
    if origin_branches(&listing).is_empty() {
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

pub fn prior_actors(entries: &[entry::StoredEntry]) -> BTreeSet<String> {
    entries
        .iter()
        .filter_map(|stored| stored.entry.written_by.clone())
        .filter(|actor| !actor.is_empty())
        .collect()
}

pub fn takeover_warning(id: &str, prior: &BTreeSet<String>, current: &str) -> String {
    if prior.is_empty() || prior.contains(current) {
        return String::new();
    }
    format!(
        "thread {} is already associated with other actor(s): {}",
        id,
        prior.iter().cloned().collect::<Vec<_>>().join(",")
    )
}

pub fn resume(session: &Session, id: &str) -> Result<ResumeOutcome, String> {
    check_thread_id(id)?;
    session.refresh()?;
    let store = session.store.as_ref();
    if !store.thread_exists(id)? {
        return Err(format!("error: no such thread: {}", id));
    }
    let entries = entries_of(store, id)?;
    let current = session.actor_urn();
    let warning = takeover_warning(id, &prior_actors(&entries), &current);

    // A cloud session pushes its binding. A worktree binding is a local ledger change,
    // pushed with the next push, as a record write is.
    let bound = match &session.cloud_session {
        Some(_) => Binding::Cloud(id.to_string()),
        None => Binding::Worktree(id.to_string()),
    };
    if resolve(session)? != Some(bound) {
        bind_current(session, id)?;
        if session.cloud_session.is_some() {
            session.push(&format!("Bind {} to thread {}", current, id))?;
        }
    }
    clear_skip(session)?;

    Ok(ResumeOutcome {
        thread_id: id.to_string(),
        latest: entries
            .last()
            .map(|stored| stored.raw.clone())
            .unwrap_or_else(|| "{}".to_string()),
        warning,
    })
}

pub fn render_resume(outcome: &ResumeOutcome) -> String {
    format!(
        "{{\"thread_id\":{},\"latest\":{},\"warning\":{}}}",
        serde_json::Value::String(outcome.thread_id.clone()),
        outcome.latest,
        serde_json::Value::String(outcome.warning.clone())
    )
}

pub fn detach(session: &Session) -> Result<String, String> {
    session.refresh()?;
    let binding = resolve(session)?.ok_or_else(|| {
        "error: no thread binding found for this session or worktree; nothing to detach".to_string()
    })?;
    // A cloud session pushes its detach. A worktree detach is a local ledger change,
    // pushed with the next push.
    if unbind_current(session)? == BindingKind::Cloud {
        session.push(&format!(
            "Detach {} from thread {}",
            session.actor_urn(),
            binding.thread_id()
        ))?;
    }
    Ok(binding.thread_id().to_string())
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

pub fn mission_link(index_md: &str) -> String {
    const MARK: &str = "Mission briefing: [";
    index_md
        .lines()
        .find_map(|line| {
            line.find(MARK).map(|at| {
                let rest = &line[at + MARK.len()..];
                rest.split(']').next().unwrap_or("").to_string()
            })
        })
        .unwrap_or_default()
}

pub fn list_rows(store: &dyn LedgerStore) -> Result<Vec<ListRow>, String> {
    let bindings = store.bindings_list()?;
    let mut rows = Vec::new();
    for name in store.thread_ids()? {
        let info = store.thread_info(&name)?;
        let entries = entries_of(store, &name)?;
        let latest = entries.last().map(|stored| &stored.entry);
        rows.push(ListRow {
            mission_id: thread_mission(&info),
            mission_link: info.briefing_path.unwrap_or_default(),
            latest_whats_next: latest.and_then(|e| e.whats_next.clone()),
            latest_timestamp: latest.and_then(|e| e.timestamp.clone()),
            actors: bindings
                .iter()
                .filter(|binding| binding.thread_id == name)
                .map(|binding| ListActor {
                    actor: binding.actor.clone(),
                    kind: binding.kind.as_str(),
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

pub fn list(session: &Session) -> Result<Vec<ListRow>, String> {
    session.refresh()?;
    list_rows(session.store.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn index_md_names_the_mission_and_links_a_briefing_only_when_given() {
        assert_eq!(
            index_md("1234abcd", Some("M-X"), Some("missions/M-X.md")),
            "# Thread 1234abcd\n\nMission: M-X\nMission briefing: [missions/M-X.md](missions/M-X.md)\n"
        );
        assert_eq!(
            index_md("1234abcd", Some("M-X"), None),
            "# Thread 1234abcd\n\nMission: M-X\n"
        );
    }

    #[test]
    fn index_md_of_a_thread_with_no_mission_has_no_mission_line() {
        let text = index_md("1234abcd", None, None);
        assert_eq!(text, "# Thread 1234abcd\n");
        assert_eq!(index_mission(&text), None);
        assert_eq!(mission_link(&text), "");
        // A briefing without a mission is not written.
        assert_eq!(index_md("1234abcd", None, Some("m.md")), text);
    }

    #[test]
    fn index_mission_reads_the_mission_line() {
        assert_eq!(
            index_mission(&index_md("1234abcd", Some("M-X"), Some("missions/M-X.md"))).as_deref(),
            Some("M-X")
        );
        assert_eq!(
            index_mission("# Thread\n\nMission briefing: [a](a)\n"),
            None
        );
        assert_eq!(index_mission("Mission: \n"), None);
    }

    #[test]
    fn mission_link_reads_the_first_briefing_link() {
        assert_eq!(
            mission_link(&index_md("1234abcd", Some("M-X"), Some("missions/M-X.md"))),
            "missions/M-X.md"
        );
        assert_eq!(mission_link(&index_md("1234abcd", Some("M-X"), None)), "");
        assert_eq!(mission_link("# Thread\n\nno link\n"), "");
        assert_eq!(
            mission_link("Mission briefing: [open ended\n"),
            "open ended"
        );
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

    fn actors(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_warning_without_prior_actors_or_when_current_is_among_them() {
        assert_eq!(takeover_warning("1234abcd", &actors(&[]), "urn:a"), "");
        assert_eq!(
            takeover_warning("1234abcd", &actors(&["urn:a", "urn:b"]), "urn:a"),
            ""
        );
    }

    #[test]
    fn warning_names_every_other_actor_sorted_and_comma_joined() {
        assert_eq!(
            takeover_warning("1234abcd", &actors(&["urn:c", "urn:b"]), "urn:a"),
            "thread 1234abcd is already associated with other actor(s): urn:b,urn:c"
        );
    }

    #[test]
    fn render_resume_passes_the_latest_entry_through_as_stored() {
        let outcome = ResumeOutcome {
            thread_id: "1234abcd".to_string(),
            latest: "{\"whats_next\":\"x\",\"git\":{\"ledger\":{\"commit\":\"b\"}}}".to_string(),
            warning: String::new(),
        };
        assert_eq!(
            render_resume(&outcome),
            "{\"thread_id\":\"1234abcd\",\"latest\":{\"whats_next\":\"x\",\"git\":{\"ledger\":{\"commit\":\"b\"}}},\"warning\":\"\"}"
        );
    }

    fn row(id: &str, ts: Option<&str>) -> ListRow {
        ListRow {
            id: id.to_string(),
            mission_id: None,
            mission_link: String::new(),
            latest_whats_next: None,
            latest_timestamp: ts.map(str::to_string),
            actors: Vec::new(),
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
            mission_id: Some("M-X".to_string()),
            mission_link: "m.md".to_string(),
            latest_whats_next: None,
            latest_timestamp: None,
            actors: vec![
                ListActor {
                    actor: "cse_1".to_string(),
                    kind: "cloud",
                },
                ListActor {
                    actor: "tsk-1a2b/main".to_string(),
                    kind: "worktree",
                },
            ],
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            "{\"id\":\"1234abcd\",\"mission_id\":\"M-X\",\"mission_link\":\"m.md\",\"latest_whats_next\":null,\"latest_timestamp\":null,\"actors\":[{\"actor\":\"cse_1\",\"kind\":\"cloud\"},{\"actor\":\"tsk-1a2b/main\",\"kind\":\"worktree\"}]}"
        );
        assert!(serde_json::to_string(&row("1234abcd", None))
            .unwrap()
            .ends_with("\"actors\":[]}"));
    }
}
