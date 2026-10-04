---
name: stop-thread
description: Detach this session or code worktree from its current tsk thread, then delete the thread entirely, including its directory, continuation state, and every cloud session binding to it. Invoke explicitly as /tsk:stop-thread. Destructive.
---

Backs the `/tsk:stop-thread` command. This skill is the agent-facing wrapper around
`tsk thread stop`, which does the deterministic work.

This is destructive: once pushed, the thread's directory and continuation state
are gone from the ledger's working state (recoverable from the branch's git
history, not from anything the ledger itself presents). Only run this for a
thread you mean to end, not merely pause. `tsk:pause-thread` is for that.

## Steps

1. **Resolve the target.** With no argument, the command targets whatever thread
   this session or code worktree is currently bound to. Given an explicit thread ID
   (for example, when invoked from `tsk:switch-thread`), pass it directly instead.

2. **Run the command.**

   ```bash
   tsk thread stop ["<thread-id>"]
   ```

   It detaches the current binding first, if it points at the target thread,
   deletes `threads/<id>/`, and purges every cloud session binding still
   pointing at it. A binding marker in some *other* code worktree naming this
   thread cannot be reached from here and goes stale silently, the same
   limitation the binding design already accepts for code worktree markers
   generally.

3. **Handle the result.**
   - Exit 0, output `stopped:<thread-id>`: tell me the thread was stopped and
     deleted.
   - Exit 1: no thread to target (not bound, and no ID given), or the named
     thread does not exist. Report plainly.
