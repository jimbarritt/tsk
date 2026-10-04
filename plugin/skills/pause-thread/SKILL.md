---
name: pause-thread
description: Record a continuation state entry for the current thread before ending the session, so a later /tsk:resume-thread (by this actor or another) can pick the work back up. Invoke explicitly as /tsk:pause-thread, always run by the human themselves before /clear.
---

Backs the `/tsk:pause-thread` command. This skill is the agent-facing wrapper around
`tsk thread pause`.

This command is invoked manually, by a human, before `/clear`. Do not build or
suggest an automated trigger for it.

## Steps

1. **Resolve the current thread ID.** Run:

   ```bash
   tsk thread binding
   ```

   This prints `cloud:<thread-id>` or `worktree:<thread-id>`. If it exits 1
   (no binding), there is no thread to pause. Say so and stop. Do not start
   one implicitly.

2. **Compose the continuation state entry's three judgement fields**, from what you
   already know about the thread (usually already on hand from thread state,
   not freshly decided):
   - `<mission-briefing-link>`: the mission briefing this thread is working, relative
     to the ledger root.
   - `<task-id>`: the task in progress when the thread paused.
   - `<whats-next>`: a short, concrete account of where things stand, specific
     enough that a different actor, reading only this line, knows what to do
     next.

3. **Run the command.**

   ```bash
   tsk thread pause "<thread-id>" "<mission-briefing-link>" "<task-id>" "<whats-next>"
   ```

   It captures the ledger commit, the code commit and branch, and the timestamp
   itself. Do not pass those, and do not compute them yourself. It exits 1 when
   `HEAD` is not on any branch on origin: push the code first, then run it again.

4. **Verify clean and pushed, in the code worktree and the ledger, before saying
   anything is safe.** A successful `paused:<thread-id>` above only confirms the
   handover entry itself was pushed. It says nothing about other work from the
   session, uncommitted in the code worktree or still pending in the ledger
   worktree. Check both:

   ```bash
   # code worktree: nothing uncommitted, HEAD on a branch origin holds
   git status --porcelain
   git branch -r --contains HEAD

   # ledger: commits any pending ledger worktree change and pushes any unpushed commit
   tsk ledger push "Flush ledger before clear"
   ```

   Pass only if `git status --porcelain` is empty, `git branch -r --contains HEAD`
   lists a branch, and `tsk ledger push` exits 0. If anything is uncommitted or
   unpushed, stop: commit and push what belongs pushed, or report the mismatch
   plainly. Do not tell me it is safe to `/clear` on an unverified guess.

5. **Confirm.** Only once step 4 passes, tell me the thread is paused, name the
   ledger commit and the code commit it recorded, and say it is safe to `/clear`.
   On any error at any step, report it and do not tell me it is safe to clear.
