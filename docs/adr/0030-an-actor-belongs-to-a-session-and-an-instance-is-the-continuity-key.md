# 30. An actor belongs to a session, and a harness instance is the continuity key

Date: 2026-10-30

## Status

Proposed. Worked out by Jim and the robot Plain Statement Of Fact on 2026-10-30 (mission
M-TSK-ADHOC, task T-35). The decisions are recorded on that mission. ADR 0024 (a thread is
the unit of continuation) and ADR 0029 (sessions and harness instances are named by URNs)
stand; this ADR says how the two relate to an actor. It replaces the design in M-TSK-ADHOC
T-33 and M-TSK-NAMES T-13, which are rewritten from it. The design document with the
inventory, the evidence and the options is
`missions/administrative/M-TSK-ADHOC/design-actors-instances-threads-2026-10-30.md` in the
ledger.

## Context

The ledger at layout 11 holds five things that take part in continuing work across sessions:
an actor (`actors`, keyed by name), a harness instance (`instances`, keyed by the cloud
session ID or `<clone-id>/<worktree-name>`), a session (`sessions`, keyed by its URN), a
thread (`threads`) and a binding (`bindings`, keyed by the instance key in a column named
`actor`). A continuation entry records a pause and names its writer by the instance key.

Two links carry the work across a `/clear` or a regenerated cloud session, and both are on
the instance: the binding to the thread, and the registration as an actor. That works
because the instance key is the one thing a new session holds that it did not mint itself. A
terminal session has a new ID at every `/clear`, so it can find nothing by it.

It also leaves three things unclear, found during T-35:

- Who the actor is in a supervised session. M-TSK-NAMES Q-01 says the person. The code
  registers the robot. M-TSK-ATTRIB says the robot, with the person as a co-author.
- What an instance is. It holds the actor, so it is more than a place where a harness runs.
  Jim's reading, 2026-10-30: an instance is infrastructure, the harness, the environment and
  its capabilities, and has no direct connection to an actor.
- What a thread is to its sessions. Jim, 2026-10-30: the thread is the context that binds
  several sessions. Its timeline is a sequence of sessions, each one a unit of execution.
  The thread is the coordination point for the work. The ledger cannot show that timeline,
  because the ledger holds no link from a pause to the session that wrote it.

The data shows the cost. Thread uu44oy4p has 41 bound instances and no actor: one scheduled
task, one firing per day, each a fresh session that never registered. Thread 01d143d5 has
three instances, all the same robot across three regenerations, which the model carries
only because each regeneration registers the name again.

## Decision

1. **The terms.** A *harness instance* is a place where a harness runs: its harness, its
   environment and its capabilities (plugins, connectors). A *session* is one conversation
   in an instance, the unit of execution: it starts, and it ends at a `/clear` or when the
   work stops. An instance has one or many sessions. A *thread* is the coordination point
   for a piece of work: its timeline is its sessions in order, with a continuation entry at
   each pause. An *actor* is who does the work in a session: a human or a robot.

2. **An actor is associated with a session, never with an instance.** A session has one
   primary actor and any number of supporting actors. A human who supervises a robot's
   session is a supporting actor of that session. The session title, the commit author and
   a task claim use the primary actor. The actors of a thread are the actors of its
   sessions: a query over the timeline, not a field on the thread. `instances.actor_name`
   stops being a link. (M-TSK-ADHOC constraint 3.)

3. **An actor is identified by a uid, and its name can change.** The uid is minted at
   registration and is the key every link names. A previous name is kept as a history, with
   the time it held. A registration of an existing name joins that actor, so a regenerated
   session rejoins its robot by name. (M-TSK-ADHOC decision of 2026-10-30; M-TSK-NAMES
   Q-02.)

4. **The harness instance is the continuity key.** A binding links an instance to the
   thread it works, and the key is the instance key. The column is named `instance`, not
   `actor`. The rename is made as a new table, as layout 8 did for `cloud_session_bindings`:
   a renamed primary key column does not merge in Dolt (ADR 0026), and a new table that
   each clone creates and fills the same way does.

5. **A pause names the session that wrote it.** `written_by` of a new continuation entry
   holds the session URN, `urn:tsk:session:<harness>:<id>`. A reader accepts the instance
   URN and the two older forms, because stored entries hold them. A pause is the one
   explicit event a session leaves on its thread.

6. **A new session finds its thread and its actors through its instance.** At session start
   the hook reads the instance key, then: the binding, so the thread; the latest pause on
   that thread written by a session of this instance; that session's actors. The new session
   is recorded with those actors as its own. When no such pause exists, the latest recorded
   session of the same instance supplies the actors instead. Both routes go through the
   instance, and neither stores an actor on it.

7. **A new session with no binding is handled by who started it.** A session a human
   started asks, with suggestions from a lookup: this is a fresh session, or a thread was
   not recorded. A session an agent spawned gets the thread and the actors in its initial
   prompt, as the regenerate skill already passes them.

8. **The session start context prints the current bindings:** the thread, the session URN,
   the instance URN, and the primary and supporting actors.

## Consequences

- **Layout 12**, all additive except the binding table:
  - `thread_bindings` (`instance`, `kind`, `thread_id`, `registered_at`) replaces `bindings`.
    The upgrade creates it, copies every row, and drops `bindings`.
  - `actors` gains `uid`, filled at upgrade. `actor_names` (`actor_uid`, `name`, `from`,
    `until`) holds the name history; the upgrade adds one open row per actor.
  - `session_actors` (`session`, `actor_uid`, `role` in `primary`, `supporting`) holds a
    session's actors. The upgrade gives each recorded session the actor its instance was
    registered as, as primary, so no session loses its actor.
  - `instances.actor_name` stays until the next layout that rebuilds the table, and nothing
    reads it.
- **Commands.** `tsk actor register` prints the uid and joins the current session as its
  primary actor. `tsk actor join <uid> [--session <urn>] [--primary]` adds an actor to a
  session; with no flag, the current one. `tsk session actor` prints the primary actor and
  lists the supporting ones, and `tsk actor name` stays as an alias for a time. `tsk thread
  binding` prints the bindings of decision 8. The `--instance` and `--thread` flags of T-33
  go: an actor is on a session only.
- **Claims.** A task set to `in-progress` records the primary actor's uid and the session
  URN (M-TSK-COMMS T-03), so a reader finds who holds a task and in which conversation.
- **Skills.** The start-thread skill joins the supervising human when there is one. The
  regenerate skill passes the actor uid and the thread, as it passes the name today. A
  scheduled task's prompt registers its robot by name, so a firing joins one actor rather
  than adding an anonymous instance.
- **Commit attribution** (M-TSK-ATTRIB) reads the primary actor of the session. A
  supervised session is one with a supporting human, and T-09 of that mission is answered
  by this.
- **The timeline of a thread is readable from the ledger**: its continuation entries name
  sessions, the sessions name instances and actors. `tsk thread list` can show it.
- **A URN for an actor**, `urn:tsk:actor:<uid>`, follows the form of ADR 0029. The rest of
  the scheme for the domain types (ledger, thread, mission) is a separate decision.
- The dissonance Jim noted stays parked: a session in this model is the unit of execution,
  which is what an operating system calls a thread.
