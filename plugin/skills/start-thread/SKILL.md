---
name: start-thread
description: Bind this session or code worktree to a tsk thread, for a named mission or with no mission, minting a new thread if none exists yet. Invoke explicitly as /tsk:start-thread <mission>, or when a natural-language request names a mission to begin work on.
---

Backs the `/tsk:start-thread` command. This skill is the agent-facing wrapper around
`tsk thread start`, which does the deterministic work.

## Steps

1. **Resolve the mission argument.** You were given a mission reference, by name,
   ID, or natural language. Resolve it to the mission's stable ID, e.g.
   `M-TSK-SCHEMA`. List the missions with `tsk mission list`, and read one in full with
   `tsk mission list <id> --json`.

   If you cannot resolve the reference to an actual mission, stop and ask rather
   than guessing. Do not invent an ID.

   If I want continuity but no mission, start a thread with no mission: skip the
   mission ID in step 2. A thread with no mission has no tasks. Tasks belong to
   missions, so for ad hoc tasks use an administrative mission such as `M-ADMIN-02`
   instead. Attach a mission to a thread later with
   `tsk thread set mission <mission-id>`.

2. **Run the command.**

   ```bash
   tsk thread start "<mission-id>"
   ```

   The command checks that the mission has a record. It does not trust your resolution
   alone.

   For a thread with no mission, run it with no argument:

   ```bash
   tsk thread start
   ```

   Starting a thread is a claim, so the command publishes at once: the thread and the
   binding go to origin, with any record you wrote earlier. This is one of the four points
   where records are published (thread start, thread pause, the end of the session, and
   when asked). Every other write, such as `tsk task set`, stays local until one of them.

   If you take a task, set it to `in-progress` and publish it straight away, so that no
   other actor takes the same task:

   ```bash
   tsk task set "<mission-id>" "<task-id>" status in-progress
   tsk ledger push "Claim <mission-id> <task-id>"
   ```

3. **Handle the result.**
   - Exit 0, output `started:<thread-id>`: a new thread was minted and bound.
     Tell me the new thread ID and confirm the mission it is working, or that it has
     no mission.
   - Exit 2, output `resume-required:<thread-id>`: this session or code worktree is
     already bound to a thread. This is a resume, not a start. Invoke the
     `tsk:resume-thread` skill with that thread ID instead of proceeding here.
   - Exit 1: the mission ID is unknown, or another error occurred. Report the
     command's stderr and stop. Do not retry with a guessed ID.

4. **Set the session title.** Run `tsk thread title` after exit 0. Exit 1 means there is no
   actor and no mission: skip this step. Exit 0 prints the title, `<robot name> (<mission
   ID>)`, the name alone, or the mission ID alone.
   - In a cloud session (`CLAUDE_CODE_REMOTE_SESSION_ID` is set), call `get_session` from
     the `claude-code-remote` MCP server with no `session_id` to read this session's ID.
     Then call `set_session_title` with that ID and the title.
   - In a terminal, no tool sets the title. Ask me to run `/rename <title>`.

If I want to work with no thread at all, run `tsk thread skip` instead. It records that
this session or code worktree works without a thread, and the Stop hook stops asking for
one. It is local to this clone and is never pushed.
