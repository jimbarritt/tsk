---
name: switch-thread
description: Detach from the current tsk thread, optionally stop (delete) it, then resume a different thread — named explicitly, or chosen from a list of existing threads. Invoke as /switch-thread [<thread-id>]. Design reference — docs/domain/session-continuation-design.md.
---

Backs the `/switch-thread` command from `docs/domain/session-continuation-design.md`
on `main`. Read that document if you have not already. This skill composes the
`detach-thread`, `stop-thread`, and `resume-thread` skills rather than
duplicating their logic — run each of those as described below, do not inline
their scripts' work here.

## Steps

1. **Resolve the current binding**, if any:

   ```bash
   ops/local/thread-resolve-binding.sh
   ```

   Exit 1 (no output) means nothing is currently bound — skip to step 4; there is
   nothing to detach or offer to stop.

2. **Detach.** Run the `detach-thread` skill's steps
   (`ops/local/thread-detach.sh`). Keep the thread ID it printed — you need it
   for the next two steps.

3. **Ask whether to stop the thread just left.** Use `AskUserQuestion`: offer to
   stop (delete) it, or leave it as is for someone to resume later. If the
   answer is to stop it, run the `stop-thread` skill's steps, passing that
   thread ID explicitly:

   ```bash
   ops/local/thread-stop.sh "<old-thread-id>"
   ```

   It is already detached, so this only deletes it.

4. **Resolve the target thread.**
   - Given an explicit thread ID as this skill's argument: use it directly, and
     skip the rest of this step.
   - Given anything else (a mission name, natural language) or nothing: run
     `ops/local/thread-list.sh` and, for each candidate, read the mission title
     from its briefing (`mission_link`) the same way `resume-thread` does.
     Present the threads as `AskUserQuestion` options — thread ID, mission
     title, and `latest_whats_next` if present — with free text open for a
     thread ID not listed. If the argument named a mission, use it only to pick
     your suggested option, not to skip asking.
   - If no threads exist at all, say so and stop. There is nothing to switch
     to — offer `start-thread` instead.

5. **Resume.** Run the `resume-thread` skill's steps for the chosen thread ID.
