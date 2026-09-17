---
name: pause-thread
description: Record a continuation event for the current thread before ending the session, so a later /resume-thread (by this actor or another) can pick the work back up. Invoke explicitly as /pause-thread, always run by the human themselves before /clear in this supervised-interactive-only mission. Design reference — docs/domain/session-continuation-design.md.
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

2. **Compose the continuation event's three judgement fields**, from what you
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

4. **Confirm.** On `paused:<thread-id>`, tell me the thread is paused and it is
   safe to `/clear`. On any error, report it and do not tell me it is safe to
   clear.
