# Design: actors, harness instances, sessions, code worktrees and threads

Mission M-TSK-ADHOC, task T-35. Date: 2026-10-30. Robot: Plain Statement Of Fact. Second draft,
after Jim's ruling that a session is a term of the model and ADR 0029 (URNs for sessions and instances).

This document states what each of the five things is in `tsk` today, how they relate, where
the model is unclear, and the questions that need an answer before T-33 (actor uid and
`tsk actor join`) is built. The terms are those of `docs/domain/ubiquitous-language.md` and `docs/adr/0029-sessions-and-harness-instances-are-named-by-urns.md`.
The tables are those of `docs/domain/ledger-layout.md` at layout 10.

Evidence: the ledger of this repo on 2026-10-30 (`tsk actor list`, `tsk thread list`), the
code (`cli/src/thread/session.rs`, `cli/src/actor/mod.rs`), and the records of M-TSK-NAMES,
M-TSK-ATTRIB and M-TSK-COMMS.

## 1. What each is today

| Term | What it is | Where it is recorded | Key | Written by |
|---|---|---|---|---|
| Actor | A human or a robot, by name and kind. Identity of its own, outside any instance. | `actors` | `name` (1 to 200 characters) | `tsk actor register` adds a row for a new name. Nothing deletes or renames one. |
| Harness instance | A running harness. The thing that registers and binds. Named `urn:tsk:instance:<kind>:<key>` (ADR 0029). | `instances` | `instance`: the cloud session ID `cse_...`, or `<clone-id>/<worktree-name>` | `tsk actor register` sets or replaces the row, with `actor_name`. |
| Session | One conversation context in a harness: one start, and in a terminal each `/clear` starts a new one. A harness instance runs one or many sessions. In the cloud the session ID survives `/clear`, so an instance has one. Named `urn:tsk:session:<harness>:<id>` (ADR 0029). | Not yet a record. The URN is written in a commit trailer (`Agent-Session`). ADR 0029 decision 5 adds a record, a task of its own. | The ID the harness mints, with the harness name | The harness |
| Code worktree | A worktree of the repo's code. Locally it is the instance key. | Not a record. Appears as the `worktree` kind of an instance or a binding. | `<clone-id>/<worktree-name>` | The clone ID is minted once into `.git/tsk-clone-id`. |
| Thread | The execution sequence: the unit of continuation. Has at most one mission, or none. | `threads` | `thread_id` (8 characters, minted by `tsk`) | `tsk thread start` mints it. `tsk thread set mission` changes its mission. |
| Binding | The link from an instance to the thread it works. One per instance. | `bindings` | `actor` (the instance key; the column keeps its old name), `kind` | `tsk thread start`, `resume`, `switch`. `detach` removes one. `stop` removes every binding of the thread. |
| Continuation entry | A snapshot of a thread at a pause. | `continuation_entries` | (`thread_id`, `seq`) | `tsk thread pause`. `written_by` holds the instance key. |

Three facts follow from the table:

- The ledger records actor, instance, thread, binding and entry. It does not record a
  session. A session is named only in a commit trailer, and `written_by` names the instance.
- The two links are on the instance. An instance has one actor (`instances.actor_name`) and one
  thread (`bindings.thread_id`). A thread has its mission (`threads.mission_id`).
- A session and an instance differ in lifetime and in place. An instance is a place where a
  harness runs, and it lasts. A session is one conversation in it, and it ends at a `/clear`
  or at the end of the work. The thread continues across sessions, which is why the binding
  is on the instance and not on the session.

## 2. The relationships as they stand

```
mission 1 ---- 0..* thread 1 ---- 0..* binding 1 ---- 1 instance 1 ---- 1..* session
                                                          |
                                                          0..* ---- 0..1 actor (by name)
```

- Mission to thread: one to many. A thread has at most one mission at a time, and may have
  none. The mission is on the thread, and only there.
- Thread to instance: one to many, through bindings. An instance has one binding, so an
  instance works one thread. Several instances can work one thread at once.
- Instance to session: one to many. In the cloud the two are one to one in effect. In a terminal
  an instance (a code worktree) has a new session at each `/clear`, and every one of them
  continues the thread of the instance.
- Instance to actor: many to one, by name. An instance has at most one actor. An actor has
  any number of instances, at once or over time. The same name in a new instance is that
  actor returning.
- Thread to actor: derived, not stored. The actors of a thread are the actors of its bound
  instances. `written_by` on an entry names an instance, so "who wrote this" goes through
  `instances` to a name, and the name can be null.
- Human to anything: a human actor is a row of `actors` with kind `human`. Nothing in the
  model connects a human to an instance a robot runs, so a supervised session has no record of
  its supervisor.

## 2a. The thread as a timeline of sessions

Jim, 2026-10-30: the thread is the context that binds several sessions. Its timeline is a
sequence of sessions, each one a unit of execution. The thread is the coordination point for
the work. (A session is then closer to a thread of an operating system than to the thread of
this model. That dissonance is noted and parked.)

Thread 01d143d5 as that timeline:

| Order | Session | Instance | Actor | What happened |
|---|---|---|---|---|
| 1 | `cse_01NPz…` | cloud instance 1 | Plain Statement Of Fact | started the thread, worked T-37, paused |
| 2 | `cse_01XoX…` | cloud instance 2 | Plain Statement Of Fact | resumed, updated tsk, paused for regenerate |
| 3 | `cse_01Sw1…` | cloud instance 3 | Plain Statement Of Fact | resumed, working now |

In the cloud each session is in its own instance. In a terminal the same table has one
instance in every row and a new session at each `/clear`.

The three relationships that follow:

1. **Thread to session: one to many, in time.** A session works one thread. The thread's
   history is its sessions in order, with a continuation entry at each pause.
2. **Session to actor: one primary, any number supporting.** The actor is who did that unit
   of execution. The actors of a thread are the actors of its sessions: a query over the
   timeline, not a field on the thread.
3. **Session to instance: many to one.** The instance is where the session ran: harness,
   environment, capabilities. Jim, 2026-10-30: an instance is infrastructural and has no
   direct connection to an actor.

Today the ledger holds the binding and the registration on the instance, and no session. The
timeline above is therefore not readable from the ledger. It was read from the cloud
session records and the `written_by` of the continuation entries.

## 2b. What happens at `/clear` today

The steps, from `plugin/hooks/session-start.sh`, `ops/local/claude-session-start.sh` and
`cli/src/thread/session_start.rs` at 0.37.1:

1. Claude Code ends the conversation. The process stays. In a terminal it mints a new session
   ID. In the cloud the session ID (`cse_…`) stays the same.
2. Claude Code fires `SessionStart` with `source: "clear"` and the session ID, to the repo's
   hook and to the plugin's hook. Both run `tsk thread session-start` with that input.
3. `tsk thread session-start` claims the event by session ID and source, so the second run
   exits with no output. The source is used for nothing else: `tsk` is told of the `/clear`
   and keeps no record of it.
4. It starts the Dolt server if none runs, and in the cloud upgrades `tsk` and the plugin.
5. `Session::discover` reads the instance key: `CLAUDE_CODE_REMOTE_SESSION_ID` in the cloud,
   or `<clone-id>/<worktree-name>` from the git directory. The new terminal session ID is
   read from the hook input for the claim and then dropped.
6. The ledger is fetched and the store refreshed.
7. The binding is resolved by the instance key: the `bindings` row. **This is where the
   thread crosses the `/clear`: through the instance, not the session.**
8. The actor is resolved by the same key: the `instances` row, then the `actors` row by name.
   **This is where the actor crosses the `/clear`, the same way.**
9. The context is printed: the mission, the latest continuation entry, a takeover warning when
   `written_by` of that entry is another instance key, the actor, and the session title.
   The git identity of the robot is written again from the attribution settings.
10. Claude reads the context and runs the resume-thread skill, which writes its summary from
    the context and runs no command.

What is not written at a `/clear`: no continuation entry (a pause is a person's act), no
session record, and no change to the binding. The work of the session that ended is on origin
only if it paused or the `Stop` hook published it.

So the ledger sees a terminal instance as one long session, and sees a cloud instance as one
session that never clears. The sequence of sessions on a thread exists in the harness and in
`git` (the `Agent-Session` trailer), and not in the ledger.

## 3. What the data shows

From `tsk thread list` and `tsk actor list` on 2026-10-30:

- Five actors, all robots. One, "Noted, Not Expanded", has no instance: a robot that
  registered a name and never bound. No command removes it (M-TSK-NAMES T-12).
- Thread 01d143d5 has three instances, all "Plain Statement Of Fact": one robot across three
  cloud sessions, each a regeneration of the last. The model carries this well.
- Thread uu44oy4p (M-TSK-VULNS) has 41 bound instances, all with no actor name. Each is one
  firing of the daily scheduled task. The thread is worked by one robot in fact, and the ledger
  shows 41 anonymous instances and no actor. The cost is one `bindings` row per firing and a
  resume warning that lists them all.
- Thread 3376badb has one anonymous instance and two named ones. An instance that bound before
  the actor tables existed has no name and never gets one.

## 4. Where the model is unclear

1. **Two records say who an actor is, and they disagree.** M-TSK-NAMES Q-01: in a supervised
   session the actor is the person supervising. The code: the robot registers as the actor,
   and the person is nowhere. M-TSK-ATTRIB: a supervised session writes `Co-Authored-By: Jim`,
   so the robot is the author and the person a second party. T-33: the person joins as a
   supporting actor.
2. **Name as identity against non-unique names.** `actors.name` is the key and `register`
   joins an existing name. M-TSK-NAMES Q-02: names need not be unique, each actor has a guid.
   T-33 point 6: `register` creates only, and refuses an existing name. The three cannot all
   hold.
3. **Where a primary and supporting actor would live.** T-33 names three targets: the session,
   the instance and the thread, and lets `--session`, `--instance` and `--thread` be passed
   together. Today an actor is only on the instance. Whether a thread can have an actor that
   no instance of it has, and what "session" is when it is not the instance, is not stated.
4. **Thread or instance as the unit a claim names.** M-TSK-COMMS T-03: a task claim records the
   uid of the actor that holds it. `written_by` records the instance. Which one a reader
   follows to find "who has this" is not stated.
5. **The column `bindings.actor` holds an instance key.** The name is a leftover. A rename
   changes the schema, and ADR 0026 recorded that a changed primary key does not merge in
   Dolt. A new column is safe; a renamed key is not.
6. **Scheduled firings have no actor.** A scheduled task starts a fresh session each time, and
   the prompt does not register a name, so the firings are anonymous instances on one thread.

## 5. Open questions

The questions are in the order they need answering. Each has the answer I propose.

- **Q-A. Is "session" a term of the model?** Settled by Jim, 2026-10-30: yes. A session is a
  valid concept and it differs from an instance. The model has both. What stays open is the
  record of sessions that ADR 0029 decision 5 adds: its columns (the session URN, the instance
  key, the start time, and whether an end time is kept) and whether the actor and supervision
  links of Q-C sit on it. T-33 keeps both flags, `--session` and `--instance`.
- **Q-B. Is an actor identified by a uid, with a name that can change?** Settled by Jim,
  2026-10-30: yes. A uid is minted at `register` and is the key of `actors`. `instances` and
  every later link (claims, supporting actors, commit trailers) name the uid. A name is a
  property that can change, and **a previous name is kept as a history**. The name stays unique
  for now, so `register` of an existing name joins that actor and a new name makes one, as
  today. M-TSK-NAMES T-13 and T-33 point 1 are the same work. T-33 point 6 (refuse an existing
  name) is dropped, because a regenerated session must rejoin its actor by name.
- **Q-C. Which thing holds the primary and supporting actors?** Settled by Jim, 2026-10-30:
  the session, and never the instance (constraint 3). An instance is infrastructure and the
  continuity key. `--instance` and `--thread` go from `tsk actor join`.
- **Q-D. What does a claim record?** The primary actor's uid and the session URN.
- **Q-E. How does a session know it is supervised?** A human joins the session as a
  supporting actor; nothing is inferred.
- **Q-F. Do scheduled firings register an actor?** Yes, by name in the trigger prompt.
- **Q-G. Is `bindings.actor` renamed?** Settled by Jim, 2026-10-30: yes, to `instance`, as a
  new table `thread_bindings`, since a renamed key does not merge in Dolt.
- **Continuity at a new session.** Settled by Jim, 2026-10-30: option C, with B as the
  fallback. A pause names the session that wrote it; a new session goes instance, binding,
  thread, the latest pause by its instance, that session's actors. With no such pause, the
  latest session of the instance. With no binding: a human-started session asks, an
  agent-spawned session has the context in its prompt. The session start context prints the
  bindings.

These are written up as ADR 0030,
`docs/adr/0030-an-actor-belongs-to-a-session-and-an-instance-is-the-continuity-key.md`.
Still open:

- **Q-H. Is a ledger identified by a minted UUID?** Proposal: yes. It is the only form that
  stays unique across nexuses, and it survives a repo rename and a move into a nexus.
- **Q-I. Do task, decision and question get a uid?** Proposal: yes, a uid column, so a claim,
  a trailer or a link from another ledger keeps pointing at the right row after a renumber.
  If the first uses are only session, instance, actor and thread, this waits.
- **Q-J. Do documents get a URN?** Proposal: not now. A document is named by its path inside a
  ledger, and `doc mv` already updates every link that names the old path.
- **Q-K. Is the type segment singular and lower case?** Proposal: yes, as in ADR 0029
  (`session`, `instance`). The type names are the domain terms.

## 7. Consequences for T-33, if the proposals stand

- `actors` gets a `uid` column, filled for existing rows at upgrade; `instances` gets
  `actor_uid`; a new `instance_actors` table holds supporting actors (`instance`, `actor_uid`,
  `role`), or `instances` keeps the primary and the new table the supporting ones.
- `actors` keeps the current `name`. A new table `actor_names` (`actor_uid`, `name`, `from`,
  `until`) keeps every earlier name with the time it held it, and the current name has no
  `until`. A rename appends a row and closes the last one. Both tables are additive, so two
  clones that upgrade on their own merge. A lookup by name finds the actor by its current name,
  and by an earlier name when no current name matches, and says that it matched a former name.
- `tsk actor register` prints the uid and the actor URN. `tsk actor join <uid> [--session <urn>]
  [--instance <urn>] [--primary]` is the only new command; with no flag it acts on the current
  session. `tsk actor name` prints the primary and lists the supporting.
- `tsk task set <mission> <task> status in-progress` records the claim (uid and instance).
  `tsk task list` shows the actor's current name.
- `tsk thread list` and `tsk thread resume` show actors by name through the uid, so a
  renamed actor shows its new name everywhere.
- No column is renamed and no key changes, so two clones that upgrade on their own merge.
