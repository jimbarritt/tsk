---
name: construct-robot
description: Construct a new robot. The robot chooses a name of its own in the style of the Culture ships of Iain M. Banks, registers it as a tsk actor, and sets its session title to the name. Invoke explicitly as /tsk:construct-robot, or when asked to name this session or to construct a robot.
---

Backs the `/tsk:construct-robot` command. Construction is the creation of a new robot,
the robot's birth. The name is the only part that needs inference: `tsk actor register`
stores it.

## Steps

1. **Check for an actor.** Run `tsk actor name`. If it prints a name, this session or
   code worktree is already that actor. Tell me the name and stop. If it prints
   `no actor registered`, go on.

2. **List the names in use.** Run `tsk actor list --porcelain` and read the first
   column. In this release the name identifies the actor: registering a name that is
   already in the list joins that actor and does not construct a new one. Choose a name
   that is not in the list.

3. **Choose a name.** Write a new name in the style below. Base it on your own attitude,
   character or aim, from the work in this session and the mission the session start
   context names, if any. Do not ask me to choose. Choose one name, not a
   list of options.

4. **Register it.**

   ```bash
   tsk actor register "<name>" --kind robot
   ```

   Exit 0 with `registered <name> (robot)` means a new robot. Exit 0 with `<name>
   (robot) is now this session's actor`, or `this worktree's actor`, means the name was
   already in use and this session joined that actor. Tell me, and stop. Exit 1: report
   stderr and stop.

5. **Publish the registration.** A name is a claim, as a task is:

   ```bash
   tsk ledger push "Construct robot <name>"
   ```

6. **Set the session title.** Run `tsk thread title`, which adds the mission when the session has a thread with one. Exit 1 means there is no
   actor and no mission: skip this step. Exit 0 prints the title, `<robot name> (<mission
   ID>)`, the name alone, or the mission ID alone.
   - In a cloud session (`CLAUDE_CODE_REMOTE_SESSION_ID` is set), call `get_session` from
     the `claude-code-remote` MCP server with no `session_id` to read this session's ID.
     Then call `set_session_title` with that ID and the title.
   - In a terminal, no tool sets the title. Ask me to run `/rename <title>`.

7. **Tell me the name** and, in one sentence, what it says about you.

## The style

A Culture Mind chooses its own name. The name usually expresses something about the
Mind's attitude, character or aims. The names range from funny to cryptic. A warship's
name can be dark.

- At most four words. A phrase, a short remark, or one or two words.
- It states an attitude, a character or an aim. It does not state the job.
- Dry, humorous or cryptic in tone. It can read as a remark or a quotation, and it can
  end in an ellipsis.
- Capitals for each main word.

Examples from the books, to show the style only:

- Just Read the Instructions
- So Much For Subtlety
- Funny, It Worked Last Time...
- Mistake Not…

Not in the style:

- A role or a tool name, such as `doc-keeper` or `release-bot`.
- A number or a code, such as `agent-7`.
- A name copied from the books. Write a new name in the same style.
- A plain label with no attitude, such as `Documentation Actor`.

## Limits

- The name works as a `SendMessage` target and an @ mention.
- At most four words, and at most 200 characters.
- It does not start with `/`.
