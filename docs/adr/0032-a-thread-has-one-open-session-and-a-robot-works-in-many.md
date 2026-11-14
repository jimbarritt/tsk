# 32. A thread has one open session at a time, and a robot works in many sessions

Date: 2026-11-14

## Status

Accepted. Decided by Jim on 2026-11-13 and 2026-11-14 (mission M-TSK-ENV, task T-30). ADR
0024 (a thread is the unit of continuation), ADR 0030 (an actor belongs to a session, and a
harness instance is the continuity key) and ADR 0031 remain in force. This ADR states how
many sessions work a thread at once, and how many sessions a robot works in.

## Context

Releases 0.59.0 and 0.60.0 treated a robot as taken while a session used it: a new session
was not offered a robot that an open session held. The field trials showed two faults in
that rule.

- The domain model says the opposite. `docs/domain/ubiquitous-language.md` stated: "Two
  sessions may be one actor at the same time: the model allows it, nothing checks it."
- No place sees every session. A cloud session lists cloud sessions and not terminal
  processes. A terminal session sees its own machine and not the cloud. A rule about
  robots that needs every session cannot be checked.

The rule that protects the work is about the thread. A thread is one timeline of sessions
in order, with a pause entry at each hand-off. Two open sessions on one thread give two
contexts and two competing pause entries, whichever robots they are.

## Decision

1. **A robot is an identity.** A robot works in any number of sessions at the same time,
   in any number of harness instances. No rule limits it.

2. **A thread has at most one open session at a time.** That session is the holder of the
   thread. Two robots never work one thread at the same time, and one robot does not work
   one thread from two sessions at the same time.

3. **The thread only ties sessions together.** The mission and the task bind the work. A
   robot that works in a second session takes another thread, or no thread, and works any
   mission and task from there.

4. **A session takes a thread with a start or a resume, and gives it up with a pause, a
   stop or a detach.** A thread whose latest event is a pause is paused, and any session
   can resume it. A thread with a start or a resume after its latest pause is held. Both
   states are read from the ledger, so a cloud session and a terminal session read the same
   answer.

5. **A holder that no longer runs is stale.** A session that ended, crashed or was archived
   with no pause still shows as the holder. A resume of that thread is a take-over: the
   person confirms it, and the resume warns.

6. **A new session with no binding chooses the robot first.** It offers the robots that no
   open session uses, then a new robot, then a robot in use. It then chooses the work: a
   paused thread to resume, a new thread, or no thread. A held thread is shown as held and
   is not offered.

7. **The binding stays on the harness instance** (ADR 0030, decision 4). It answers which
   thread a new session of the instance continues. The holder answers which session works
   the thread now. In a terminal, a second session in a code worktree whose thread is held
   works with no thread, or runs in another code worktree.

## Consequences

- The check of 0.59.0 and 0.60.0 for a robot in use becomes a sort order and a mark. It no
  longer removes a robot from the choice.
- `tsk thread list` and `tsk thread next` show whether a thread is paused or held, and by
  which session.
- `tsk thread resume` of a held thread needs a confirmation, as a take-over.
- A session that ends with no pause leaves its thread held. The liveness checks (the
  `claude` process and the tmux pane in a terminal, `list_sessions` in a cloud session)
  say when a holder is stale.
- Archiving a cloud session stays a person's action. tsk never archives a session: an
  archive releases the container, and work that is not pushed is lost.
