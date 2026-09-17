---
name: resume-thread
description: Load a tsk thread's latest continuation event and pick the work back up, as the thread's original actor after a session boundary, or as a different actor taking over. Invoke as /resume-thread <thread-id>, or automatically when the SessionStart hook finds an existing binding. Design reference — docs/domain/session-continuation-design.md.
---

Backs the `/resume-thread <thread-id>` command from
`docs/domain/session-continuation-design.md` on `main`. Read that document if
you have not already.

Take-over is additive, not exclusive: binding to a thread that already has
another binding elsewhere is allowed and expected — that is exactly how a
different actor picks up someone else's thread. The script warns when this
happens; it does not refuse.

## Steps

1. **Run the script.**

   ```bash
   ops/local/thread-resume.sh "<thread-id>"
   ```

   This binds the current session or worktree to the thread (additive), and
   prints one JSON object: `{"thread_id", "latest", "warning"}`, where `latest`
   is the most recent continuation event (or `{}` if the thread has never been
   paused) and `warning` is non-empty if the thread was already associated
   with a different actor.

2. **If `warning` is non-empty**, surface it to me plainly before going further
   — do not silently take over a thread another actor is or was working.

3. **Present the fixed-shape summary**, reading `latest.mission_link`,
   `latest.task_id` and `latest.whats_next` (if `latest` is `{}`, say so
   instead of inventing a summary):

   ```
   thread id: <thread-id>
   we are working on mission <mission title>
   this is where we are at: <summary of the handover note>
   ```

   Resolve `<mission title>` from `latest.mission_link` — read the briefing if
   you need the title, don't just repeat the raw link.

4. **Ask whether to continue** with this thread, or do something else. Do not
   assume continuation.
