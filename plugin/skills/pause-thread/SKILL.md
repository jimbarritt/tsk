---
name: pause-thread
description: Record a continuation state entry for the current thread before ending the session, so a later /tsk:resume-thread (by this actor or another) can pick the work back up. Invoke explicitly as /tsk:pause-thread, always run by the human themselves before /clear.
---

Backs the `/tsk:pause-thread` command. This skill is the agent-facing wrapper around
`tsk thread pause`.

This command is invoked manually, by a human, before `/clear`. Do not build or
suggest an automated trigger for it.

## Steps

1. **Compose the continuation state entry's two judgement fields**, from what you
   already know about the thread (usually already on hand from thread state,
   not freshly decided). The mission comes from the thread.
   - `<task-id>`: the task in progress when the thread paused, for example `T-05`. It
     can be empty. When the thread's mission has a record, it must be a task of that
     mission. When the thread has no mission (the session start context says so),
     pass an empty task ID: a thread with no mission has no tasks, and the command
     refuses a task ID. To give the thread a mission first, run
     `tsk thread set mission <mission-id>`. List the mission's tasks with
     `tsk task list <mission-id>`.
   - `<whats-next>`: a short, concrete account of where things stand, specific
     enough that a different actor, reading only this line, knows what to do
     next.

2. **Run one command.**

   ```bash
   tsk thread pause --bound "<task-id>" "<whats-next>"
   ```

   `--bound` pauses the thread this session or code worktree is bound to, read from the
   ledger store with no network call, so no lookup is needed first. If the command exits 1
   and reports that nothing is bound, there is no thread to pause. Say so and stop. Do not start
   one implicitly. When the thread ID is already known, `tsk thread pause "<thread-id>"
   "<task-id>" "<whats-next>"` does the same.

   The command captures the ledger commit, the code commit and branch, and the
   timestamp itself. Do not pass those, and do not compute them yourself. It exits 1
   when `HEAD` is not on any branch on origin: push the code first, then run it again.

   The pause is a publish point. It pushes the handover entry together with every record
   you wrote since the last push. Records are written freely and published at thread
   start, thread pause, the end of the session and when asked, so this is the call that
   carries a session's progress to origin.

3. **Read the output.** After `paused:<thread-id>` the command prints three lines that
   check the hand-off, so no separate check is needed:

   ```
   paused:<thread-id>
   code: clean                  or  code: dirty (N files)
   code pushed: yes             or  code pushed: no
   ledger: pushed               or  ledger: N changes not pushed
   ```

   A line that reads `unknown (...)` means the check could not run. Treat it as not
   passed.

4. **Confirm only when all three pass.** If `code: clean`, `code pushed: yes` and
   `ledger: pushed` all print, tell me the thread is paused, name the thread ID and what
   is next, and say it is safe to `/clear`.

   If any line differs, do not say it is safe. Commit and push what belongs pushed (for
   a dirty code worktree or `code pushed: no`), or run `tsk ledger push "<describe the
   update>"` (for `ledger: N changes not pushed`), then run the pause again, or report
   the mismatch to me plainly. A `paused:<thread-id>` line alone only confirms that the
   handover entry was pushed. It does not cover other work from the session. On any
   error, report it and do not tell me it is safe to clear.
