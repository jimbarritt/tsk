---
name: pause-thread
description: Record a continuation state entry for the current thread before ending the session, so a later /resume-thread (by this actor or another) can pick the work back up. Invoke explicitly as /pause-thread, always run by the human themselves before /clear in this supervised-interactive-only mission. Design reference — docs/domain/session-continuation-design.md.
---

Backs the `/pause-thread` command from `docs/domain/session-continuation-design.md`
on `main`. Read that document if you have not already.

This command is invoked manually, by a human, before `/clear`. Do not build or
suggest an automated trigger for it — that is explicitly out of scope for this
mission (see `docs/domain/session-continuation-design.md`'s "Deferred" section
and mission M-BOOT-02-01's Scope).

## Steps

1. **Resolve the current thread ID.** Run:

   ```bash
   ops/local/thread-resolve-binding.sh
   ```

   This prints `cloud:<thread-id>` or `worktree:<thread-id>`. If it exits 1
   (no binding), there is no thread to pause — say so and stop; do not start
   one implicitly.

2. **Compose the continuation state entry's three judgement fields**, from what you
   already know about the thread (usually already on hand from thread state,
   not freshly decided):
   - `<mission-briefing-link>`: the mission briefing this thread is working.
   - `<task-id>`: the task in progress when the thread paused.
   - `<whats-next>`: a short, concrete account of where things stand — specific
     enough that a different actor, reading only this line, knows what to do
     next.

3. **Run the script.**

   ```bash
   ops/local/thread-append-handover.sh "<thread-id>" "<mission-briefing-link>" "<task-id>" "<whats-next>"
   ```

   It captures the commit on `tsk/bootstrap`, the commit on `main`, and the
   timestamp itself — do not pass those, and do not compute them yourself.

4. **Verify clean and pushed, on both branches, before saying anything is safe.**
   A successful `paused:<thread-id>` above only confirms the handover entry itself
   was pushed. It says nothing about other work from the session, on `main` or
   still uncommitted in the bootstrap worktree. Check both explicitly:

   ```bash
   # main: nothing uncommitted, nothing unpushed
   git status --porcelain
   git rev-parse HEAD
   git ls-remote origin main

   # tsk/bootstrap: nothing uncommitted in the worktree, worktree HEAD matches
   # origin. Spell the ref out in full, per CLAUDE.md, rather than a bare
   # `git fetch origin tsk/bootstrap` — the same reasoning applies to this
   # check as to any other read of the branch.
   git -C "$WT" status --porcelain
   git -C "$WT" rev-parse HEAD
   git fetch origin refs/heads/tsk/bootstrap
   git rev-parse FETCH_HEAD
   ```

   Pass only if `git status --porcelain` is empty on both, and `HEAD` on each
   matches what its own remote check just reported. If anything is uncommitted or
   the local HEAD differs from origin's, stop: commit and push what belongs
   pushed, or report the mismatch plainly. Do not tell me it is safe to `/clear`
   on an unverified guess.

5. **Confirm.** Only once step 4 passes, tell me the thread is paused, name the
   commit each branch is now at, and say it is safe to `/clear`. On any error at
   any step, report it and do not tell me it is safe to clear.
