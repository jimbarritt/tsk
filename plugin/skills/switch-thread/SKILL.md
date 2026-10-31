---
name: switch-thread
description: Move this session from its current tsk thread to a different thread, a different line of work, named explicitly or chosen from a list. The thread left behind stays for its actor to resume. To change the mission of the same line of work, use /tsk:switch-mission. Invoke as /tsk:switch-thread [<thread-id>].
---

Backs the `/tsk:switch-thread` command. This skill composes the `tsk:detach-thread`,
`tsk:stop-thread`, and `tsk:resume-thread` skills rather than duplicating their logic. Run
each of those in the steps below, and do not inline their work here.

## Steps

1. **Resolve the current binding**, if any:

   ```bash
   tsk thread binding
   ```

   Exit 1 (no output) means nothing is currently bound. Skip to step 4, because
   there is nothing to detach or offer to stop.

2. **Detach.** Run the `tsk:detach-thread` skill's steps (`tsk thread detach`). Keep the
   thread ID it printed, because the next two steps need it.

3. **Ask whether the line of work on the thread just left is finished.** Use
   `AskUserQuestion`. The default is to keep the thread as it is, so that its actor or
   another actor can resume it later. Offer to stop (delete) it only when the line of
   work is finished. If the answer is to stop it, run the `tsk:stop-thread` skill's steps, passing that
   thread ID explicitly:

   ```bash
   tsk thread stop "<old-thread-id>"
   ```

   It is already detached, so this only deletes it.

4. **Resolve the target thread.**
   - Given an explicit thread ID as this skill's argument: use it directly, and
     skip the rest of this step.
   - Given anything else (a mission name, natural language) or nothing: run
     `tsk thread list` and `tsk mission list` once each, and read each candidate's mission
     title from the mission list by its `mission_id` (a null `mission_id` is a thread with
     no mission). Do not run a command for each candidate. Present the
     threads as `AskUserQuestion` options with thread ID, mission title, and
     `latest_whats_next` if present, with free text open for a thread ID not
     listed. If the argument named a mission, use it only to pick your suggested
     option, not to skip asking.
   - If no threads exist at all, say so and stop. There is nothing to switch
     to. Offer `tsk:start-thread` instead.

5. **Resume.** Run the `tsk:resume-thread` skill's steps for the chosen thread ID. Its
   title step sets the session title for the new thread's mission.

Detach and resume write locally and do not push. Stop pushes. The next thread start,
pause or stop publishes the binding change, as does the end of the session and
`tsk ledger push`. Run `tsk ledger push` after the switch if another actor must see the
new binding now.
