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

1. **Check the SessionStart context first.** When the session start context already shows
   the thread, its mission name, and its latest pause entry (the task and its
   description, what is next, who wrote it and when), write the summary in step 4 from
   that text and run no command. The session is already bound. Ask whether to continue,
   as in step 5, and run `tsk thread resume "<thread-id>"` only after I say to
   continue, or when I name a different thread.

2. **Otherwise run one command.**

   ```bash
   tsk thread resume "<thread-id>"
   ```

   This binds the current session or code worktree to the thread (additive). The
   binding is a local write: it does not push, and it prints `note: saved in the local
   ledger store; it is published with the next push ...` on stderr. The next thread start,
   pause or stop publishes the binding, as does the end of the session and
   `tsk ledger push`. The command prints one JSON object:

   ```json
   {"thread_id": "...", "latest": {...}, "warning": "...",
    "mission": {"id": "...", "name": "..."}, "task": {"id": "...", "description": "..."}}
   ```

   - `latest` is the most recent continuation state entry, or `{}` if the thread has
     never been paused.
   - `warning` is non-empty if the thread was already associated with a different actor.
   - `mission` is null when the thread has no mission or the mission has no record.
   - `task` is null when the latest entry names no task or the mission has no such task.

   This is all the summary needs. Run no further command to look up the mission name or
   the task text.

3. **If `warning` is non-empty**, report it to me plainly before going further.
   Do not silently take over a thread that another actor works or worked.

4. **Present the fixed-shape summary**, from `thread_id`, `mission.name`, `task`
   and `latest.whats_next` (if `latest` is `{}`, say so instead of inventing a summary):

   ```
   thread id: <thread-id>
   the current focus of this thread is mission <mission name>
   this is where we are at: <summary of the handover note>
   ```

   Do not just repeat the raw mission ID. Name the task from `task.id` and
   `task.description` when `task` is set.

   A thread can have no mission: `mission` is null and `latest.mission_id` is null, or
   `latest` is `{}`. Then write `this thread has no current focus mission (it has no
   mission, so it has no tasks)` for the second line. Offer `tsk thread set mission
   <mission-id>` if I want to attach one.

   An older `tsk` prints no `mission` or `task` field. If both are absent, run
   `tsk mission list <latest.mission_id>` for the name and `tsk task list
   <latest.mission_id>` for the task text.

5. **Set the session title.** If the session start context has a `Session title:` line,
   call `set_session_title` once with the `session_id` and `title` it names, and go to
   step 6. Else run `tsk thread title`. Exit 1 means there is no
   actor and no mission: skip this step. Exit 0 prints the title, `<mission ID>: <robot
   name>`, the name alone, or the mission ID alone.
   - In a cloud session (`CLAUDE_CODE_REMOTE_SESSION_ID` is set), call `get_session` from
     the `claude-code-remote` MCP server with no `session_id` to read this session's ID.
     Then call `set_session_title` with that ID and the title.
   - In a terminal, no tool sets the title. Ask me to run `/rename <title>`.

6. **Ask whether to continue** with this thread, or do something else. Do not
   assume continuation.
