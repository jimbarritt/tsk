# Mission Briefing Template

A briefing is a rendering of a mission for a specific actor. The same mission renders
differently for a human and for a cloud agent. This template is the rendering format.

The template works at any level. A top level mission and a leaf mission use the same
fields. There is one task list. A task becomes a mission only when it is delegated to a
different actor from the one holding this mission; those tasks hold a Delegated to
value and get their own briefing rendered from them. All other tasks are the holder's
own work and stay plain.

Fields marked **tsk** come from the tsk domain. Fields marked **harness** are supplied
by ksobr and are not part of the mission itself. A briefing rendered for a human may
omit most harness fields.

Fill every required field. Omit an optional field rather than writing "none".

The metadata block is a table. One row per field. Do not add fields beyond those listed;
anything else worth recording belongs in a section below.

---

## Template

```markdown
# Mission: {title}

| Field | Value |
|---|---|
| ID | {stable identifier, never reused} |
| Territory | {territory name} |
| Assignee | {actor, or unassigned} |
| Blocked by | {mission IDs, or none} |

## Objective

{One checkable statement per line. A line is either a state that is true or false, or a
measure that moves over time. Both are objectives; neither needs declaring as a type.}

## Purpose

{The parent mission's objective, one line. Link to the parent mission. Omit for a top
level mission.}

## Intelligence

- {link or path, with one line on what it contains}

## Decision authority

{What the actor decides. What the actor does not decide, and which mission does. Omit
if the actor decides everything within the objective.}

## Constraints

{Constraints on the objective. Each one would still apply if a human received this
mission.}

## Out of scope

{Implied tasks the actor must not pursue. One per line.}

## Plan

This section is the mission's plan: the proposed sequence of tasks for meeting the
objective. It is provisional, and it is rewritten as execution proceeds. The objective
above does not move with it.

The plan belongs to the actor executing the mission. What the briefing carries here are
the specified tasks, including any essential one. The actor decides the plan from them,
adding implied tasks, reordering, and rewriting. The briefing seeds the plan, it does
not fix it.

One list. Every task has an ID, an objective and a status.

| ID | Task | Objective, in short | Delegated to | Blocked by | Status |
|---|---|---|---|---|---|
| {id} | {task} | {checkable state} | {actor, or none} | {ids, or none} | {status} |

**Essential task**: {which ID, and why the mission fails without it.}

Tasks the actor adds during execution are implied tasks. They join the same list.
Tasks in this briefing are specified tasks. The distinction is a flag, not a section.

A plan that outgrows this table is a signal to delegate, not to start a second document.
Break the work out as sub-missions, each with its own briefing and its own plan.

## First behaviour

{Harness. Take ownership of the plan above before any other action. Add the implied
tasks, reorder as you see fit, and write it back as your own.}

## Execution constraints

{Harness. Permitted files. Attempt limit. Budget. Omit for a human.}

## Report on completion

{Harness. Outcome: done, failed, or blocked, with attempt count. Your account: what you
did, what this briefing failed to give you, what you found wrong in it, including in
your own work. Feedback only. Do not write an open question, a request for a decision,
or anything that waits on a reply.}
```

---

## Field notes

**ID.** Stable across renames. Numbers restart nowhere. Use the ID in every reference to
the mission.

**Category.** A mission is administrative or operational, set by which subdirectory of
the ledger's `missions/` it sits in rather than by a field in the briefing.

**Reports.** A mission reports once or repeatedly over time, whichever suits it. There
is no rule fixing which. A report also takes addenda: a later finding is appended to the
existing report rather than replacing it or starting a new document.

A report carries feedback and never an open question. State what you found, including an
inconsistency in your inputs and a judgement you had to make to get past it. Do not ask
the reader to confirm, decide, or come back to you. Where a finding needs a decision or
further work, that belongs where work is tracked, as a mission or a task; the report
records the finding. Reading a report tells you what happened, and never leaves you
owing it a reply.

**Task objectives.** Every task has one, by the model. In the table, write it short. A
delegated task's full objective lives in its own briefing.

**Objective lines.** Write states, not activities. "The push arrives and `git ls-remote`
returns the commit" is a state. "Implement push" is an activity. Where possible, make
the state executable: a test that passes, a command that returns zero, a file that
exists.

**Purpose.** Derived from the parent. Do not restate the parent's whole briefing. The
actor needs the parent objective to adapt when the plan meets reality, so include it
even when it seems obvious.

**Intelligence.** Links, not content. If the actor needs the content, the content
belongs in `docs/` and the briefing points at it.

**Decision authority.** The value is sometimes "not yours, see mission {id}". Write
that explicitly. An actor that does not know a decision is deferred will make it.

**Constraints versus execution constraints.** Test: would a human doing this mission
need it? If yes, it is a constraint on the objective and belongs under Constraints. If
it only exists because a harness enforces it, it belongs under Execution constraints.

**Out of scope.** This is the most useful section to write. It is the list of implied
tasks the actor would otherwise infer.

**Delegation.** A task becomes a mission when it is handed to a different actor from the
one holding this mission. Only then does it need a briefing. Assignment alone is not the
test: every task on an agent's own task list has that agent as its actor, and none of
them is a mission. Most tasks never become missions.

**One list only.** Do not keep a second list of the delegated tasks. Any summary table
elsewhere is generated from this one. Two authored copies of the same fact drift.

**Harness fields.** The ksobr plugin supplies these when rendering for an agent. Do not
write them into the mission itself.

**Report location.** Written to `missions/{id}/{id}-{slug}-report.md`: the briefing's
own filename with a `report` suffix, inside a subdirectory named for the mission's ID.
The subdirectory exists only once the mission starts executing; the briefing itself
stays a flat file until then. The full filename, not a generic `report.md`, is
deliberate: the file may be found outside its directory, and its name should still say
which mission it belongs to.

---

## Worked example, leaf mission

```markdown
# Mission: Push a single actor's event log to the ledger

| Field | Value |
|---|---|
| ID | M-008-01 |
| Territory | agentic research |
| Assignee | cloud agent, Sonnet 5 |
| Blocked by | M-006 |

## Objective

- Given a repo with a local tsk event log and a git remote, running `tsk sync push`
  results in `git ls-remote origin refs/tsk/data` returning a commit.
- That commit's tree contains the actor's `events-{hash}.ndjson`, byte identical to
  the local file.
- Running `tsk sync push` again with no new events is a no-op.
- All three are proven by tests against a local bare repo as remote.

## Purpose

tsk state persists outside the local machine. Parent: M-008.

## Intelligence

- docs/decisions/git-ref-sync.md: why git refs, not Dolt
- docs/reference/dolt-gitblobstore.md: the plumbing sequence and force-with-lease
- src/events/: the existing event log write path

## Decision authority

Shell out to the git binary. The Rust git library choice is not yours. See M-008-04.

## Constraints

- No change to the event file format.
- Single actor only.

## Out of scope

- Pull.
- The manifest.
- Multi-actor.
- Conflict handling and the compare and swap retry loop.

## Plan

### Specified

1. Build blob, tree and commit for the actor's log file.
2. Point `refs/tsk/data` at the commit. Push with `--force-with-lease`.
3. Test suite using a local bare repo as origin.

### Essential

Task 2. The mission fails if the push does not land.

## First behaviour

Take ownership of the plan above, adding implied tasks, before any code.

## Execution constraints

- Files: `src/sync/` (new), `tests/`.
- Attempt limit: 3.

## Report on completion

Outcome, attempt count, and your account.
```
