//! The `tsk actor` commands. An actor is a human or a robot, with a name and a kind, and
//! its identity persists outside any harness instance: a row of `actors`. A harness
//! instance is the running harness, a cloud session or a code worktree of a clone, keyed
//! as `bindings.actor` is. It registers as an actor, which is a row of `instances`, and
//! that is separate from binding to a thread.

use clap::Subcommand;
use serde::Serialize;

use crate::ledger::store::{
    ActorKind, ActorNameRow, ActorRole, ActorRow, BindingKind, InstanceRow, LedgerStore,
    SessionActorRow,
};
use crate::mission::table::print_lines;
use crate::thread::binding::no_clone_id;
use crate::thread::clock::utc_now;
use crate::thread::session::Session;

/// The longest name `tsk actor register` accepts, in characters: the width of
/// `actors.name`.
pub const NAME_MAX_CHARS: usize = 200;

/// Said on stderr after a registration: a local write (ADR 0019).
const NOT_PUBLISHED_NOTE: &str = "note: the registration is a local write; it goes out with the next push (thread start, pause or stop, or tsk ledger push)";

#[derive(Subcommand)]
pub enum ActorCommands {
    #[command(
        about = "Register this session or worktree as an actor, by name and kind. A new name makes the actor and prints registered <name> (<kind>). A name already registered must have the same kind, and then this session or worktree joins it: <name> (<kind>) is now this session's actor, or this worktree's actor. A name registered with the other kind is refused with exit status 1. Needs no thread binding. The change is a local write: it goes out with the next push (thread start, pause or stop, or tsk ledger push)"
    )]
    Register {
        #[arg(help = "The actor's name: 1 to 200 characters, not starting with /")]
        name: String,
        #[arg(
            long,
            value_parser = ["human", "robot"],
            help = "What the actor is: a person, or a program such as a coding agent"
        )]
        kind: String,
    },
    #[command(
        about = "Add an actor, by uid, to the current session as a supporting actor, or as its primary actor with --primary (the earlier primary becomes supporting). --session names another session by its URN. Refuses an unknown uid. A human who supervises a robot's session joins it this way. A local write"
    )]
    Join {
        #[arg(
            help = "The actor's uid, as tsk actor list prints it, or its URN urn:tsk:actor:<uid>"
        )]
        uid: String,
        #[arg(long, help = "Make the actor the session's primary actor")]
        primary: bool,
        #[arg(
            long,
            value_name = "URN",
            help = "The session to join, by URN; the current session when omitted"
        )]
        session: Option<String>,
    },
    #[command(
        about = "Print the name of the primary actor of this session, or no actor registered. The actor is read from the session, else from the latest session of this harness instance, else from the instance's own registration (a ledger below layout 12)"
    )]
    Name,
    #[command(
        name = "git-identity",
        about = "Write the registered robot's git identity to the repo's local git config, when [attribution] identity is robot: user.name, user.email, gpg.format, user.signingkey and commit.gpgsign. The key comes from TSK_ROBOT_SIGNING_KEY. With the default identity, or a human actor, nothing is written. tsk actor register runs it after a registration"
    )]
    GitIdentity,
    #[command(
        about = "List the actors, one per line: name, kind, uid, and the harness instances that ran as it (- for none, comma-separated otherwise). The columns are aligned with spaces; --porcelain prints them tab-separated; --json prints the full records with the URN and the name history"
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
}

/// The actor a harness instance is registered as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Current {
    pub name: String,
    /// The kind from the `actors` row. `None` when the instance names an actor that has
    /// no row, which no `tsk` command writes.
    pub kind: Option<ActorKind>,
    /// The uid of the actor (ADR 0030). `None` on a ledger below layout 12.
    pub uid: Option<String>,
}

impl Current {
    /// `<name> (<kind>)`, or the name alone with no kind.
    pub fn label(&self) -> String {
        match self.kind {
            Some(kind) => format!("{} ({})", self.name, kind.as_str()),
            None => self.name.clone(),
        }
    }
}

/// The primary actor of this session, from the local ledger store as it is: no fetch. Read
/// from the session's own actors; else from the latest recorded session of the same harness
/// instance that has a primary; else from the instance's own registration, the link a
/// ledger below layout 12 holds (ADR 0030). `None` when none of them names an actor, when
/// there is no ledger store yet, and when the clone has no clone ID.
pub fn current(session: &Session) -> Result<Option<Current>, String> {
    if !session.store.exists_locally()? {
        return Ok(None);
    }
    let Some((instance, _)) = session.actor_key()? else {
        return Ok(None);
    };
    let store = session.store.as_ref();
    let actors = store.actors_list()?;
    let by_uid = |uid: &str| -> Option<Current> {
        actors
            .iter()
            .find(|actor| actor.uid.as_deref() == Some(uid))
            .map(|actor| Current {
                name: actor.name.clone(),
                kind: Some(actor.kind),
                uid: actor.uid.clone(),
            })
    };
    let session_actors = store.session_actors_list()?;
    let primary_of = |urn: &str| -> Option<Current> {
        session_actors
            .iter()
            .find(|row| row.session == urn && row.role == ActorRole::Primary)
            .and_then(|row| by_uid(&row.actor_uid))
    };
    if let Some(urn) = session.session_urn() {
        if let Some(current) = primary_of(&urn) {
            return Ok(Some(current));
        }
    }
    // The latest session of this instance that has a primary actor.
    let mut sessions: Vec<_> = store
        .sessions_list()?
        .into_iter()
        .filter(|row| row.instance == instance)
        .collect();
    sessions.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    if let Some(current) = sessions
        .iter()
        .rev()
        .find_map(|row| primary_of(&row.session))
    {
        return Ok(Some(current));
    }
    let Some(row) = store
        .instances_list()?
        .into_iter()
        .find(|row| row.instance == instance)
    else {
        return Ok(None);
    };
    let actor = actors.iter().find(|actor| actor.name == row.actor_name);
    Ok(Some(Current {
        name: row.actor_name,
        kind: actor.map(|actor| actor.kind),
        uid: actor.and_then(|actor| actor.uid.clone()),
    }))
}

/// The actors of a session, as `tsk session actor` prints them: the primary first, then the
/// supporting actors in the order they joined. Empty when the session has none recorded.
pub fn actors_of_session(
    store: &dyn LedgerStore,
    urn: &str,
) -> Result<Vec<(ActorRole, ActorRow)>, String> {
    let actors = store.actors_list()?;
    let mut rows: Vec<(ActorRole, ActorRow)> = store
        .session_actors_list()?
        .into_iter()
        .filter(|row| row.session == urn)
        .filter_map(|row| {
            actors
                .iter()
                .find(|actor| actor.uid.as_deref() == Some(row.actor_uid.as_str()))
                .map(|actor| (row.role, actor.clone()))
        })
        .collect();
    rows.sort_by_key(|(role, _)| *role != ActorRole::Primary);
    Ok(rows)
}

/// The start of every actor URN (ADR 0030).
pub const URN_PREFIX: &str = "urn:tsk:actor:";

/// `urn:tsk:actor:<uid>`.
pub fn actor_urn(uid: &str) -> String {
    format!("{}{}", URN_PREFIX, uid)
}

/// The uid in a `tsk actor join` argument: the uid itself, or the URN.
pub fn uid_of_argument(text: &str) -> &str {
    text.trim().strip_prefix(URN_PREFIX).unwrap_or(text.trim())
}

/// Join a session: the actor `uid` becomes its primary or a supporting actor. Refuses an
/// unknown uid.
pub fn join(
    store: &dyn LedgerStore,
    session_urn: &str,
    uid: &str,
    role: ActorRole,
    now: &str,
) -> Result<ActorRow, String> {
    let actor = store
        .actors_list()?
        .into_iter()
        .find(|actor| actor.uid.as_deref() == Some(uid))
        .ok_or_else(|| {
            format!(
                "error: no actor has the uid {}; tsk actor list prints the uids",
                uid
            )
        })?;
    store.session_actor_set(&SessionActorRow {
        session: session_urn.to_string(),
        actor_uid: uid.to_string(),
        role,
        joined_at: now.to_string(),
    })?;
    Ok(actor)
}

/// The check a name passes before it is registered: not empty, at most
/// [`NAME_MAX_CHARS`] characters, and not starting with `/`.
pub fn check_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("error: the name is empty".to_string());
    }
    let length = name.chars().count();
    if length > NAME_MAX_CHARS {
        return Err(format!(
            "error: the name is {} characters long; the limit is {}",
            length, NAME_MAX_CHARS
        ));
    }
    if name.starts_with('/') {
        return Err("error: a name cannot start with /".to_string());
    }
    Ok(())
}

/// What `tsk actor register` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Registered {
    /// The name was new: the actor was made, and this session registered as it.
    New,
    /// The name existed with the same kind: this session registered as it.
    Joined,
}

/// Register as the actor `name` of kind `kind`: see [`ActorCommands::Register`]. The actor
/// becomes the primary actor of the session `session_urn` when one is known (ADR 0030). The
/// instance row is written too, for a reader of the older link. Returns what happened and
/// the actor's uid, which is `None` only on a ledger that has no uids yet.
pub fn register(
    store: &dyn LedgerStore,
    instance: &str,
    instance_kind: BindingKind,
    session_urn: Option<&str>,
    name: &str,
    kind: ActorKind,
    now: &str,
) -> Result<(Registered, Option<String>), String> {
    check_name(name)?;
    let existing = store
        .actors_list()?
        .into_iter()
        .find(|actor| actor.name == name);
    let (outcome, uid) = match existing {
        Some(actor) if actor.kind != kind => {
            return Err(format!(
                "error: {} is registered as a {}",
                name,
                actor.kind.as_str()
            ))
        }
        Some(actor) => (Registered::Joined, actor.uid),
        None => {
            let uid = crate::ledger::dolt::mint_uuid();
            store.actor_add(&ActorRow {
                name: name.to_string(),
                kind,
                registered_at: now.to_string(),
                uid: Some(uid.clone()),
            })?;
            (Registered::New, Some(uid))
        }
    };
    store.instance_set(&InstanceRow {
        instance: instance.to_string(),
        kind: instance_kind,
        actor_name: name.to_string(),
        registered_at: now.to_string(),
    })?;
    // An actor that joined an existing name on a ledger upgraded by this write has a uid
    // now: read it back.
    let uid = match uid {
        Some(uid) => Some(uid),
        None => store
            .actors_list()?
            .into_iter()
            .find(|actor| actor.name == name)
            .and_then(|actor| actor.uid),
    };
    if let (Some(urn), Some(uid)) = (session_urn, uid.as_deref()) {
        store.session_actor_set(&SessionActorRow {
            session: urn.to_string(),
            actor_uid: uid.to_string(),
            role: ActorRole::Primary,
            joined_at: now.to_string(),
        })?;
    }
    Ok((outcome, uid))
}

/// The line `tsk actor register` prints.
pub fn render_registered(
    outcome: &Registered,
    name: &str,
    kind: ActorKind,
    instance_kind: BindingKind,
) -> String {
    match outcome {
        Registered::New => format!("registered {} ({})", name, kind.as_str()),
        Registered::Joined => format!(
            "{} ({}) is now this {}'s actor",
            name,
            kind.as_str(),
            match instance_kind {
                BindingKind::Cloud => "session",
                BindingKind::Worktree => "worktree",
            }
        ),
    }
}

/// One actor as `tsk actor list --json` prints it.
#[derive(Debug, PartialEq, Serialize)]
pub struct JsonActor {
    pub name: String,
    pub kind: &'static str,
    pub registered_at: String,
    /// The uid (ADR 0030), and the URN `urn:tsk:actor:<uid>`. Null on a ledger below layout 12.
    pub uid: Option<String>,
    pub urn: Option<String>,
    /// The names the actor has held, oldest first; the last has no `until`.
    pub names: Vec<JsonName>,
    /// The harness instances registered as the actor, oldest registration first.
    pub instances: Vec<JsonInstance>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct JsonName {
    pub name: String,
    pub from: String,
    pub until: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct JsonInstance {
    pub instance: String,
    pub kind: &'static str,
    pub registered_at: String,
}

/// The actors with their name history and instances, in the order of `actors`.
pub fn listed(
    actors: &[ActorRow],
    instances: &[InstanceRow],
    names: &[ActorNameRow],
) -> Vec<JsonActor> {
    actors
        .iter()
        .map(|actor| JsonActor {
            name: actor.name.clone(),
            kind: actor.kind.as_str(),
            registered_at: actor.registered_at.clone(),
            uid: actor.uid.clone(),
            urn: actor.uid.as_deref().map(actor_urn),
            names: names
                .iter()
                .filter(|row| Some(row.actor_uid.as_str()) == actor.uid.as_deref())
                .map(|row| JsonName {
                    name: row.name.clone(),
                    from: row.from_at.clone(),
                    until: row.until_at.clone(),
                })
                .collect(),
            instances: instances
                .iter()
                .filter(|row| row.actor_name == actor.name)
                .map(|row| JsonInstance {
                    instance: row.instance.clone(),
                    kind: row.kind.as_str(),
                    registered_at: row.registered_at.clone(),
                })
                .collect(),
        })
        .collect()
}

/// The tab-separated line of one actor: name, kind, uid (`-` with none), and its instances
/// joined by commas, or `-` with none.
pub fn list_line(actor: &JsonActor) -> String {
    let instances = if actor.instances.is_empty() {
        "-".to_string()
    } else {
        actor
            .instances
            .iter()
            .map(|row| row.instance.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "{}\t{}\t{}\t{}",
        actor.name,
        actor.kind,
        actor.uid.as_deref().unwrap_or("-"),
        instances
    )
}

pub fn run(action: ActorCommands) -> Result<i32, String> {
    match action {
        ActorCommands::Register { name, kind } => {
            check_name(&name)?;
            let kind = ActorKind::parse(&kind)
                .ok_or_else(|| format!("error: unknown actor kind: {}", kind))?;
            let session = Session::discover()?;
            let (instance, instance_kind) = session.actor_key()?.ok_or_else(no_clone_id)?;
            let urn = session.session_urn();
            let (outcome, uid) = register(
                session.store.as_ref(),
                &instance,
                instance_kind,
                urn.as_deref(),
                &name,
                kind,
                &utc_now(),
            )?;
            eprintln!("{}", NOT_PUBLISHED_NOTE);
            println!(
                "{}",
                render_registered(&outcome, &name, kind, instance_kind)
            );
            if let Some(uid) = uid {
                println!("{}", actor_urn(&uid));
            }
            if urn.is_none() && instance_kind == BindingKind::Worktree {
                eprintln!("note: this session has no recorded URN (the session start did not run), so the actor is recorded on the harness instance only");
            }
            if let Err(error) = crate::attribution::apply(&session) {
                eprintln!(
                    "warning: the registration stands, and the git identity is not set: {}",
                    error.trim_start_matches("error: ")
                );
            }
            Ok(0)
        }
        ActorCommands::Join {
            uid,
            primary,
            session: target,
        } => {
            let session = Session::discover()?;
            let urn = match target {
                Some(urn) => urn,
                None => session.session_urn().ok_or_else(|| {
                    "error: this session has no recorded URN; pass --session <urn>, or run tsk thread session-start first".to_string()
                })?,
            };
            if !urn.starts_with(crate::sessions::URN_PREFIX) {
                return Err(format!(
                    "error: {} is not a session URN (urn:tsk:session:<harness>:<id>)",
                    urn
                ));
            }
            let role = if primary {
                ActorRole::Primary
            } else {
                ActorRole::Supporting
            };
            let actor = join(
                session.store.as_ref(),
                &urn,
                uid_of_argument(&uid),
                role,
                &utc_now(),
            )?;
            eprintln!("{}", NOT_PUBLISHED_NOTE);
            println!(
                "{} ({}) joined {} as {}",
                actor.name,
                actor.kind.as_str(),
                urn,
                role.as_str()
            );
            Ok(0)
        }
        ActorCommands::GitIdentity => crate::attribution::run_git_identity(),
        ActorCommands::Name => {
            let session = Session::discover()?;
            match current(&session)? {
                Some(actor) => println!("{}", actor.name),
                None => println!("no actor registered"),
            }
            Ok(0)
        }
        ActorCommands::List { json, porcelain } => {
            let session = Session::discover()?;
            let store = session.store.as_ref();
            let actors = listed(
                &store.actors_list()?,
                &store.instances_list()?,
                &store.actor_names_list()?,
            );
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&actors)
                        .map_err(|e| format!("error: could not print JSON: {}", e))?
                );
            } else {
                print_lines(actors.iter().map(list_line).collect(), porcelain);
            }
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_not_empty_not_too_long_and_not_a_path() {
        assert_eq!(check_name("alice"), Ok(()));
        assert_eq!(check_name(&"x".repeat(200)), Ok(()));
        assert_eq!(check_name("é".repeat(200).as_str()), Ok(()));
        assert!(check_name("").unwrap_err().contains("empty"));
        assert!(check_name("  ").unwrap_err().contains("empty"));
        let long = check_name(&"x".repeat(201)).unwrap_err();
        assert!(long.contains("201 characters"), "{}", long);
        assert!(long.contains("limit is 200"), "{}", long);
        assert!(check_name("/tmp").unwrap_err().contains("start with /"));
        assert_eq!(check_name("a/b"), Ok(()));
    }

    #[test]
    fn the_label_names_the_kind_when_it_is_known() {
        let known = Current {
            name: "alice".to_string(),
            kind: Some(ActorKind::Robot),
            uid: None,
        };
        assert_eq!(known.label(), "alice (robot)");
        let unknown = Current {
            name: "alice".to_string(),
            kind: None,
            uid: None,
        };
        assert_eq!(unknown.label(), "alice");
    }

    #[test]
    fn register_prints_the_new_or_joined_line_for_the_instance_kind() {
        assert_eq!(
            render_registered(
                &Registered::New,
                "alice",
                ActorKind::Robot,
                BindingKind::Cloud
            ),
            "registered alice (robot)"
        );
        assert_eq!(
            render_registered(
                &Registered::Joined,
                "alice",
                ActorKind::Human,
                BindingKind::Cloud
            ),
            "alice (human) is now this session's actor"
        );
        assert_eq!(
            render_registered(
                &Registered::Joined,
                "alice",
                ActorKind::Robot,
                BindingKind::Worktree
            ),
            "alice (robot) is now this worktree's actor"
        );
    }

    fn actor(name: &str, kind: ActorKind) -> ActorRow {
        ActorRow {
            name: name.to_string(),
            kind,
            registered_at: "2026-10-24T00:00:00Z".to_string(),
            uid: Some(format!("uid-{}", name)),
        }
    }

    fn instance(key: &str, name: &str) -> InstanceRow {
        InstanceRow {
            instance: key.to_string(),
            kind: BindingKind::Cloud,
            actor_name: name.to_string(),
            registered_at: "2026-10-24T00:00:01Z".to_string(),
        }
    }

    #[test]
    fn the_list_groups_the_instances_under_their_actor_in_actor_order() {
        let actors = [
            actor("bob", ActorKind::Human),
            actor("alice", ActorKind::Robot),
        ];
        let instances = [
            instance("cse_1", "alice"),
            instance("c1/main", "alice"),
            instance("cse_9", "nobody"),
        ];
        let names = [ActorNameRow {
            actor_uid: "uid-alice".to_string(),
            name: "alice".to_string(),
            from_at: "2026-10-24T00:00:00Z".to_string(),
            until_at: None,
        }];
        let listed = listed(&actors, &instances, &names);
        assert_eq!(
            listed.iter().map(list_line).collect::<Vec<_>>(),
            vec![
                "bob\thuman\tuid-bob\t-",
                "alice\trobot\tuid-alice\tcse_1,c1/main"
            ]
        );
        assert_eq!(
            serde_json::to_string(&listed[1]).unwrap(),
            "{\"name\":\"alice\",\"kind\":\"robot\",\"registered_at\":\"2026-10-24T00:00:00Z\",\
             \"uid\":\"uid-alice\",\"urn\":\"urn:tsk:actor:uid-alice\",\
             \"names\":[{\"name\":\"alice\",\"from\":\"2026-10-24T00:00:00Z\",\"until\":null}],\
             \"instances\":[{\"instance\":\"cse_1\",\"kind\":\"cloud\",\"registered_at\":\"2026-10-24T00:00:01Z\"},\
             {\"instance\":\"c1/main\",\"kind\":\"cloud\",\"registered_at\":\"2026-10-24T00:00:01Z\"}]}"
        );
    }
}

#[cfg(test)]
mod urn_tests {
    use super::*;

    #[test]
    fn an_actor_urn_names_the_uid_and_a_join_argument_may_be_either() {
        assert_eq!(actor_urn("ab-1"), "urn:tsk:actor:ab-1");
        assert_eq!(uid_of_argument("urn:tsk:actor:ab-1"), "ab-1");
        assert_eq!(uid_of_argument(" ab-1 "), "ab-1");
    }
}
