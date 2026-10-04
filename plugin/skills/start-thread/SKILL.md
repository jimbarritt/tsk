---
name: start-thread
description: Bind this session or code worktree to a tsk thread for a named mission, minting a new thread if none exists yet. Invoke explicitly as /tsk:start-thread <mission>, or when a natural-language request names a mission to begin work on.
---

Backs the `/tsk:start-thread` command. This skill is the agent-facing wrapper around
`tsk thread start`, which does the deterministic work.

## Steps

1. **Resolve the mission argument.** You were given a mission reference, by name,
   ID, or natural language. Resolve it to:
   - `<mission-id>`: the mission's stable ID, e.g. `M-BOOT-02-01`.
   - `<briefing-path>`: the mission briefing's path, relative to the ledger root
     (not the absolute path), e.g. `missions/operational/M-BOOT-04-official-ledger.md`.
     Look it up in `$TSK_LEDGER_WT/index.md`'s mission tree if you don't already know
     it. If `$TSK_LEDGER_WT` is not set, use `tsk ledger path`.

   If you cannot resolve the reference to an actual mission, stop and ask rather
   than guessing. Do not invent an ID or a path.

2. **Run the command.**

   ```bash
   tsk thread start "<mission-id>" "<briefing-path>"
   ```

   The command validates that the briefing path exists in the ledger worktree. It
   does not trust your resolution alone.

3. **Handle the result.**
   - Exit 0, output `started:<thread-id>`: a new thread was minted and bound.
     Tell me the new thread ID and confirm the mission it is working.
   - Exit 2, output `resume-required:<thread-id>`: this session or code worktree is
     already bound to a thread. This is a resume, not a start. Invoke the
     `tsk:resume-thread` skill with that thread ID instead of proceeding here.
   - Exit 1: the mission argument failed validation, or another error occurred.
     Report the command's stderr and stop. Do not retry with a guessed path.
