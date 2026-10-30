# Design: actors, harness instances, sessions, code worktrees and threads

Mission M-TSK-ADHOC, task T-35. Date: 2026-10-30. Robot: Plain Statement Of Fact.

This document states what each of the five things is in `tsk` today, how they relate, where
the model is unclear, and the questions that need an answer before T-33 (actor uid and
`tsk actor join`) is built. The terms are those of `docs/domain/ubiquitous-language.md`.
The tables are those of `docs/domain/ledger-layout.md` at layout 10.

Evidence: the ledger of this repo on 2026-10-30 (`tsk actor list`, `tsk thread list`), the
code (`cli/src/thread/session.rs`, `cli/src/actor/mod.rs`), and the records of M-TSK-NAMES,
M-TSK-ATTRIB and M-TSK-COMMS.

## 1. What each is today

| Term | What it is | Where it is recorded | Key | Written by |
|---|---|---|---|---|
| Actor | A human or a robot, by name and kind. Identity of its own, outside any instance. | `actors` | `name` (1 to 200 characters) | `tsk actor register` adds a row for a new name. Nothing deletes or renames one. |
| Harness instance | A running harness. The thing that registers and binds. | `instances` | `instance`: the cloud session ID `cse_...`, or `<clone-id>/<worktree-name>` | `tsk actor register` sets or replaces the row, with `actor_name`. |
| Session | A platform term. In the cloud, one session is one instance, and the ID survives `/clear`. In a terminal, the CLI mints a new session ID at every `/clear`, so `tsk` uses the code worktree as the instance. | Not a record | None | Nothing |
| Code worktree | A worktree of the repo's code. Locally it is the instance key. | Not a record. Appears as the `worktree` kind of an instance or a binding. | `<clone-id>/<worktree-name>` | The clone ID is minted once into `.git/tsk-clone-id`. |
| Thread | The execution sequence: the unit of continuation. Has at most one mission, or none. | `threads` | `thread_id` (8 characters, minted by `tsk`) | `tsk thread start` mints it. `tsk thread set mission` changes its mission. |
| Binding | The link from an instance to the thread it works. One per instance. | `bindings` | `actor` (the instance key; the column keeps its old name), `kind` | `tsk thread start`, `resume`, `switch`. `detach` removes one. `stop` removes every binding of the thread. |
| Continuation entry | A snapshot of a thread at a pause. | `continuation_entries` | (`thread_id`, `seq`) | `tsk thread pause`. `written_by` holds the instance key. |

Two facts follow from the table:

- The ledger models four of the five: actor, instance, thread and (as a key form) code
  worktree. It does not model a session. "Session" is used in prose and in command help
  ("this session or worktree") to mean the instance.
- The two links are both on the instance. An instance has one actor (`instances.actor_name`)
  and one thread (`bindings.thread_id`). A thread has its mission (`threads.mission_id`).

## 2. The relationships as they stand

```
mission 1 ---- 0..* thread 1 ---- 0..* binding 1 ---- 1 instance 0..* ---- 0..1 actor
                                                                           (by name)
```

- Mission to thread: one to many. A thread has at most one mission at a time, and may have
  none. The mission is on the thread, and only there.
- Thread to instance: one to many, through bindings. An instance has one binding, so an
  instance works one thread. Several instances can work one thread at once.
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

- **Q-A. Is "session" a term of the model?** Proposal: no. The model has the harness instance.
  "Session" is the platform's word for it, and prose and command help say "session" only
  where the platform does. T-33's `--session` flag goes, and `--instance` stays. A terminal
  session is identified by its code worktree, as today.
- **Q-B. Is an actor identified by a uid, with a name that can change?** Proposal: yes. A uid
  is minted at `register` and is the key of `actors`; `instances` and every future link
  (claims, supporting actors) name the uid. The name stays unique for now, so `register` of
  an existing name joins that actor and a new name makes one, as today, and no clash rule is
  needed until names collide in practice. This keeps the current behaviour and adds the uid
  under it. M-TSK-NAMES T-13 and T-33 point 1 are the same work. T-33 point 6 (refuse an
  existing name) is dropped: a regenerated session must be able to rejoin its actor by name.
- **Q-C. Which thing holds the primary and supporting actors?** Proposal: the instance. An
  instance has one primary actor (the one it registered as) and any number of supporting
  actors. A thread's actors stay derived from its bound instances. `tsk actor join <uid>`
  acts on the current instance; `--instance <key>` names another; `--thread` is dropped,
  because a thread with an actor of its own and no instance would be a claim with nobody
  running it. A human who supervises joins the instance as supporting. The session title,
  the commit author and the task claim use the primary actor of the instance.
- **Q-D. What does a claim record?** Proposal: the actor uid of the primary actor of the
  instance that set the task to `in-progress`, and the instance key beside it. A reader asks
  "who" and gets the actor, or "where" and gets the instance.
- **Q-E. How does an instance know it is supervised?** Proposal: it does not infer it. A
  human joins as a supporting actor by a command, in the start-thread skill of an interactive
  session, and a scheduled or spawned session has none. M-TSK-ATTRIB T-09 then reads the
  supporting actors of the instance. This answers the disagreement in section 4 point 1: the
  robot is the actor; the person is recorded as supporting.
- **Q-F. Do scheduled firings register an actor?** Proposal: yes, by name in the trigger
  prompt, as the regenerate prompt does, so the 41 firings become one actor with many
  instances. `tsk thread prune-bindings` removes the old rows.
- **Q-G. Is `bindings.actor` renamed?** Proposal: no, at layout 10. The layout document and
  the ubiquitous language say what it holds. A rename waits for a layout change that needs a
  new table anyway.

## 6. Consequences for T-33, if the proposals stand

- `actors` gets a `uid` column, filled for existing rows at upgrade; `instances` gets
  `actor_uid`; a new `instance_actors` table holds supporting actors (`instance`, `actor_uid`,
  `role`), or `instances` keeps the primary and the new table the supporting ones.
- `tsk actor register` prints the uid. `tsk actor join <uid> [--instance <key>] [--primary]`
  is the only new command. `tsk actor name` prints the primary and lists the supporting.
- `tsk task set <mission> <task> status in-progress` records the claim (uid and instance).
  `tsk task list` shows the actor's current name.
- `tsk thread list` and `tsk thread resume` show actors by name through the uid, so a
  renamed actor shows its new name everywhere.
- No column is renamed and no key changes, so two clones that upgrade on their own merge.
