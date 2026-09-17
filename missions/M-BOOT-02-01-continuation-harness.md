# Mission: Continuation harness

| Field | Value |
|---|---|
| ID | M-BOOT-02-01 |
| Territory | agentic research |
| Assignee | unassigned |
| Blocked by | none |

## Scope

Supervised interactive only. A human is present: running `/pause-thread` themselves,
and answering the `SessionStart` hook's prompt directly when it finds no binding or
finds one. Building any automated trigger for pausing, such as one driven later by the
goal verifier, is out of scope for this mission.

## Objective

Kind: attainable

- `/start-thread` exists as a skill backed by a deterministic script: it resolves the
  current binding (cloud session ID or worktree marker), mints a new thread only when
  none exists, and takes a mission argument the agent resolves and validates before the
  script runs.
- `/pause-thread` exists as a skill: it appends one continuation event to
  `threads/<slug>/continuation-state.jsonl` via `append-handover.sh`, with the commit
  hash on `tsk/bootstrap`, the commit hash on `main`, and a timestamp captured by the
  script, and the mission link, task ID and what's-next text supplied by the agent.
- `/resume-thread <thread-id>` exists as a skill: it loads the latest continuation
  event, presents it to the agent, and the agent replies with the fixed-shape summary
  and asks whether to continue.
- The `SessionStart` hook resolves the current binding automatically: it prompts
  `/resume-thread` when a binding is found, and prompts the human for a mission, then
  `/start-thread`, when none is found.
- A full cycle proves the mechanism end to end: start a thread, pause it, `/clear`,
  resume it, and the resumed session's summary correctly names the mission and the
  prior what's-next text.

## Purpose

Parent: M-BOOT-02, harness. The harness needs a mechanism for a session to pick up its
own prior thread, or another actor's, across a session boundary. This mission builds
it.

## Intelligence

- `docs/domain/session-continuation-design.md` (in the tsk repo, on `main`): the full
  design this mission implements. Read this in full before writing anything; it is the
  specification, not background reading.
- `docs/domain/ubiquitous-language.md`: Actor, Thread, Thread continuation.
- `docs/kb/session-creation-and-environments.md`: session and environment mechanics,
  particularly the two distinct session-ID environment variables on a cloud session.
- `docs/kb/agent-context-self-regulation-and-unattended-handoff.md`: operating
  contexts, and the `/clear` findings the design depends on.

## Decision authority

Jim decides any change to the design recorded in `docs/domain/session-continuation-design.md`
itself. Implementation choices the design doesn't cover, such as script language or file
layout inside `threads/<slug>/` beyond what it specifies, are this mission's own.

## Constraints

- Hooks must be bash. Cloud virtual machines are Linux.
- Where the design doc marks a point deferred, leave it deferred. Do not resolve an
  open question on this mission's own initiative.
- Operating context: supervised interactive only. Do not build an automated trigger for
  `/pause-thread`.

## Out of scope

- The thread state format proper (M-BOOT-02, T-04): which tasks are done, which is in
  progress. A continuation event's task ID and what's-next fields are enough here.
- An automated `/pause-thread` trigger.
- The Plan's relationship to a thread.
- A worktree registry or index beyond the marker file and the continuation log.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Thread ID minting | A script mints an 8-character lowercase base36 slug, checked against existing `threads/` entries for collision | none | DONE |
| T-02 | `threads/<slug>/` scaffolding | Script creates the directory, an `index.md` linking to the mission briefing, and an empty `continuation-state.jsonl` | T-01 | DONE |
| T-03 | Cloud session binding | Script reads `CLAUDE_CODE_REMOTE_SESSION_ID` and reads and writes `threads/lookup-by-cloud-session.json`, entries `{thread_id, registered_at}` | T-02 | DONE |
| T-04 | Worktree binding | Script resolves `$(git rev-parse --git-dir)` and reads and writes the `tsk-thread-id` marker file there | T-02 | DONE |
| T-05 | `/start-thread` skill | Wraps T-01 to T-04: resolves the current binding, mints only if none exists, agent resolves and validates the mission argument first | T-03, T-04 | DONE |
| T-06 | `append-handover.sh` and `/pause-thread` skill | Script appends a continuation event, capturing both commit hashes and a timestamp; skill supplies the mission link, task ID and what's-next text | T-02 | DONE |
| T-07 | `/resume-thread` skill | Loads the latest continuation event for a given thread ID, presents it, agent replies with the fixed-shape summary and asks whether to continue; take-over is additive, with a printed warning if the thread is already bound elsewhere | T-06 | DONE |
| T-08 | Extend the `SessionStart` hook | Resolves the binding (T-03, T-04); prompts `/resume-thread` on a hit; prompts the human for a mission then `/start-thread` on a miss | T-05, T-07 | DONE |
| T-09 | Prove the full cycle | Start a thread, pause it, `/clear`, resume it; the resumed summary correctly names the mission and the prior what's-next text | T-08 | DONE |
| T-10 | Clean up the proof cycle's test thread | T-09 exercises the real scripts against the real `tsk/bootstrap` data (there is no mock to run them against). Its test thread, and any binding entries it wrote, are removed and pushed once T-09 is confirmed, so no test debris is left in `threads/` | T-09 | DONE |
| T-11 | Guard the fetch path against an unpushed commit | `fetch-bootstrap-ref.sh` stops rather than resetting when the worktree HEAD is not an ancestor of origin's tip, so a commit made without a push is not orphaned | none | DONE |

**Essential task**: T-08. Without the hook wired up, nothing invokes the mechanism
automatically, and the objective is not met by the scripts existing alone.

**Implied task added on take-up, 2026-09-16**: T-10. T-09's proof has nothing else to run
the scripts against but the live `tsk/bootstrap` branch, so the briefing's plan was
missing the step that keeps that proof from leaving permanent test data behind.

**Task added after completion, 2026-09-17**: T-11. A worker restart can end a turn
between the commit and the push in `push-bootstrap-ref.sh`. That leaves the worktree
clean, so the dirty-tree guard in `fetch-bootstrap-ref.sh` does not catch it, and the
next `git reset --hard` orphans the commit. The `SessionStart` hook calls that fetch at
every session start, so the loss happens automatically, before any agent is in a
position to notice the pending commit.

`push-bootstrap-ref.sh` was made idempotent on `main` at `e3ec479`: it now commits only
when something is staged, and pushes a commit an earlier run left behind. That recovers
the state whenever the push script runs. It does not cover the fetch path, which is the
one that runs unattended.

Resolved 2026-09-17: the fetch leaves the worktree as it is and reports the pending
commit through the hook's `additionalContext`. It still prints the path and exits 0, so
`just fetch-refs` and the thread scripts are unaffected.

The message tells the agent to read what the commit changes before deciding, and says
the commit may be its own work from a turn it holds no record of. A worker restart can
end a turn between a commit and its push, so the agent that finds the commit is often
the one that made it. Left unsaid, the natural reading is that another actor made it,
which invites discarding real work.

Landed on `main` at `57b5320`.

**Note on the commit-on-`tsk/bootstrap` field (T-06).** The design names this field "the
commit the push script left the branch at," which read literally is self-referential: the
event line recording that commit is itself part of the commit being recorded, and a
commit hash cannot describe a tree that contains itself. Taken as an implementation
choice within the design rather than a change to it: `append-handover.sh` captures
`tsk/bootstrap`'s HEAD in `$TSK_BOOTSTRAP_WT` immediately before appending the event,
i.e. the commit the branch was already at when the pause began, matching how `commit on
main` is captured (also read before any push). Flagging this here rather than treating it
as blocking, since the alternative reading is not implementable as stated.

## First behaviour

Take ownership of the plan above, adding implied tasks, before any code. Read
`docs/domain/session-continuation-design.md` in full first; it is the specification this
mission builds, not background reading.

## Execution constraints

Files: a new `.claude/skills/` entry per command, a new `threads/` top-level directory
on `tsk/bootstrap`, `.claude/hooks/session-start.sh`, a script location for
`append-handover.sh` and the binding scripts consistent with the repo's existing
`ops/` layout, and `missions/M-BOOT-02-01/` for this mission's own report.

## Report on completion

Outcome: done, failed, or blocked, with attempt count. Your account: what you did, what
this briefing failed to give you, what you found wrong in it. Write it to
`missions/M-BOOT-02-01/M-BOOT-02-01-continuation-harness-report.md`.
