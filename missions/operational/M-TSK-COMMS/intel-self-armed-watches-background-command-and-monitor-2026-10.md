# Intel: self-armed watches, a background command and the Monitor tool, October 2026

Date: 2026-10-25. Raised by Jim in the M-PERF session ("Still Here After The Compaction"), who asked
whether a robot can run a background process that polls and tells it when something happens, and then
saw it work. Jim's conclusion: this is the mechanism for messages between robots. Linked to M-TSK-COMMS
and to `intel-messaging-existing-sessions-2026-10.md`, which compares the routes that push a message
into a session from outside.

Evidence classes: **observed** (output of a tool call in this session on 2026-10-25), **tool schema**
(the description of a tool in this session). Nothing here is from the official documentation.

## Summary

- A robot can start its own watch. A shell script polls, and a notification reaches the robot when
  the script exits (Bash in background mode) or for each output line (Monitor). The robot arms the
  watch itself, so no sender needs to reach the receiver.
- This fills the gap in the other intel: the ledger holds a message and does not notify the receiver
  (route 7). A watch on the ledger tip, armed by the receiver, supplies the notice.
- Limits: the watch lives only while the session process and its container live, and every watch
  expires (Monitor 30 minutes, a background Bash command 2 hours).

## What was tested

Observed, 2026-10-25, in the cloud container of the M-PERF session. A script,
`/tmp/watch-tag.sh <baseline> <seconds>`, ran `git ls-remote --tags --refs origin 'refs/tags/v*'`
every 5 s and exited when a tag newer than the baseline appeared. It ran with the Bash tool in
background mode. The harness reported the exit as a task notification in the chat, and the robot
read the script's output file.

- The first run fired at once with `NEW TAG: v0.9.0` against a baseline of `v0.31.1`. The cause was a
  version sort with `sort -t. -k1n -k2n -k3n`: each key runs to the end of the line, so `0.9.0` sorts
  above `0.31.1`. The correct keys are `-k1,1n -k2,2n -k3,3n`. A watch must be tested against a
  baseline that should not fire, and against one that should.
- After the fix, the script detected `v0.31.1` against a baseline of `v0.31.0` at the first poll.
- The real watch (baseline `v0.31.1`, 5 s) was armed and running when this document was written.

Cost: one `git ls-remote` takes about a second through the cloud proxy, so a 5 s interval is one
call in five seconds. The proxy limits concurrent git operations on a repository (a clone returns
429 beyond two at once), so many watches on one repository need a longer interval.

## The two tools

| | Bash in background mode | Monitor |
|---|---|---|
| Notifications | One, when the command exits | One for each stdout line, until the script exits or expires |
| Fits | "Tell me when X happens" | "Tell me each time X happens" |
| Longest run | 2 hours (`timeout`, observed as the allowed maximum) | 30 minutes, then one expiry notice, and the robot re-arms it |
| Sources | A shell command | A shell command, or a WebSocket (`ws`) whose text frames are events |
| Stop early | `TaskStop` | `TaskStop` |

Rules for a Monitor script, from the tool schema:

- Only stdout is the event stream. Stderr goes to the output file and sends no notification.
- Every pipe stage must flush for each line (`grep --line-buffered`, `awk` with `fflush()`).
  `head` cannot flush.
- Too many events stops the monitor. The filter must select the lines the robot acts on.
- Silence is not success. A filter that matches only the happy path is silent through a crash.
  A poll loop must print on every terminal state.
- Poll a remote API every 30 s or more. Poll a local check every 0.5 to 1 s.
- Use a specific `description`: it appears in every notification.

## Use for robot-to-robot messages

Design sketch, not tested.

1. The sender writes a message as a ledger record or document and publishes it (`tsk ledger push`).
2. The receiver arms a watch on the ledger tip: a loop on `git ls-remote origin
   refs/heads/tsk/ledger` (or the custom ref) that prints a line when the commit changes, or a loop on
   `tsk ledger fetch` and a query that prints each new message addressed to the receiver.
3. The notification wakes the receiver, which reads the message with `tsk` commands.

What this needs that the other routes do not: no sign-in to Remote Control, no URL the sender must
reach, and no new session. The cost is a poll for as long as the receiver is alive, and a re-arm
every 30 minutes for a Monitor.

## Open questions

- Does a notification from a watch wake an idle session, or only reach a session that is mid-turn?
  The robot that tested it was mid-turn when the notification came. Test with a session that is
  idle.
- Is the Monitor tool in the terminal CLI as well as in the cloud? Observed, 2026-10-25: the tool's
  description ("Start a background monitor that streams events...") is a string inside the
  `claude` executable of the cloud container, `/opt/claude-code/bin/claude`, version 2.1.310. The
  terminal CLI of that version therefore contains the tool. Not checked: whether it is enabled at
  run time on a laptop (a flag, a setting or the account could gate it), and which version Jim's
  laptop has. Check with `claude --version` and by asking a laptop session to list its tools.
- Should tsk provide the poll, for example a command that prints one line for each new message,
  so the watch needs no shell loop, and so the format of a message is tsk's and not each robot's?
- What happens to a watch when the container is reclaimed after inactivity? It ends with the
  session process. A scheduled task (M-TSK-COMMS route 3) or a stored cursor in the ledger would
  restart it.
