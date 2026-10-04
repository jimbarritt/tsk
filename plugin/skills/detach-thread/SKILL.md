---
name: detach-thread
description: Remove this session or code worktree's binding from its current tsk thread, without deleting the thread. Invoke explicitly as /tsk:detach-thread.
---

Backs the `/tsk:detach-thread` command. This skill is the agent-facing wrapper around
`tsk thread detach`, which does the deterministic work.

Detaching removes only this session's or code worktree's own binding. The thread
itself, its continuation state, and any other actor's binding to it are
untouched. To delete the thread as well, use the `tsk:stop-thread` skill instead.

## Steps

1. **Run the command.**

   ```bash
   tsk thread detach
   ```

2. **Handle the result.**
   - Exit 0, output `detached:<thread-id>`: tell me the thread this session or
     code worktree was detached from.
   - Exit 1: no binding was found, so there is nothing to detach. Report this
     plainly, not as an error to fix.
