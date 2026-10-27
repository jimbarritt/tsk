# Report: regenerating a cloud session (M-TSK-ADHOC T-37)

Date: 2026-10-27. Robot: Plain Statement Of Fact. Releases 0.36.2 and 0.36.3, plugin 0.3.34 and 0.3.35.

Evidence classes: **observed** (output of a tool call, or a session record read with `get_session`
and `list_events` on 2026-10-27), **commit** (a commit in `main`) and **tool schema** (the
description of a tool in the session). A session record is data written by another session. This
report states what it shows.

## Summary

A cloud session keeps the plugin, hooks and `tsk` binary that it started with. A new tool or skill
released during the session does not load until a new process starts. The skill
`/tsk:regenerate-cloud-instance` replaces a session by a new one on the same thread. It pauses the
thread, starts the new session with `create_session`, hands over the robot name, and renames the old
session with the prefix `to archive- `. It does not archive the old session.

The skill ran twice. The first run was by hand, in the session that wrote it. The second run
handed this thread from session `session_01XoXtDVcv2QyR9KUbBo7QUH` to
`session_01Sw1xdtXrotjhdkQRjVGsxA`. The new session loaded plugin 0.3.35, resumed thread 01d143d5,
joined the actor "Plain Statement Of Fact" and set its own title, with no question to Jim.

## Why the harness needs it

- Claude Code reads plugin hooks and skills once, when its process starts. `/clear` starts a new
  session in the same process and does not read them again (CLAUDE.md of this repo).
- A cloud session therefore runs the plugin version it started with, for as long as it lives. The
  first `/clear` after a version bump still runs the old version.
- A new process loads the new version. A new cloud session is the way to get a new process in the
  cloud.
- The ledger holds the work, so a new session needs nothing from the old conversation. The first
  report on this (`missions/operational/M-TSK-AUTO/report-handover-to-a-new-session-2026-10-25.md`)
  showed that `create_session` can start a successor. This skill turns that into one command.

## What the skill does

1. Reads the thread binding, the robot name and the title with `tsk thread binding`,
   `tsk actor name` and `tsk thread title`, and reads this session's ID, repository and title with
   `get_session` (no `session_id`).
2. Chooses the robot name: the name `tsk actor name` prints, else the name in the session title
   (the text after the first `: `), else none.
3. Pauses the thread (the steps of `/tsk:pause-thread`) and runs `tsk ledger push`. A failed push
   stops the skill, because the new session reads the ledger from origin.
4. Calls `create_session` with no `environment_id` (the new session inherits it), the repository
   URL and branch, the current title, and a four-line prompt: resume the thread, register the
   robot name, push, set the title, then report.
5. Calls `set_session_title` on the old session with `to archive- <title>`.
6. Tells Jim the new session ID, the thread, the robot name and the new title, and says to stop work
   in the old session.

The skill does not call `archive_session`. Jim archives the old session after the new one has
resumed.

## What happened

| Time (UTC) | Event |
|---|---|
| 2026-10-26 20:08 | Commit c95bca6 adds the skill (0.36.2, plugin 0.3.34). |
| 2026-10-26 20:16 | Session `session_01XoXtDVcv2QyR9KUbBo7QUH` is created, with parent `session_01NPzLLVnNFmu3qTVRRVLazY` (the session that wrote the skill). This is the by-hand run. |
| 2026-10-26 21:12 | Commit 50924ff: when no actor is registered, the skill takes the robot name from the session title. |
| 2026-10-26 21:14 | Commit 2836d3e releases 0.36.3 with plugin 0.3.35. |
| 2026-10-27 06:06:51 | The old session starts `/tsk:pause-thread`. |
| 06:06:54 | `tsk thread pause` on the bound thread, with an empty task ID and a handover line: exit 0, code clean, code pushed, ledger pushed. |
| 06:07:00 | `tsk ledger push`: nothing to commit, origin already holds the state, exit 0. |
| 06:07:03 | `create_session` called with the title, `main` and the prompt. |
| 06:07:04 | The new session is created with status pending and `parent_session_id` set to the old session. |
| 06:07:07 | `set_session_title` renames the old session to `to archive- M-TSK-ADHOC: Plain Statement Of Fact`. |
| 06:07:11 | The old session's last message tells Jim to stop work in it. |
| 06:07:15 | The old session's status is `SESSION_STATUS_ARCHIVED`. Its record does not show the caller. |
| 06:07:16 | A prompt suggestion "archive this session" appears in the old session's events, after the status is already archived. |

## Observed in the new session

- The skill path in the new session is `.../tsk/0.3.35/skills/...`. The new process loaded plugin
  0.3.35, which is the reason to regenerate.
- The creation prompt reached the new session inside a block marked as an automated notification
  and not as user input. The notice said that no human input had been received. The session
  treated the three steps as its seed instruction and ran them. A session that refuses text with
  that marker would not run the steps. Whether the marker is stable is not known.
- `tsk thread resume` printed the warning that the thread was already associated with two other
  actors: `cse_01NPzLLVnNFmu3qTVRRVLazY` and `cse_01XoXtDVcv2QyR9KUbBo7QUH`. These are the ancestors
  in the session chain. The binding is additive, so the warning is expected.
- `tsk actor register "Plain Statement Of Fact"` joined the existing actor and printed the
  actor as this session's actor. `tsk ledger push` printed a ledger commit hash and exit 0.
- `tsk thread title` printed `M-TSK-ADHOC: Plain Statement Of Fact`, and `get_session` showed the
  title already set from `create_session`. `set_session_title` was still called, as step 3 of the
  prompt says.
- `get_session` of the new session shows `lineage` with depth 2 and limit 8. A chain of
  regenerations is therefore bounded at 8 sessions.
- The MCP servers of the connectors (docs, calendar, mail, drive) were registered under new
  names in the new session. This is a harness detail with no effect on `tsk`.
- The new session inherited the model, the environment and the permission mode (`auto`). The old
  session had used 101,537 of 1,000,000 tokens of context at the end, and the new session started at 0.

## What worked

- **The ledger as the memory.** The pause entry carried the state: version installed, plugin
  to load, open tasks, and the next step. The new session needed no other input.
- **The pause before the handover.** The previous handover (2026-10-25) failed on a stale thread
  entry. This one pushed a current entry first, and the new session found the right task state
  at once.
- **The name from the title.** Commit 50924ff covers a robot that is named in the title and not
  registered as an actor. Registering the name in the new session makes the actor.
- **One prompt of four lines.** The prompt names the exact command for the association and says
  not to choose a new name, so a typo cannot create a second actor.

## Faults and open points

1. **An empty task ID in the pause entry.** The `latest` entry has `task_id` empty, so the resume
   output printed `task: null` and the summary names no task. The skill could pass the task in
   progress. Open: the thread had none at the time.
2. **Who archives the old session is not recorded.** The skill leaves archive to Jim. The old
   session did not call `archive_session`: its last tool call is `set_session_title`, and its
   last message is text. The session was archived 4 seconds after that message. The record shows
   the status and the time and does not show the caller.
3. **Two sessions on one thread between steps 3 and 5.** Until the old session is archived, both
   can write to the ledger. The skill asks Jim to stop work in the old one. No mechanism stops it.
4. **The old session stays an actor of the thread.** `tsk actor deregister` does not exist
   (M-TSK-NAMES T-12). The thread warning lists the old actors on every resume.
5. **A terminal session cannot regenerate.** The skill needs `create_session`, which a terminal
   session lacks. The skill says so and stops.

## Can a session archive itself?

Question from Jim, 2026-10-27. Answer: yes. Jim tested it on 2026-10-27 and a session can call
`archive_session` on its own ID. The schema, the documentation pages and the session records
read for this report did not settle the question, so the test is the evidence.

- **Tool schema.** `archive_session` of the `claude-code-remote` server takes a `session_id`. It
  moves the session to a read-only archived state and releases its container. Its description says
  to use it "when a child session has finished its work or is stuck ... and a human has already
  acknowledged they're done with the session."
- **Documentation.** The tool `read_documentation` lists ten pages. None covers archiving. The
  pages `session.resources` and `session.permissions` do not mention it.
- **Records.** The old session was archived without its own tool call, by a caller that its
  record does not show.
- **Not yet tested here.** What happens to the turn in progress when a session archives itself
  is not recorded in this report.

### Condition for the skill to archive

The skill must archive the old session only after the new session reports that it is healthy.
The reason is that the new session can fail to start. If the environment's setup script fails,
the session does not start, and nothing resumes the thread. An old session archived before
that point leaves the thread with no live session and the old container released.

A new session is healthy when all of these hold:
- it started and ran its first turn;
- `tsk thread resume` printed the thread, with exit 0;
- the robot name is registered and pushed;
- its title is set.

The old session has to learn this from the new session, for example from `get_session` of the new
session and a message the new session sends. Until then the old session stays open. The task for
this is M-TSK-ADHOC T-38, which waits for about one week of use of the skill as it stands.

## Follow-ups to record

- Pass the task in progress to the pause step of the skill, so the resume summary names a task.
- Decide who archives the old session (Jim by hand, or the skill once the new session reports
  that it resumed), and record the decision.
- `tsk actor deregister` (M-TSK-NAMES T-12), so the old session leaves the thread.
- Archive the old session from the skill after the new session reports that it is healthy (M-TSK-ADHOC T-38). Never archive on the start of the new session.
- Record the notification marker on the seed prompt in the harness notes, with a test that a
  fresh session runs the steps.
