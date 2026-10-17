---
name: start-thread
description: Bind this session or code worktree to a tsk thread, for a named mission or with no mission, minting a new thread if none exists yet. Invoke explicitly as /tsk:start-thread <mission>, or when a natural-language request names a mission to begin work on.
---

Backs the `/tsk:start-thread` command. This skill is the agent-facing wrapper around
`tsk thread start`, which does the deterministic work.

## Steps

1. **Resolve the mission argument.** You were given a mission reference, by name,
   ID, or natural language. Resolve it to the mission's stable ID, e.g.
   `M-TSK-SCHEMA`. List the missions with `tsk mission list`, or, for a ledger with no
   mission records yet, read the mission tree that `tsk ledger doc get index.md` prints.

   If you cannot resolve the reference to an actual mission, stop and ask rather
   than guessing. Do not invent an ID.

   If I want continuity but no mission, start a thread with no mission: skip the
   mission ID in step 2. A thread with no mission has no tasks. Tasks belong to
   missions, so for ad hoc tasks use an administrative mission such as `M-ADMIN-02`
   instead. Attach a mission to a thread later with
   `tsk thread set mission <mission-id>`.

2. **Run the command.**

   ```bash
   tsk thread start "<mission-id>"
   ```

   The command checks that the mission exists, as a record or as a briefing
   document. It does not trust your resolution alone.

   For a thread with no mission, run it with no argument:

   ```bash
   tsk thread start
   ```

3. **Handle the result.**
   - Exit 0, output `started:<thread-id>`: a new thread was minted and bound.
     Tell me the new thread ID and confirm the mission it is working, or that it has
     no mission.
   - Exit 2, output `resume-required:<thread-id>`: this session or code worktree is
     already bound to a thread. This is a resume, not a start. Invoke the
     `tsk:resume-thread` skill with that thread ID instead of proceeding here.
   - Exit 1: the mission ID is unknown, or another error occurred. Report the
     command's stderr and stop. Do not retry with a guessed ID.
