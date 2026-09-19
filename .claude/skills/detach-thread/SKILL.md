---
name: detach-thread
description: Remove this session or worktree's binding from its current tsk thread, without deleting the thread. Invoke explicitly as /detach-thread. Design reference — docs/domain/session-continuation-design.md.
---

Backs the `/detach-thread` command from `docs/domain/session-continuation-design.md`
on `main`. Read that document if you have not already; this skill is the thin,
agent-facing wrapper around `ops/local/thread-detach.sh`, which does the
deterministic work.

Detaching removes only this session's or worktree's own binding. The thread
itself, its continuation state, and any other actor's binding to it are
untouched. To delete the thread as well, use the `stop-thread` skill instead.

## Steps

1. **Run the script.**

   ```bash
   ops/local/thread-detach.sh
   ```

2. **Handle the result.**
   - Exit 0, output `detached:<thread-id>`: tell me the thread this session or
     worktree was detached from.
   - Exit 1: no binding was found — nothing to detach. Report this plainly, not
     as an error to fix.
