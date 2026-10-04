---
name: switch-thread
description: Detach from the current tsk thread, optionally stop (delete) it, then resume a different thread, named explicitly or chosen from a list of existing threads. Invoke as /tsk:switch-thread [<thread-id>].
---

Backs the `/tsk:switch-thread` command. This skill composes the `tsk:detach-thread`,
`tsk:stop-thread`, and `tsk:resume-thread` skills rather than duplicating their logic. Run
each of those as described below, and do not inline their work here.

## Steps

1. **Resolve the current binding**, if any:

   ```bash
   tsk thread binding
   ```

   Exit 1 (no output) means nothing is currently bound. Skip to step 4, because
   there is nothing to detach or offer to stop.

2. **Detach.** Run the `tsk:detach-thread` skill's steps (`tsk thread detach`). Keep the
   thread ID it printed, because the next two steps need it.

3. **Ask whether to stop the thread just left.** Use `AskUserQuestion`: offer to
   stop (delete) it, or leave it as is for someone to resume later. If the
   answer is to stop it, run the `tsk:stop-thread` skill's steps, passing that
   thread ID explicitly:

   ```bash
   tsk thread stop "<old-thread-id>"
   ```

   It is already detached, so this only deletes it.

4. **Resolve the target thread.**
   - Given an explicit thread ID as this skill's argument: use it directly, and
     skip the rest of this step.
   - Given anything else (a mission name, natural language) or nothing: run
     `tsk thread list` and, for each candidate, read the mission title from its
     briefing (`mission_link`) the same way `tsk:resume-thread` does. Present the
     threads as `AskUserQuestion` options with thread ID, mission title, and
     `latest_whats_next` if present, with free text open for a thread ID not
     listed. If the argument named a mission, use it only to pick your suggested
     option, not to skip asking.
   - If no threads exist at all, say so and stop. There is nothing to switch
     to. Offer `tsk:start-thread` instead.

5. **Resume.** Run the `tsk:resume-thread` skill's steps for the chosen thread ID.
