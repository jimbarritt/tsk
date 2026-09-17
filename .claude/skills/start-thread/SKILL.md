---
name: start-thread
description: Bind this session or worktree to a tsk thread for a named mission, minting a new thread if none exists yet. Invoke explicitly as /start-thread <mission>, or when a natural-language request names a mission to begin work on. Design reference — docs/domain/session-continuation-design.md.
---

Backs the `/start-thread` command from `docs/domain/session-continuation-design.md`
on `main`. Read that document if you have not already; this skill is the thin,
agent-facing wrapper around `ops/local/thread-start.sh`, which does the
deterministic work.

## Steps

1. **Resolve the mission argument.** You were given a mission reference, by name,
   ID, or natural language. Resolve it to:
   - `<mission-id>`: the mission's stable ID, e.g. `M-BOOT-02-01`.
   - `<briefing-path>`: the mission briefing's path, relative to
     `$TSK_BOOTSTRAP_WT` (not the absolute path) — e.g.
     `missions/M-BOOT-02-01-continuation-harness.md`. Look it up in
     `$TSK_BOOTSTRAP_WT/index.md`'s mission tree if you don't already know it.

   If you cannot resolve the reference to an actual mission, stop and ask rather
   than guessing — do not invent an ID or a path.

2. **Run the script.**

   ```bash
   ops/local/thread-start.sh "<mission-id>" "<briefing-path>"
   ```

   The script independently validates that the briefing path exists in the
   bootstrap worktree — it does not trust your resolution alone.

3. **Handle the result.**
   - Exit 0, output `started:<thread-id>`: a new thread was minted and bound.
     Tell me the new thread ID and confirm the mission it is working.
   - Exit 2, output `resume-required:<thread-id>`: this session or worktree is
     already bound to a thread. This is not a start, it is a resume. Invoke the
     `resume-thread` skill with that thread ID instead of proceeding here.
   - Exit 1: the mission argument failed validation, or another error occurred.
     Report the script's stderr and stop — do not retry with a guessed path.
