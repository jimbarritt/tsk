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
- **Q-C. Which thing holds the primary and supporting actors?** Proposal: the instance holds
  the primary actor, the one it registered as. A session holds its supporting actors. The
  reason is the ubiquitous language: operating context (supervised, unsupervised) "is a
  property of the session", and a supervisor attends to a conversation, not to a place where a
  harness runs. A human who supervises joins the session as a supporting actor, and a terminal
  session that changes supervisor at a `/clear` records the change on the new session. A
  thread's actors stay derived from its bound instances and their sessions. `--thread` is
  dropped from `tsk actor join`: a thread with an actor and no instance would be a claim with
  nobody running it. The session title, the commit author and the task claim use the primary
  actor of the instance. This needs the sessions record of Q-A.
- **Q-D. What does a claim record?** Proposal: the actor uid of the primary actor of the
  instance that set the task to `in-progress`, and the session URN beside it. A reader asks
  "who" and gets the actor, or "which conversation" and gets the session, whose instance is
  one lookup away.
- **Q-E. How does a session know it is supervised?** Proposal: it does not infer it. A human
  joins the session as a supporting actor by a command, run by the start-thread skill of an
  interactive session, and a scheduled or spawned session has none. M-TSK-ATTRIB T-09 reads
  the supporting actors of the session. This answers the disagreement in section 4 point 1:
  the robot is the actor, and the person is recorded as supporting.
- **Q-F. Do scheduled firings register an actor?** Proposal: yes, by name in the trigger
  prompt, as the regenerate prompt does, so the 41 firings become one actor with many
  instances. `tsk thread prune-bindings` removes the old rows.
- **Q-G. Is `bindings.actor` renamed?** Proposal: no, at layout 10. The layout document and
  the ubiquitous language say what it holds. A rename waits for a layout change that needs a
  new table anyway.

## 6. One URN scheme for the domain types

ADR 0029 names two types: `urn:tsk:session:<harness>:<id>` and `urn:tsk:instance:<kind>:<key>`.
Jim's direction, 2026-10-30: extend the scheme to every domain type. The rules of ADR 0029
carry over: the prefix `urn:tsk:`, a type segment, a fixed number of further segments, and a
reader that splits on the first N colons only, so the last segment may hold colons.

### The problem of scope

A URN is useful when it names one thing for ever and across ledgers. The identifiers in a
ledger differ in that:

| Type | Identifier today | Unique | Stable |
|---|---|---|---|
| Session | harness ID | within a harness (the URN adds the harness) | yes |
| Instance | cloud session ID, or `<clone-id>/<worktree>` | within a harness, and a clone ID is minted once | yes |
| Actor | name | within a ledger | no: a name can change (Q-B) |
| Thread | 8 characters, minted by `tsk` | within a ledger | yes |
| Mission | internal ID `missions.id`, and the code a person types | within a ledger | the internal ID, yes; the code, no (ADR 0026) |
| Task, decision, question | number per mission | within a mission | no: a publish that merges renumbers a collision (ADR 0022) |
| Document | path on the documents branch | within a ledger | no: `doc mv` renames it |
| Ledger | none | not applicable | not applicable |

So a URN for a thread, mission or task needs the ledger in it, and a ledger has no identifier
of its own today. The manifest holds `repo_id`, which is a nexus entry ID or absent for an
in-repo ledger, and unique only inside one nexus.

### The proposed forms

```
urn:tsk:ledger:<ledger-id>
urn:tsk:actor:<uid>
urn:tsk:thread:<ledger-id>:<thread-id>
urn:tsk:mission:<ledger-id>:<mission-internal-id>
urn:tsk:session:<harness>:<id>                       (ADR 0029)
urn:tsk:instance:<kind>:<key>                         (ADR 0029)
```

- `<ledger-id>` is a UUID minted at ledger creation, in a new manifest key. A ledger that has
  none gets one at the next write (a layout change, additive).
- `<uid>` of an actor is a UUID. It is globally unique, so an actor URN needs no ledger.
- A mission URN uses `missions.id`, not the code, so a rename (`tsk mission mv`) does not
  change it.
- A task, decision and question have no stable identifier until each row has a uid of its own.
  Options: give each row a uid column (additive, filled at upgrade) and name it
  `urn:tsk:task:<ledger-id>:<uid>`; or name them by mission and number and accept that the
  name is valid only after a publish. The first costs a column per table and makes the name
  stable. The second costs nothing and breaks on the one case ADR 0022 handles.
- A document has no stable name either. A document URN would need the same step as a task: a
  uid per document, kept in a record. This design leaves documents without a URN.

### Where a URN is used

- `Agent-Session` in a commit trailer (ADR 0029).
- An actor trailer on a commit, `Actor: urn:tsk:actor:<uid>`, in place of the plain
  `Tsk-Actor: <name>` of M-TSK-NAMES T-11, so a commit still names the right actor after a
  rename.
- `written_by` of a new continuation entry holds the instance URN. A reader accepts the old
  forms, as ADR 0029 says.
- A task claim (M-TSK-COMMS T-03) records the actor URN and the session URN.
- Output for a program: `--json` of a record prints its URN beside its ID.

### Open questions for the scheme

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
