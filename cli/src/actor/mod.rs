//! The `tsk actor` commands. An actor is a human or a robot, with a name and a kind, and
//! its identity persists outside any harness instance: a row of `actors`. A harness
//! instance is the running harness, a cloud session or a code worktree of a clone, keyed
//! as `bindings.actor` is. It registers as an actor, which is a row of `instances`, and
//! that is separate from binding to a thread.

use clap::Subcommand;
use serde::Serialize;

use crate::ledger::store::{ActorKind, ActorRow, BindingKind, InstanceRow, LedgerStore};
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
        about = "Print the name of the actor this session or worktree is registered as, or no actor registered"
    )]
    Name,
    #[command(
        name = "git-identity",
        about = "Write the registered robot's git identity to the repo's local git config, when [attribution] identity is robot: user.name, user.email, gpg.format, user.signingkey and commit.gpgsign. The key comes from TSK_ROBOT_SIGNING_KEY. With the default identity, or a human actor, nothing is written. tsk actor register runs it after a registration"
    )]
    GitIdentity,
    #[command(
        about = "List the actors, one per line: name, kind, and the sessions and worktrees registered as it (- for none, comma-separated otherwise). The columns are aligned with spaces; --porcelain prints them tab-separated; --json prints the full records"
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

/// The actor this session or worktree is registered as, from the local ledger store as it
/// is: no fetch. `None` when it registered none, when there is no ledger store yet, and
/// when the clone has no clone ID.
pub fn current(session: &Session) -> Result<Option<Current>, String> {
    if !session.store.exists_locally()? {
        return Ok(None);
    }
    let Some((instance, _)) = session.actor_key()? else {
        return Ok(None);
    };
    let store = session.store.as_ref();
    let Some(row) = store
        .instances_list()?
        .into_iter()
        .find(|row| row.instance == instance)
    else {
        return Ok(None);
    };
    let kind = store
        .actors_list()?
        .into_iter()
        .find(|actor| actor.name == row.actor_name)
        .map(|actor| actor.kind);
    Ok(Some(Current {
        name: row.actor_name,
        kind,
    }))
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
    /// The name was new: the actor was made, and this instance registered as it.
    New,
    /// The name existed with the same kind: this instance registered as it.
    Joined,
}

/// Register the instance `instance` of kind `instance_kind` as the actor `name` of kind
/// `kind`: see [`ActorCommands::Register`].
pub fn register(
    store: &dyn LedgerStore,
    instance: &str,
    instance_kind: BindingKind,
    name: &str,
    kind: ActorKind,
    now: &str,
) -> Result<Registered, String> {
    check_name(name)?;
    let existing = store
        .actors_list()?
        .into_iter()
        .find(|actor| actor.name == name);
    let outcome = match existing {
        Some(actor) if actor.kind != kind => {
            return Err(format!(
                "error: {} is registered as a {}",
                name,
                actor.kind.as_str()
            ))
        }
        Some(_) => Registered::Joined,
        None => {
            store.actor_add(&ActorRow {
                name: name.to_string(),
                kind,
                registered_at: now.to_string(),
            })?;
            Registered::New
        }
    };
    store.instance_set(&InstanceRow {
        instance: instance.to_string(),
        kind: instance_kind,
        actor_name: name.to_string(),
        registered_at: now.to_string(),
    })?;
    Ok(outcome)
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
    /// The harness instances registered as the actor, oldest registration first.
    pub instances: Vec<JsonInstance>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct JsonInstance {
    pub instance: String,
    pub kind: &'static str,
    pub registered_at: String,
}

/// The actors with their instances, in the order of `actors`.
pub fn listed(actors: &[ActorRow], instances: &[InstanceRow]) -> Vec<JsonActor> {
    actors
        .iter()
        .map(|actor| JsonActor {
            name: actor.name.clone(),
            kind: actor.kind.as_str(),
            registered_at: actor.registered_at.clone(),
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

/// The tab-separated line of one actor: name, kind, and its instances joined by commas,
/// or `-` with none.
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
    format!("{}\t{}\t{}", actor.name, actor.kind, instances)
}

pub fn run(action: ActorCommands) -> Result<i32, String> {
    match action {
        ActorCommands::Register { name, kind } => {
            check_name(&name)?;
            let kind = ActorKind::parse(&kind)
                .ok_or_else(|| format!("error: unknown actor kind: {}", kind))?;
            let session = Session::discover()?;
            let (instance, instance_kind) = session.actor_key()?.ok_or_else(no_clone_id)?;
            let outcome = register(
                session.store.as_ref(),
                &instance,
                instance_kind,
                &name,
                kind,
                &utc_now(),
            )?;
            eprintln!("{}", NOT_PUBLISHED_NOTE);
            println!(
                "{}",
                render_registered(&outcome, &name, kind, instance_kind)
            );
            if let Err(error) = crate::attribution::apply(&session) {
                eprintln!(
                    "warning: the registration stands, and the git identity is not set: {}",
                    error.trim_start_matches("error: ")
                );
            }
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
            let actors = listed(&store.actors_list()?, &store.instances_list()?);
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
        };
        assert_eq!(known.label(), "alice (robot)");
        let unknown = Current {
            name: "alice".to_string(),
            kind: None,
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
        let listed = listed(&actors, &instances);
        assert_eq!(
            listed.iter().map(list_line).collect::<Vec<_>>(),
            vec!["bob\thuman\t-", "alice\trobot\tcse_1,c1/main"]
        );
        assert_eq!(
            serde_json::to_string(&listed[1]).unwrap(),
            "{\"name\":\"alice\",\"kind\":\"robot\",\"registered_at\":\"2026-10-24T00:00:00Z\",\
             \"instances\":[{\"instance\":\"cse_1\",\"kind\":\"cloud\",\"registered_at\":\"2026-10-24T00:00:01Z\"},\
             {\"instance\":\"c1/main\",\"kind\":\"cloud\",\"registered_at\":\"2026-10-24T00:00:01Z\"}]}"
        );
    }
}
