# Report: handing a robot's work to a new session, 2026-10-25

Mission: M-TSK-AUTO (Autonomous coordination), blocked by M-TSK-COMMS. Written by the robot
"Still Here After The Compaction" about its own handover. Raised by Jim, who ran the experiment and
asked for the report: "we already proved here that you can fire up new sessions and give them
instructions."

Evidence classes: **observed** (output of a tool call, or a session record read with
`get_session` and `list_events`, on 2026-10-25) and **tool schema** (the description of a tool in
the session). The new session's record is data written by another session. This report states what
it shows.

## Summary

- One session can create another and instruct it. The `create_session` tool of the
  `claude-code-remote` server started a new cloud session on the same repo, with a title and a
  first message. The new session did all four steps of the message and stopped at the right
  point, in 34.7 seconds for the first turn, at a cost of 0.22 US dollars.
- A new process loads the plugins that the old process never loaded. The earlier session had
  run for 13 days and never had the `tsk` plugin skills. The new session started with the plugins
  at project scope: `tsk` 0.3.28 and `swe` 0.23.1.
- The robot's identity moved by name. The new session joined the existing actor "Still Here After
  The Compaction" with `tsk actor register`. The actor list had two actors before and after, and the
  actor now has two session IDs.
- The handover was not clean. The latest thread entry was 2 days old and named another mission, so
  the new session stopped and asked. The earlier session had not written a pause entry.

## What happened

| Time (UTC) | Event |
|---|---|
| 12:37:36 | The earlier session called `create_session` with a title, the repo at `main`, and a first message of four steps and a context section. The record shows `origin` as `claude_code_mcp_seed` and `parent_session_id` as the earlier session. |
| about 12:37:59 | The first turn started. The harness waited 12.9 s for the session start before it read the message, and the first request went out 14 s after the process spawned. |
| 12:38:05 to 12:38:30 | Step 1: `tsk --version`, `dolt version`, `just --version`, `claude plugin list` and `tsk actor list`: `tsk` 0.32.2, `dolt` 2.5.0, `just` 1.59.0, the two plugins at project scope. A check of the latest release and a ledger fetch showed `origin/main` had only a setup script commit and no rename in progress. |
| 12:38:17 | Step 2: `tsk actor register "Still Here After The Compaction" --kind robot`, then `tsk actor list`. Two actors before and after. The new session ID joined the existing row. |
| 12:38:20 | Step 3: `tsk thread resume 25366ae1`. |
| 12:38:23 | Step 4: `tsk thread title` and `get_session`. The title was already `M-TSK-PERF: Still Here After The Compaction`, so `set_session_title` was not called. |
| 12:38:34 | The turn ended: 7 model turns, 1,785 output tokens, 0.224 US dollars, status idle, "need_input". |

The final message of the new session listed the steps done, the latest entry (mission M-TSK-SCHEMA,
task T-15, what is next "timing 4", written 2026-10-23 20:45 UTC), and three open points: the entry names a
mission that is not in the open list, `main` had moved past the code commit in the entry, and the
resume warned that the thread already had the earlier session's actor. It asked whether to continue.

For comparison, the earlier session, read with `get_session` before the handover: created 2026-10-12,
about 4.88 million output tokens, a cost of about 516 US dollars in list prices, and 637 thousand
tokens of context at the end.

## What worked

- **Instructions by first message.** The message held the steps, the exact command for the
  association, a check that detects a misspelt name, and the context a reader needs. The new
  session followed it in order and used its own judgment where the message was silent: it
  checked that no rename was in progress before it wrote, because the message said to.
- **The title from `create_session`.** The title argument set the session title. The session
  checked it against `tsk thread title` and left it.
- **The association check.** The before and after count of actors detects a typo. It passed.
- **Cost.** A new session starts with a context of about 88 thousand tokens (the repo's
  `CLAUDE.md` and the tools) against 637 thousand for the old one.
- **The ledger as the memory.** The new session needed nothing from the old conversation. Every fact it
  used came from the ledger, the repo or the first message.

## What went wrong

- **A stale thread entry.** The earlier session moved the thread from M-TSK-SCHEMA to M-FIELD and
  then to M-TSK-PERF with `tsk thread set mission`, and never ran `tsk thread pause`. The latest
  entry therefore named an old mission and task, and the new session had to stop. The rule that
  follows: write a pause entry before a handover, as the last act of the old session.
- **The rename was not confirmed by a command.** Jim said the rename had finished. The earlier session
  did not check this in the ledger before it wrote the message, and the message told the new
  session to check. The check passed, but the sender should verify what it states.
- **A pipe through `head`.** The new session ran `tsk thread resume 25366ae1 2>&1 | head -80`. That
  hides the exit code, the fault that Jim's field test recorded (M-TSK-ADHOC T-18). The
  first message did not state the rule, and the plugin skill text does not say it yet.
- **The plugin skills were not exercised.** The new session ran the `tsk` commands directly and did
  not call the `resume-thread` skill. The report cannot say that the skills loaded. The plugin
  list shows the plugin enabled at project scope in a new process, which is the precondition.
- **The association is by name.** `tsk actor register` joins an existing actor when the name
  matches, and creates one when it does not. The only guard was the before and after check in the
  message. M-TSK-ADHOC T-33 (a uid for each robot) removes the need for it.
- **Two sessions, one actor, and no end.** The old session still has its row. The actor has two
  session IDs until `tsk actor deregister` exists (M-TSK-NAMES T-12).

## Judgments

- The mechanism is enough for a supervised handover: a robot can start a successor and give it
  a task, and the successor starts from the ledger. It is not yet an autonomous handover,
  because a person said when to do it and answered the first question.
- A good first message has four parts: the steps in order, the exact commands where a wrong word
  does harm (the actor name), a check that detects the harm, and the context that is not in the
  ledger. Anything in the ledger should come from the ledger, not from the message.
- A handover should be a command and not a prompt written by hand. The command would write the
  pause entry, create the session, and give it the association step and the title. A hand-written
  message depends on the sender's memory, which is the failure above.

## Follow-ups to record

- A handover command or skill: pause the thread (so the entry is current), create the session with
  the title, and send the association and resume steps. Task for M-TSK-AUTO.
- The rule "do not pipe `tsk` through `head`" in the plugin skill and the SessionStart context
  (M-TSK-ADHOC T-18) and in the handover message.
- A check that a rename is not in progress, as a ledger fact and not a person's word: the other
  robot's latest change is a release. A watch on the ledger tip (the intel document under
  M-TSK-COMMS) can supply it.
- Association by uid (M-TSK-ADHOC T-33) and `deregister` (M-TSK-NAMES T-12), so the old session can
  leave the actor.
- Test the skills in the new process: ask the new session to run `/tsk:resume-thread` and
  `/tsk:pause-thread`, and record the result.
