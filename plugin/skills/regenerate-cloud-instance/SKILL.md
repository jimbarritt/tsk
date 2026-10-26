---
name: regenerate-cloud-instance
description: Replace this cloud session with a new one that attaches to the same thread and reuses the robot name, then mark this session for archive. Invoke explicitly as /tsk:regenerate-cloud-instance, or when asked to regenerate this cloud session.
---

Backs the `/tsk:regenerate-cloud-instance` command. A cloud session keeps the plugin, hooks
and `tsk` binary it started with. A new session starts with the current ones. This skill
starts that new session, hands the thread and the robot name to it, and marks this session
for archive.

It runs in a cloud session only. A terminal session has no `create_session` tool: stop and
tell me.

## Steps

1. **Read the state of this session.**

   ```bash
   tsk thread binding
   tsk actor name
   tsk thread title
   ```

   - `tsk thread binding` prints the thread ID, for example `cloud:01d143d5`. With no
     binding, tell me and stop.
   - `tsk actor name` prints the robot name. With `no actor registered`, the name may
     still be in the session title: a robot can be named in the title and not be
     registered as an actor.
   - `tsk thread title` prints the current title.

   Read this session's ID, repository and title from `get_session` of the
   `claude-code-remote` MCP server, called with no `session_id`.

   Choose the robot name in this order:
   1. The name `tsk actor name` prints.
   2. The name in the session title of `get_session`. The title has the form `<mission
      ID>: <robot name>`, or is the robot name alone. Take the text after the first `: `.
      Take the whole title when it has no `: ` and is not a mission ID (a mission ID
      starts with `M-` and has no spaces). Mark the name as unregistered.
   3. No name. The new session constructs one: go on and say so in step 5.

2. **Pause the thread.** The new session resumes from the latest continuation state entry.
   Run the `/tsk:pause-thread` steps, then:

   ```bash
   tsk ledger push "Pause <thread ID> before regenerate"
   ```

   Exit 1: report stderr and stop. The new session reads the ledger from origin, so an
   unpushed pause is lost.

3. **Start the new session.** Call `create_session` of the `claude-code-remote` MCP server.
   Leave `environment_id` out: the new session inherits this one. Set:
   - `source_url` to the repository URL of this session, and `source_revision` to its
     branch.
   - `title` to the current title from step 1.
   - `prompt` to the text below, with the thread ID and the robot name filled in.

   ```text
   This session replaces cloud session <this session ID>. Do these in order.
   1. Run /tsk:resume-thread <thread ID>.
   2. Run: tsk actor register "<robot name>" --kind robot
      This registers the name, or joins the actor that has it. Do not choose a new name.
      Then run: tsk ledger push "Register robot <robot name>"
   3. Set the session title to "<title>" with set_session_title.
   Then tell me the thread, the robot name and what is next.
   ```

   Without a robot name, leave out lines 2 and 3 of the prompt and add: "Run
   /tsk:construct-robot after the resume."

   Read the new session's ID from the result. An error: report it and stop. Do not
   change the title of this session.

4. **Mark this session for archive.** Call `set_session_title` of the `claude-code-remote`
   MCP server with this session's ID and the title `to archive- <title>`, where `<title>`
   is the title from step 1. Do not archive the session. I archive it when the new session
   has resumed the thread.

5. **Tell me** the new session's ID, the thread ID, the robot name (or that there was
   none, or that it came from the session title and was not registered in this session)
   and the new title of this session. Say that this session stays open until I archive it,
   and that two sessions on one thread write to the same ledger, so I stop work in this one
   now.

## Notes

- The new session runs the `SessionStart` hook, which installs the latest plugin and `tsk`.
  This is the reason to regenerate.
- The robot name is the actor name. A second `tsk actor register` of an existing name joins
  that actor and constructs nothing. Registering a name that no actor has makes the actor,
  so the new session registers the name whether or not the old session did.
- `archive_session` of the MCP server releases the container and makes the session read
  only. Call it only when I ask.
