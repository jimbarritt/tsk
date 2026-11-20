use super::store::{ActorRow, BindingRow, InstanceRow, LedgerStore, SessionActorRow, SessionRow};
use crate::thread::session::Session;

#[derive(Debug, Default)]
pub struct Identity {
    binding: Option<BindingRow>,
    instance: Option<InstanceRow>,
    sessions: Vec<SessionRow>,
    session_actors: Vec<SessionActorRow>,
    actors: Vec<ActorRow>,
}

pub fn capture(store: &dyn LedgerStore) -> Identity {
    let Ok(session) = Session::discover() else {
        return Identity::default();
    };
    let Ok(Some((key, _))) = session.actor_key() else {
        return Identity::default();
    };
    let urn = session.session_urn();
    read(store, &key, urn.as_deref()).unwrap_or_default()
}

fn read(store: &dyn LedgerStore, key: &str, urn: Option<&str>) -> Result<Identity, String> {
    let binding = store
        .bindings_list()?
        .into_iter()
        .find(|row| row.instance == key);
    let instance = store
        .instances_list()?
        .into_iter()
        .find(|row| row.instance == key);
    let sessions: Vec<SessionRow> = store
        .sessions_list()?
        .into_iter()
        .filter(|row| row.instance == key || Some(row.session.as_str()) == urn)
        .collect();
    let session_actors: Vec<SessionActorRow> = store
        .session_actors_list()?
        .into_iter()
        .filter(|row| sessions.iter().any(|s| s.session == row.session))
        .collect();
    let actors = store
        .actors_list()?
        .into_iter()
        .filter(|actor| {
            session_actors
                .iter()
                .any(|row| Some(row.actor_uid.as_str()) == actor.uid.as_deref())
        })
        .collect();
    Ok(Identity {
        binding,
        instance,
        sessions,
        session_actors,
        actors,
    })
}

pub trait Target {
    fn actors_list(&self) -> Result<Vec<ActorRow>, String>;
    fn actor_add(&self, row: &ActorRow) -> Result<(), String>;
    fn instances_list(&self) -> Result<Vec<InstanceRow>, String>;
    fn instance_set(&self, row: &InstanceRow) -> Result<(), String>;
    fn sessions_list(&self) -> Result<Vec<SessionRow>, String>;
    fn session_add(&self, row: &SessionRow) -> Result<(), String>;
    fn session_actors_list(&self) -> Result<Vec<SessionActorRow>, String>;
    fn session_actor_set(&self, row: &SessionActorRow) -> Result<(), String>;
    fn bindings_list(&self) -> Result<Vec<BindingRow>, String>;
    fn binding_set(&self, row: &BindingRow) -> Result<(), String>;
    fn thread_exists(&self, id: &str) -> Result<bool, String>;
}

impl Target for dyn LedgerStore + '_ {
    fn actors_list(&self) -> Result<Vec<ActorRow>, String> {
        LedgerStore::actors_list(self)
    }
    fn actor_add(&self, row: &ActorRow) -> Result<(), String> {
        LedgerStore::actor_add(self, row)
    }
    fn instances_list(&self) -> Result<Vec<InstanceRow>, String> {
        LedgerStore::instances_list(self)
    }
    fn instance_set(&self, row: &InstanceRow) -> Result<(), String> {
        LedgerStore::instance_set(self, row)
    }
    fn sessions_list(&self) -> Result<Vec<SessionRow>, String> {
        LedgerStore::sessions_list(self)
    }
    fn session_add(&self, row: &SessionRow) -> Result<(), String> {
        LedgerStore::session_add(self, row)
    }
    fn session_actors_list(&self) -> Result<Vec<SessionActorRow>, String> {
        LedgerStore::session_actors_list(self)
    }
    fn session_actor_set(&self, row: &SessionActorRow) -> Result<(), String> {
        LedgerStore::session_actor_set(self, row)
    }
    fn bindings_list(&self) -> Result<Vec<BindingRow>, String> {
        LedgerStore::bindings_list(self)
    }
    fn binding_set(&self, row: &BindingRow) -> Result<(), String> {
        LedgerStore::binding_set(self, row)
    }
    fn thread_exists(&self, id: &str) -> Result<bool, String> {
        LedgerStore::thread_exists(self, id)
    }
}

pub fn restore<T: Target + ?Sized>(store: &T, kept: &Identity) -> Vec<String> {
    let mut notes = Vec::new();
    if let Err(e) = restore_into(store, kept, &mut notes) {
        notes.push(format!(
            "warning: the discard completed, but this session's robot and thread binding were not fully restored: {}",
            e.trim_start_matches("error: ")
        ));
    }
    notes
}

fn restore_into<T: Target + ?Sized>(
    store: &T,
    kept: &Identity,
    notes: &mut Vec<String>,
) -> Result<(), String> {
    let actors = store.actors_list()?;
    let mut robot = None;
    for actor in &kept.actors {
        if actors.iter().any(|row| row.uid == actor.uid) {
            continue;
        }
        if actors.iter().any(|row| row.name == actor.name) {
            eprintln!(
                "not kept: the actor {} exists on origin with another uid",
                actor.name
            );
            continue;
        }
        store.actor_add(actor)?;
        robot = Some(actor.name.clone());
    }
    if let Some(row) = &kept.instance {
        if !store
            .instances_list()?
            .iter()
            .any(|held| held.instance == row.instance)
        {
            store.instance_set(row)?;
        }
    }
    let sessions = store.sessions_list()?;
    for row in &kept.sessions {
        if !sessions.iter().any(|held| held.session == row.session) {
            store.session_add(row)?;
        }
    }
    let session_actors = store.session_actors_list()?;
    let actors = store.actors_list()?;
    for row in &kept.session_actors {
        let known = actors
            .iter()
            .any(|actor| actor.uid.as_deref() == Some(row.actor_uid.as_str()));
        let held = session_actors
            .iter()
            .any(|held| held.session == row.session && held.actor_uid == row.actor_uid);
        if known && !held {
            store.session_actor_set(row)?;
        }
    }
    if let Some(name) = robot {
        notes.push(format!("kept this session's robot {}", name));
    }
    if let Some(row) = &kept.binding {
        let held = store
            .bindings_list()?
            .iter()
            .any(|held| held.instance == row.instance);
        if !held && store.thread_exists(&row.thread_id)? {
            store.binding_set(row)?;
            notes.push(format!("kept this session's binding to {}", row.thread_id));
        } else if !held {
            eprintln!(
                "not kept: the thread {} is not on origin, so this session is unbound",
                row.thread_id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::store::{ActorKind, ActorRole};
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        actors: RefCell<Vec<ActorRow>>,
        session_actors: RefCell<Vec<SessionActorRow>>,
        fail_sessions: bool,
    }

    impl Target for Fake {
        fn actors_list(&self) -> Result<Vec<ActorRow>, String> {
            Ok(self.actors.borrow().clone())
        }
        fn actor_add(&self, row: &ActorRow) -> Result<(), String> {
            self.actors.borrow_mut().push(row.clone());
            Ok(())
        }
        fn instances_list(&self) -> Result<Vec<InstanceRow>, String> {
            Ok(Vec::new())
        }
        fn instance_set(&self, _: &InstanceRow) -> Result<(), String> {
            Ok(())
        }
        fn sessions_list(&self) -> Result<Vec<SessionRow>, String> {
            if self.fail_sessions {
                return Err("error: the store is locked".to_string());
            }
            Ok(Vec::new())
        }
        fn session_add(&self, _: &SessionRow) -> Result<(), String> {
            Ok(())
        }
        fn session_actors_list(&self) -> Result<Vec<SessionActorRow>, String> {
            Ok(self.session_actors.borrow().clone())
        }
        fn session_actor_set(&self, row: &SessionActorRow) -> Result<(), String> {
            self.session_actors.borrow_mut().push(row.clone());
            Ok(())
        }
        fn bindings_list(&self) -> Result<Vec<BindingRow>, String> {
            Ok(Vec::new())
        }
        fn binding_set(&self, _: &BindingRow) -> Result<(), String> {
            Ok(())
        }
        fn thread_exists(&self, _: &str) -> Result<bool, String> {
            Ok(true)
        }
    }

    fn actor(name: &str, uid: &str) -> ActorRow {
        ActorRow {
            name: name.to_string(),
            kind: ActorKind::Robot,
            registered_at: "2026-01-01T00:00:00Z".to_string(),
            uid: Some(uid.to_string()),
        }
    }

    fn kept(name: &str, uid: &str) -> Identity {
        Identity {
            binding: None,
            instance: None,
            sessions: Vec::new(),
            session_actors: vec![SessionActorRow {
                session: "urn:tsk:session:claude:1".to_string(),
                actor_uid: uid.to_string(),
                role: ActorRole::Primary,
                joined_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            actors: vec![actor(name, uid)],
        }
    }

    #[test]
    fn a_store_error_becomes_a_warning_after_the_actor_is_restored() {
        let store = Fake {
            fail_sessions: true,
            ..Fake::default()
        };
        let notes = restore(&store, &kept("Robot Bee", "uid-1"));
        assert_eq!(store.actors.borrow().len(), 1);
        assert_eq!(notes.len(), 1);
        assert!(
            notes[0].starts_with("warning: the discard completed"),
            "{:?}",
            notes
        );
        assert!(notes[0].contains("the store is locked"), "{:?}", notes);
    }

    #[test]
    fn a_skipped_actor_keeps_no_session_row() {
        let store = Fake::default();
        store
            .actors
            .borrow_mut()
            .push(actor("Robot Bee", "uid-other"));
        let notes = restore(&store, &kept("Robot Bee", "uid-1"));
        assert!(notes.is_empty(), "{:?}", notes);
        assert!(store.session_actors.borrow().is_empty());
        assert_eq!(store.actors.borrow().len(), 1);
    }

    #[test]
    fn an_actor_that_is_added_gets_its_session_row() {
        let store = Fake::default();
        let notes = restore(&store, &kept("Robot Bee", "uid-1"));
        assert_eq!(
            notes,
            vec!["kept this session's robot Robot Bee".to_string()]
        );
        assert_eq!(store.session_actors.borrow().len(), 1);
    }
}
