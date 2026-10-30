//! Who wrote a continuation entry. `written_by` holds a session URN since layout 12 (ADR
//! 0030), the instance key before it, and two older URN forms before that. A reader maps
//! each value to the harness instance and the actor behind it, so the takeover warning
//! compares actors, and only falls back to instances when an actor is unknown.

use std::collections::BTreeSet;

use crate::ledger::store::{ActorRole, LedgerStore};

use super::entry::StoredEntry;

/// What a `written_by` value names, as far as the ledger knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Writer {
    /// The instance key, or `None` for an old-format value that names no known instance.
    pub instance: Option<String>,
    /// The uid of the primary actor of the session, or of the instance's latest session,
    /// or of the instance's own registration.
    pub actor_uid: Option<String>,
    /// How the writer is named to a person: the actor's name, else the instance key, else
    /// the value as stored.
    pub label: String,
}

/// The rows a resolver needs, read once.
pub struct Writers {
    sessions: Vec<crate::ledger::store::SessionRow>,
    session_actors: Vec<crate::ledger::store::SessionActorRow>,
    actors: Vec<crate::ledger::store::ActorRow>,
    instances: Vec<crate::ledger::store::InstanceRow>,
}

impl Writers {
    pub fn load(store: &dyn LedgerStore) -> Result<Writers, String> {
        Ok(Writers {
            sessions: store.sessions_list()?,
            session_actors: store.session_actors_list()?,
            actors: store.actors_list()?,
            instances: store.instances_list()?,
        })
    }

    #[cfg(test)]
    pub fn empty() -> Writers {
        Writers {
            sessions: Vec::new(),
            session_actors: Vec::new(),
            actors: Vec::new(),
            instances: Vec::new(),
        }
    }

    fn primary_of(&self, session_urn: &str) -> Option<String> {
        self.session_actors
            .iter()
            .find(|row| row.session == session_urn && row.role == ActorRole::Primary)
            .map(|row| row.actor_uid.clone())
    }

    fn name_of(&self, uid: &str) -> Option<String> {
        self.actors
            .iter()
            .find(|actor| actor.uid.as_deref() == Some(uid))
            .map(|actor| actor.name.clone())
    }

    /// The primary actor of the latest session of an instance, else the actor the instance
    /// registered as.
    fn actor_of_instance(&self, instance: &str) -> Option<String> {
        let mut sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|row| row.instance == instance)
            .collect();
        sessions.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        if let Some(uid) = sessions
            .iter()
            .rev()
            .find_map(|row| self.primary_of(&row.session))
        {
            return Some(uid);
        }
        let name = &self
            .instances
            .iter()
            .find(|row| row.instance == instance)?
            .actor_name;
        self.actors
            .iter()
            .find(|actor| &actor.name == name)
            .and_then(|actor| actor.uid.clone())
    }

    pub fn resolve(&self, value: &str) -> Writer {
        let (instance, actor_uid) =
            if let Some(rest) = value.strip_prefix(crate::sessions::URN_PREFIX) {
                let instance = self
                    .sessions
                    .iter()
                    .find(|row| row.session == value)
                    .map(|row| row.instance.clone())
                    .or_else(|| {
                        // A cloud session is named by its instance key.
                        rest.strip_prefix("claude-cloud:").map(str::to_string)
                    });
                let actor = self
                    .primary_of(value)
                    .or_else(|| instance.as_deref().and_then(|i| self.actor_of_instance(i)));
                (instance, actor)
            } else if value.starts_with("urn:") {
                // `urn:tsk:instance:...`, `urn:tsk:cloudsession:...`, `urn:tsk:worktree:...`:
                // none tells two clones' main worktrees apart, so each names an unknown writer.
                (None, None)
            } else {
                (Some(value.to_string()), self.actor_of_instance(value))
            };
        let label = actor_uid
            .as_deref()
            .and_then(|uid| self.name_of(uid))
            .or_else(|| instance.clone())
            .unwrap_or_else(|| value.to_string());
        Writer {
            instance,
            actor_uid,
            label,
        }
    }

    /// The warning for a thread that other actors wrote to, or an empty string. A prior
    /// writer is the current actor when its actor and the current one are both known and
    /// equal, or when either is unknown and the instances are both known and equal; a
    /// thread the current actor has written to needs no warning. Otherwise every prior
    /// writer is named. A writer with no known instance is left out: it names an unknown
    /// writer, and a warning against it would fire for every thread paused before the
    /// instance key was written.
    pub fn takeover_warning(&self, id: &str, entries: &[StoredEntry], current: &str) -> String {
        let current = self.resolve(current);
        if current.instance.is_none() {
            return String::new();
        }
        let mut others = BTreeSet::new();
        for value in entries
            .iter()
            .filter_map(|stored| stored.entry.written_by.as_deref())
            .filter(|value| !value.is_empty())
        {
            let prior = self.resolve(value);
            let Some(prior_instance) = &prior.instance else {
                continue;
            };
            let same = match (&prior.actor_uid, &current.actor_uid) {
                (Some(a), Some(b)) => a == b,
                _ => Some(prior_instance) == current.instance.as_ref(),
            };
            if same {
                return String::new();
            }
            others.insert(prior.label);
        }
        if others.is_empty() {
            return String::new();
        }
        format!(
            "thread {} is already associated with other actor(s): {}",
            id,
            others.into_iter().collect::<Vec<_>>().join(",")
        )
    }

    /// The session URN that wrote the latest entry of `entries` from the instance `instance`,
    /// newest first: the pause a new session of that instance continues from (ADR 0030).
    pub fn latest_session_of_instance(
        &self,
        entries: &[StoredEntry],
        instance: &str,
    ) -> Option<String> {
        entries
            .iter()
            .rev()
            .filter_map(|stored| stored.entry.written_by.as_deref())
            .filter(|value| value.starts_with(crate::sessions::URN_PREFIX))
            .find(|value| self.resolve(value).instance.as_deref() == Some(instance))
            .map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::store::{
        ActorKind, ActorRow, BindingKind, InstanceRow, SessionActorRow, SessionRow,
    };
    use crate::thread::entry::parse_store;

    fn stored_by(written_by: &str) -> StoredEntry {
        let raw = format!("{{\"whats_next\":\"x\",\"written_by\":\"{}\"}}", written_by);
        parse_store(&raw, "test").unwrap().remove(0)
    }

    fn writers() -> Writers {
        Writers {
            sessions: vec![
                SessionRow {
                    session: "urn:tsk:session:claude-cloud:cse_1".to_string(),
                    instance: "cse_1".to_string(),
                    kind: BindingKind::Cloud,
                    started_at: "2026-10-30T01:00:00Z".to_string(),
                },
                SessionRow {
                    session: "urn:tsk:session:claude-cli:s1".to_string(),
                    instance: "c1/main".to_string(),
                    kind: BindingKind::Worktree,
                    started_at: "2026-10-30T02:00:00Z".to_string(),
                },
            ],
            session_actors: vec![
                SessionActorRow {
                    session: "urn:tsk:session:claude-cloud:cse_1".to_string(),
                    actor_uid: "u-robot".to_string(),
                    role: ActorRole::Primary,
                    joined_at: "2026-10-30T01:00:00Z".to_string(),
                },
                SessionActorRow {
                    session: "urn:tsk:session:claude-cli:s1".to_string(),
                    actor_uid: "u-robot".to_string(),
                    role: ActorRole::Primary,
                    joined_at: "2026-10-30T02:00:00Z".to_string(),
                },
            ],
            actors: vec![ActorRow {
                name: "Robot".to_string(),
                kind: ActorKind::Robot,
                registered_at: "2026-10-30T00:00:00Z".to_string(),
                uid: Some("u-robot".to_string()),
            }],
            instances: vec![InstanceRow {
                instance: "c9/main".to_string(),
                kind: BindingKind::Worktree,
                actor_name: "Robot".to_string(),
                registered_at: "2026-10-30T00:00:00Z".to_string(),
            }],
        }
    }

    #[test]
    fn a_session_urn_resolves_to_its_instance_and_primary_actor() {
        let w = writers().resolve("urn:tsk:session:claude-cli:s1");
        assert_eq!(w.instance.as_deref(), Some("c1/main"));
        assert_eq!(w.actor_uid.as_deref(), Some("u-robot"));
        assert_eq!(w.label, "Robot");
    }

    #[test]
    fn an_unrecorded_cloud_session_urn_still_names_its_instance() {
        let w = writers().resolve("urn:tsk:session:claude-cloud:cse_9");
        assert_eq!(w.instance.as_deref(), Some("cse_9"));
        assert_eq!(w.actor_uid, None);
        assert_eq!(w.label, "cse_9");
    }

    #[test]
    fn an_instance_key_resolves_through_its_sessions_or_its_registration() {
        let writers = writers();
        assert_eq!(
            writers.resolve("cse_1").actor_uid.as_deref(),
            Some("u-robot")
        );
        assert_eq!(
            writers.resolve("c9/main").actor_uid.as_deref(),
            Some("u-robot")
        );
        let unknown = writers.resolve("c2/main");
        assert_eq!(unknown.instance.as_deref(), Some("c2/main"));
        assert_eq!(unknown.label, "c2/main");
    }

    #[test]
    fn old_format_values_name_no_instance() {
        for value in [
            "urn:tsk:worktree:.git",
            "urn:tsk:cloudsession:cse_a",
            "urn:tsk:instance:worktree:.git",
        ] {
            assert_eq!(writers().resolve(value).instance, None, "{}", value);
        }
    }

    #[test]
    fn the_same_actor_in_another_instance_does_not_warn_even_beside_other_writers() {
        let entries = vec![
            stored_by("c2/main"),
            stored_by("urn:tsk:session:claude-cloud:cse_1"),
        ];
        assert_eq!(
            writers().takeover_warning("1234abcd", &entries, "urn:tsk:session:claude-cli:s1"),
            ""
        );
    }

    #[test]
    fn another_actor_or_an_unknown_actor_in_another_instance_warns_by_name_or_key() {
        let entries = vec![
            stored_by("urn:tsk:session:claude-cloud:cse_1"),
            stored_by("c2/main"),
            stored_by("urn:tsk:worktree:.git"),
        ];
        assert_eq!(
            writers().takeover_warning("1234abcd", &entries, "c3/main"),
            "thread 1234abcd is already associated with other actor(s): Robot,c2/main"
        );
    }

    #[test]
    fn the_current_instance_never_warns_against_itself_and_an_old_format_current_never_warns() {
        let entries = vec![stored_by("c2/main")];
        assert_eq!(
            Writers::empty().takeover_warning("1234abcd", &entries, "c2/main"),
            ""
        );
        assert_eq!(
            Writers::empty().takeover_warning("1234abcd", &entries, "urn:tsk:worktree:.git"),
            ""
        );
    }

    #[test]
    fn the_latest_session_of_an_instance_is_found_newest_first() {
        let entries = vec![
            stored_by("urn:tsk:session:claude-cli:s1"),
            stored_by("urn:tsk:session:claude-cloud:cse_1"),
            stored_by("c1/main"),
        ];
        assert_eq!(
            writers()
                .latest_session_of_instance(&entries, "c1/main")
                .as_deref(),
            Some("urn:tsk:session:claude-cli:s1")
        );
        assert_eq!(
            writers().latest_session_of_instance(&entries, "c7/main"),
            None
        );
    }
}
