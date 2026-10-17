---
name: resume-thread
description: Load a tsk thread's latest continuation state entry and pick the work back up, as the thread's original actor after a session boundary, or as a different actor taking over. Invoke as /tsk:resume-thread <thread-id>, or automatically when the SessionStart hook finds an existing binding.
---

Backs the `/tsk:resume-thread <thread-id>` command. This skill is the agent-facing wrapper
around `tsk thread resume`.

Take-over is additive, not exclusive: binding to a thread that already has
another binding elsewhere is allowed and expected. That is how a different actor
picks up someone else's thread. The command warns when this happens. It does not
refuse.

## Steps

1. **Run the command.**

   ```bash
   tsk thread resume "<thread-id>"
   ```

   This binds the current session or code worktree to the thread (additive), and
   prints one JSON object: `{"thread_id", "latest", "warning"}`, where `latest`
   is the most recent continuation state entry (or `{}` if the thread has never been
   paused) and `warning` is non-empty if the thread was already associated
   with a different actor.

2. **If `warning` is non-empty**, surface it to me plainly before going further.
   Do not silently take over a thread another actor is or was working.

3. **Present the fixed-shape summary**, reading `latest.mission_id` (or
   `latest.mission_link` in an older entry), `latest.task_id` and `latest.whats_next`
   (if `latest` is `{}`, say so instead of inventing a summary):

   ```
   thread id: <thread-id>
   we are working on mission <mission title>
   this is where we are at: <summary of the handover note>
   ```

   Resolve `<mission title>` from `latest.mission_id`: `tsk mission list <mission-id>`
   prints the name. For an older entry that has only `latest.mission_link`, read the
   briefing with `tsk ledger doc get <mission_link>` if you need the title. Do not just
   repeat the raw ID or link.

   A thread can have no mission: `latest.mission_id` is null, or `latest` is `{}`. Then
   write `we are working on no mission (this thread has no mission, so it has no
   tasks)` for the second line. Offer `tsk thread set mission <mission-id>` if I want
   to attach one.

4. **Ask whether to continue** with this thread, or do something else. Do not
   assume continuation.
