//! The `tsk session` commands, and the record of a session at session start. A session is
//! one run of a harness: a cloud session, or a Claude Code or Copilot CLI session in a code
//! worktree, which has a new session for each start and each `/clear`. It is named by a URN,
//! `urn:tsk:session:<harness>:<id>`, and each recorded session is a row of `sessions` that
//! names the harness instance it ran in (ADR 0029).

use clap::Subcommand;
use serde::Serialize;

use crate::ledger::store::{ActorRole, LedgerStore, SessionRow};
use crate::mission::table::print_lines;
use crate::thread::clock::utc_now;
use crate::thread::session::{instance_urn, Session};
use crate::thread::session_start::HookFormat;

/// The start of every session URN.
pub const URN_PREFIX: &str = "urn:tsk:session:";

#[derive(Subcommand)]
pub enum SessionCommands {
    #[command(
        about = "Print the harness instance of a session, and the actor that instance registered as, one line: session URN, instance URN, actor name (- when none). Takes the URN or its id alone. Exit status 1 when no session matches. A session is recorded at session start, so a commit made before layout 11 or in a session that did not run the hook has no row"
    )]
    Find {
        #[arg(help = "A session URN, urn:tsk:session:<harness>:<id>, or the id alone")]
        session: String,
    },
    #[command(
        about = "List the recorded sessions, one per line: session URN, instance URN, actor name (- when none), start time. The columns are aligned with spaces; --porcelain prints them tab-separated; --json prints the full records"
    )]
    List {
        #[arg(long, help = "Print the full records as JSON")]
        json: bool,
        #[arg(
            long,
            help = "Print tab-separated columns, the stable form for scripts, instead of aligned columns"
        )]
        porcelain: bool,
    },
    #[command(
        about = "Print the actors of this session, one per line: role (primary or supporting), name, kind, uid; the primary first. Prints no actor registered when the session has none. --session names another session by its URN"
    )]
    Actor {
        #[arg(
            long,
            value_name = "URN",
            help = "The session, by URN; the current session when omitted"
        )]
        session: Option<String>,
    },
}

/// The name of the harness that minted a session ID. Claude Code in the cloud and Claude Code
/// on a machine are two harnesses, because they mint different ID forms (ADR 0029).
pub fn harness_name(format: HookFormat, cloud: bool) -> &'static str {
    match (format, cloud) {
        (HookFormat::Claude, true) => "claude-cloud",
        (HookFormat::Claude, false) => "claude-cli",
        (HookFormat::Copilot, _) => "copilot-cli",
    }
}

/// `urn:tsk:session:<harness>:<id>`.
pub fn session_urn(harness: &str, id: &str) -> String {
    format!("{}{}:{}", URN_PREFIX, harness, id)
}

/// The URN of the session that `tsk` runs in, or `None` when the session has no ID. A cloud
/// session is named by the cloud session ID, which is also its instance key. Any other
/// session is named by the ID the harness gives its hook.
pub fn current_urn(
    format: HookFormat,
    cloud_session: Option<&str>,
    hook_session_id: Option<&str>,
) -> Option<String> {
    match cloud_session {
        Some(id) => Some(session_urn(harness_name(format, true), id)),
        None => hook_session_id
            .filter(|id| !id.is_empty() && !id.contains(char::is_whitespace))
            .map(|id| session_urn(harness_name(format, false), id)),
    }
}

/// The row to record for a session start, or `None` when the session has no URN or the
/// harness instance has no key.
pub fn row_for(
    session: &Session,
    format: HookFormat,
    hook_session_id: Option<&str>,
    now: &str,
) -> Result<Option<SessionRow>, String> {
    let Some(urn) = current_urn(format, session.cloud_session.as_deref(), hook_session_id) else {
        return Ok(None);
    };
    let Some((instance, kind)) = session.actor_key()? else {
        return Ok(None);
    };
    Ok(Some(SessionRow {
        session: urn,
        instance,
        kind,
        started_at: now.to_string(),
    }))
}

/// Record the session at session start, and remember its URN for the later commands of the
/// session. A session that is already recorded changes nothing in the ledger.
pub fn record(
    session: &Session,
    format: HookFormat,
    hook_session_id: Option<&str>,
) -> Result<Option<String>, String> {
    match row_for(session, format, hook_session_id, &utc_now())? {
        Some(row) => {
            session.remember_session_urn(&row.session)?;
            session.store.session_add(&row)?;
            Ok(Some(row.session))
        }
        None => Ok(None),
    }
}

/// One session as `tsk session list --json` prints it.
#[derive(Debug, PartialEq, Serialize)]
pub struct JsonSession {
    pub session: String,
    pub instance: String,
    pub instance_urn: String,
    pub kind: &'static str,
    pub actor: Option<String>,
    pub started_at: String,
}

/// The sessions with the instance URN and the primary actor of each, in the order given. A
/// session with no actor row of its own shows the actor its instance registered as, the
/// link a ledger below layout 12 holds.
pub fn listed(store: &dyn LedgerStore) -> Result<Vec<JsonSession>, String> {
    let instances = store.instances_list()?;
    let actors = store.actors_list()?;
    let session_actors = store.session_actors_list()?;
    Ok(store
        .sessions_list()?
        .into_iter()
        .map(|row| JsonSession {
            actor: session_actors
                .iter()
                .find(|a| a.session == row.session && a.role == ActorRole::Primary)
                .and_then(|a| {
                    actors
                        .iter()
                        .find(|actor| actor.uid.as_deref() == Some(a.actor_uid.as_str()))
                })
                .map(|actor| actor.name.clone())
                .or_else(|| {
                    instances
                        .iter()
                        .find(|i| i.instance == row.instance)
                        .map(|i| i.actor_name.clone())
                }),
            instance_urn: instance_urn(row.kind, &row.instance),
            session: row.session,
            instance: row.instance,
            kind: row.kind.as_str(),
            started_at: row.started_at,
        })
        .collect())
}

/// The sessions that `wanted` names: the URN, or the id after the harness segment.
pub fn matching<'a>(sessions: &'a [JsonSession], wanted: &str) -> Vec<&'a JsonSession> {
    let wanted = wanted.trim();
    sessions
        .iter()
        .filter(|s| {
            s.session == wanted
                || (!wanted.starts_with(URN_PREFIX) && s.session.ends_with(&format!(":{}", wanted)))
        })
        .collect()
}

fn line(session: &JsonSession) -> String {
    format!(
        "{}\t{}\t{}",
        session.session,
        session.instance_urn,
        session.actor.as_deref().unwrap_or("-")
    )
}

pub fn run(action: SessionCommands) -> Result<i32, String> {
    let session = Session::discover()?;
    let sessions = listed(session.store.as_ref())?;
    match action {
        SessionCommands::Find { session: wanted } => {
            let found = matching(&sessions, &wanted);
            if found.is_empty() {
                eprintln!("error: no session is recorded for {}", wanted.trim());
                return Ok(1);
            }
            for session in found {
                println!("{}", line(session));
            }
            Ok(0)
        }
        SessionCommands::Actor { session: target } => {
            let urn = match target {
                Some(urn) => urn,
                None => session.session_urn().ok_or_else(|| {
                    "error: this session has no recorded URN; pass --session <urn>, or run tsk thread session-start first".to_string()
                })?,
            };
            let rows = crate::actor::actors_of_session(session.store.as_ref(), &urn)?;
            if rows.is_empty() {
                println!("no actor registered");
            }
            for (role, actor) in rows {
                println!(
                    "{}\t{}\t{}\t{}",
                    role.as_str(),
                    actor.name,
                    actor.kind.as_str(),
                    actor.uid.as_deref().unwrap_or("-")
                );
            }
            Ok(0)
        }
        SessionCommands::List { json, porcelain } => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&sessions)
                        .map_err(|e| format!("error: could not print JSON: {}", e))?
                );
            } else {
                print_lines(
                    sessions
                        .iter()
                        .map(|s| format!("{}\t{}", line(s), s.started_at))
                        .collect(),
                    porcelain,
                );
            }
            Ok(0)
        }
    }
}

/// Give a new session the actors of the session it continues (ADR 0030, decision 6). With
/// actors of its own already, nothing changes. Else the latest pause on the bound thread
/// written by a session of this instance supplies them; else the latest recorded session of
/// this instance that has a primary actor; else the actor the instance registered as (the
/// link a ledger below layout 12 holds). A ledger that has no `ledger_id` is below layout
/// 12: nothing is written, because a session start is not a write that upgrades.
pub fn continue_actors(session: &Session, urn: &str) -> Result<(), String> {
    use crate::ledger::store::{ActorRole, SessionActorRow};
    use crate::thread::writers::Writers;

    let store = session.store.as_ref();
    if store.ledger_id()?.is_none() {
        return Ok(());
    }
    let session_actors = store.session_actors_list()?;
    if session_actors.iter().any(|row| row.session == urn) {
        return Ok(());
    }
    let Some((instance, _)) = session.actor_key()? else {
        return Ok(());
    };
    let writers = Writers::load(store)?;
    let now = utc_now();
    // The pause this instance continues from.
    let from_pause = crate::thread::binding::resolve_local(session)?
        .and_then(|binding| crate::thread::ops::entries_of(store, binding.thread_id()).ok())
        .and_then(|entries| writers.latest_session_of_instance(&entries, &instance));
    let mut rows: Vec<SessionActorRow> = Vec::new();
    if let Some(previous) = from_pause {
        rows = session_actors
            .iter()
            .filter(|row| row.session == previous)
            .cloned()
            .collect();
    }
    if rows.is_empty() {
        // The latest session of this instance that has actors.
        let mut sessions: Vec<_> = store
            .sessions_list()?
            .into_iter()
            .filter(|row| row.instance == instance && row.session != urn)
            .collect();
        sessions.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        for previous in sessions.iter().rev() {
            let found: Vec<SessionActorRow> = session_actors
                .iter()
                .filter(|row| row.session == previous.session)
                .cloned()
                .collect();
            if !found.is_empty() {
                rows = found;
                break;
            }
        }
    }
    if rows.is_empty() {
        if let Some(uid) = writers.resolve(&instance).actor_uid {
            rows.push(SessionActorRow {
                session: urn.to_string(),
                actor_uid: uid,
                role: ActorRole::Primary,
                joined_at: now.clone(),
            });
        }
    }
    // The primary first, so a replace of the primary never demotes it.
    rows.sort_by_key(|row| row.role != ActorRole::Primary);
    for row in rows {
        store.session_actor_set(&SessionActorRow {
            session: urn.to_string(),
            actor_uid: row.actor_uid,
            role: row.role,
            joined_at: now.clone(),
        })?;
    }
    Ok(())
}

/// The lines after the binding in `tsk thread binding` and the session start context: the
/// session URN, the instance URN, and one line per actor of the session (ADR 0030, decision
/// 8). Empty when the session has no URN.
pub fn binding_lines(session: &Session) -> Result<Vec<String>, String> {
    let Some(urn) = session.session_urn() else {
        return Ok(Vec::new());
    };
    let mut lines = vec![format!("session: {}", urn)];
    if let Some((instance, kind)) = session.actor_key()? {
        lines.push(format!("instance: {}", instance_urn(kind, &instance)));
    }
    for (role, actor) in crate::actor::actors_of_session(session.store.as_ref(), &urn)? {
        lines.push(format!(
            "{}: {} ({}) {}",
            role.as_str(),
            actor.name,
            actor.kind.as_str(),
            actor
                .uid
                .as_deref()
                .map(crate::actor::actor_urn)
                .unwrap_or_default()
        ));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(session: &str, instance: &str, actor: Option<&str>) -> JsonSession {
        JsonSession {
            session: session.to_string(),
            instance: instance.to_string(),
            instance_urn: format!("urn:tsk:instance:worktree:{}", instance),
            kind: "worktree",
            actor: actor.map(str::to_string),
            started_at: "2026-10-30T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn the_harness_is_named_from_the_hook_format_and_whether_the_session_is_a_cloud_one() {
        assert_eq!(harness_name(HookFormat::Claude, true), "claude-cloud");
        assert_eq!(harness_name(HookFormat::Claude, false), "claude-cli");
        assert_eq!(harness_name(HookFormat::Copilot, false), "copilot-cli");
        assert_eq!(harness_name(HookFormat::Copilot, true), "copilot-cli");
    }

    #[test]
    fn a_session_urn_has_the_prefix_the_harness_and_the_id() {
        assert_eq!(
            session_urn("claude-cloud", "cse_01abc"),
            "urn:tsk:session:claude-cloud:cse_01abc"
        );
    }

    #[test]
    fn a_cloud_session_is_named_by_the_cloud_session_id() {
        assert_eq!(
            current_urn(HookFormat::Claude, Some("cse_01abc"), Some("uuid")).as_deref(),
            Some("urn:tsk:session:claude-cloud:cse_01abc")
        );
    }

    #[test]
    fn a_cli_session_is_named_by_the_id_the_hook_gives() {
        assert_eq!(
            current_urn(HookFormat::Claude, None, Some("3f2a-9c1e")).as_deref(),
            Some("urn:tsk:session:claude-cli:3f2a-9c1e")
        );
        assert_eq!(
            current_urn(HookFormat::Copilot, None, Some("abc")).as_deref(),
            Some("urn:tsk:session:copilot-cli:abc")
        );
    }

    #[test]
    fn a_cli_session_with_no_usable_id_has_no_urn() {
        assert_eq!(current_urn(HookFormat::Claude, None, None), None);
        assert_eq!(current_urn(HookFormat::Claude, None, Some("")), None);
        assert_eq!(current_urn(HookFormat::Claude, None, Some("a b")), None);
    }

    #[test]
    fn a_session_is_found_by_its_urn_or_by_its_id_alone() {
        let sessions = vec![
            sample("urn:tsk:session:claude-cli:aaa", "c1/main", Some("Robot")),
            sample("urn:tsk:session:claude-cli:bbb", "c1/main", Some("Robot")),
            sample("urn:tsk:session:claude-cloud:cse_1", "cse_1", None),
        ];
        assert_eq!(
            matching(&sessions, "urn:tsk:session:claude-cli:aaa").len(),
            1
        );
        assert_eq!(
            matching(&sessions, "bbb")[0].session,
            "urn:tsk:session:claude-cli:bbb"
        );
        assert_eq!(matching(&sessions, "cse_1").len(), 1);
        assert!(matching(&sessions, "urn:tsk:session:claude-cli:cse_1").is_empty());
        assert!(matching(&sessions, "aa").is_empty());
    }

    #[test]
    fn a_line_names_the_session_the_instance_and_the_actor() {
        assert_eq!(
            line(&sample(
                "urn:tsk:session:claude-cli:aaa",
                "c1/main",
                Some("Robot")
            )),
            "urn:tsk:session:claude-cli:aaa\turn:tsk:instance:worktree:c1/main\tRobot"
        );
        assert!(line(&sample("s", "c1/main", None)).ends_with("\t-"));
    }
}
