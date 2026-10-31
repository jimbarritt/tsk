---
name: switch-mission
description: Change the current focus of the tsk thread this session is bound to, to another mission. Pauses the thread on its current focus, sets the new mission, and sets the session title. Invoke as /tsk:switch-mission <mission>, or when a request names a mission to work on and this session is bound to a thread.
---

Backs the `/tsk:switch-mission <mission>` command. A thread is a line of work with one
continuation history, and its mission is its current focus. Changing the focus keeps the
thread. A new thread is created only by `tsk:start-thread`, for a harness instance that has
no bound thread and no thread to join.

## Steps

1. **Check the binding first.**

   ```bash
   tsk thread binding
   ```

   Exit 1 with no output means this session is not bound. Do not start a thread here.
   Run the `tsk:start-thread` skill with the mission, which first looks for a thread to
   join, and stop.

   Exit 0 prints `cloud:<thread-id>` or `worktree:<thread-id>`. Keep the thread ID.

2. **Resolve the mission argument** to the mission's stable ID, as step 1 of
   `tsk:start-thread` does. List the missions with `tsk mission list`, and read one in full
   with `tsk mission list <id> --json`. If you cannot resolve the reference to an actual
   mission, stop and ask. Do not invent an ID.

   If the mission is already the thread's current mission, say so and stop.

3. **Pause on the current focus.** The pause records where the work on the old mission
   stands, so the next actor who reads the thread finds the hand-over:

   ```bash
   tsk thread pause --bound "<task-id>" "<whats-next>"
   ```

   `<task-id>` is the task in progress on the old mission, or empty. `<whats-next>` says
   where the old mission stands and that this thread now works the new mission. The pause
   is a publish point, so it also publishes every record written since the last push. If
   it exits 1 because `HEAD` is on no branch on origin, push the code first and run it
   again.

4. **Set the new mission.**

   ```bash
   tsk thread set mission "<mission-id>"
   ```

   The command prints `mission-set:<thread-id>:<mission-id>`. It writes locally. The next
   pause, the end of the session or `tsk ledger push` publishes the change.

5. **Set the session title.** Run `tsk thread title`. Exit 1 means there is no actor and
   no mission: skip this step. Exit 0 prints the title.
   - In a cloud session (`CLAUDE_CODE_REMOTE_SESSION_ID` is set), call `get_session` from
     the `claude-code-remote` MCP server with no `session_id` to read this session's ID.
     Then call `set_session_title` with that ID and the title.
   - In a terminal, no tool sets the title. Ask me to run `/rename <title>`.

6. **Report.** Tell me the thread ID, which is unchanged, the old mission and the new
   mission. List the open tasks of the new mission with `tsk task list "<mission-id>"`.

   If I take a task, set it to `in-progress` and publish it straight away, so that no
   other actor takes the same task:

   ```bash
   tsk task set "<mission-id>" "<task-id>" status in-progress
   tsk ledger push "Claim <mission-id> <task-id>"
   ```
