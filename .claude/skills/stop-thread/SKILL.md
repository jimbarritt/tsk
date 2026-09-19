---
name: stop-thread
description: Detach this session or worktree from its current tsk thread, then delete the thread entirely — its directory, continuation state, and every binding to it. Invoke explicitly as /stop-thread. Destructive. Design reference — docs/domain/session-continuation-design.md.
---

Backs the `/stop-thread` command from `docs/domain/session-continuation-design.md`
on `main`. Read that document if you have not already; this skill is the thin,
agent-facing wrapper around `ops/local/thread-stop.sh`, which does the
deterministic work.

This is destructive: once pushed, the thread's directory and continuation state
are gone from `tsk/bootstrap`'s working state (recoverable from the branch's git
history, not from anything the ledger itself presents). Only run this for a
thread you mean to end, not merely pause — `pause-thread` is for that.

## Steps

1. **Resolve the target.** With no argument, the script targets whatever thread
   this session or worktree is currently bound to. Given an explicit thread ID
   (for example, when invoked from `switch-thread`), pass it directly instead.

2. **Run the script.**

   ```bash
   ops/local/thread-stop.sh ["<thread-id>"]
   ```

   It detaches the current binding first, if it points at the target thread,
   deletes `threads/<id>/`, and purges every cloud-session binding still
   pointing at it. A worktree marker in some *other* worktree naming this
   thread cannot be reached from here — it goes stale silently, the same
   limitation the binding design already accepts for worktree markers
   generally.

3. **Handle the result.**
   - Exit 0, output `stopped:<thread-id>`: tell me the thread was stopped and
     deleted.
   - Exit 1: no thread to target (not bound, and no ID given), or the named
     thread does not exist. Report plainly.
