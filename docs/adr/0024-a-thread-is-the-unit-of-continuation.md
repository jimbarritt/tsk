# 24. A thread is the unit of continuation; there is no current mission

Date: 2026-10-23

## Status

Accepted. Decided by Jim on 2026-10-15 and 2026-10-16 (M-TSK-SCHEMA, the schema
decisions and T-10). Extracted from the decision records in M-TIDY T-11. ADR 0021 (state
is typed records) stands.

## Context

Work stops and starts. A session runs out of context, a person clears it, a container is
reclaimed. Something has to record where the work stopped, for the next session. One
option was a "current mission" mark in the ledger. That mark is shared state: two
sessions on different missions would overwrite each other's mark, and it says nothing
about where a piece of work stopped.

## Decision

1. **A thread is the unit of continuation.** A session or a code worktree binds to a
   thread. `tsk thread pause` appends a continuation entry: the mission, the task in
   progress and a short account of where things stand. `tsk thread resume` binds a
   session to the thread and prints its latest entry.
2. **A mission owns the work; a thread works it.** A mission owns tasks, decisions and
   questions. A thread names a mission by ID. A pause entry names a mission and a task by
   ID, and `tsk` refuses an ID it cannot find.
3. **A thread can exist without a mission.** `tsk thread start` with no mission ID mints
   a thread with no mission and no tasks. `tsk thread set mission` attaches or changes the
   mission later. Work that has no mission yet still has a place to continue from.
4. **There is no current mission.** The ledger holds no mark for what to work on next.
   Each thread knows its own mission. Choosing the next piece of work is a person's or an
   actor's decision, not ledger state.

## Consequences

- Several sessions can work different missions, or the same mission, from one ledger
  without a shared mark to contend for.
- A new session asks which thread to bind to, or starts one. The `SessionStart` hook
  prints that prompt.
- A thread can move between missions over its life, so one line of work that crosses
  missions keeps one continuation history.
