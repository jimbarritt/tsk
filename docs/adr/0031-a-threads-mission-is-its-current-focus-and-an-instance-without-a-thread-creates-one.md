# 31. A thread's mission is its current focus, and only an instance with no thread creates one

Date: 2026-10-31

## Status

Accepted. Decided by Jim on 2026-10-31 (mission M-TSK-ADHOC, decisions of that date and
tasks T-47 to T-50). ADR 0024 (a thread is the unit of continuation) and ADR 0030 (an actor
belongs to a session, and a harness instance is the continuity key) remain in force. This
ADR states what a change of mission does to a thread, and when a thread is created.

## Context

In one cloud session, a request to work on another mission paused the thread and started a
new one. ADR 0024 allows a thread to cross missions: one line of work that crosses missions
keeps one continuation history. The skills did not follow it. `start-thread` ran on any
request that named a mission, and `switch-thread` offered to stop the thread left behind.

The ledger shows threads that cross missions. One thread ran M-DOLT and then M-ENV. This
session's thread is bound to M-TSK-IDEA, and its work covered M-TSK-ATTRIB,
M-TSK-SCALE, M-TSK-COMMS and M-TSK-NAMES.

Jim, 2026-10-31: in a supervised session the thread is the human's attention with one
robot. A robot that runs on a schedule is different. Its thread and its mission are the
same line of work.

## Decision

1. **A thread is a line of work with one continuation history.** Its mission is its
   current focus, stored in `threads.mission_id`. A thread has at most one focus at a time,
   and may have none.

2. **Two commands write the focus.** `tsk thread start` sets it for a new thread, and
   `tsk thread set mission` changes it. A pause reads it and records it in its entry. The
   session start context, `tsk thread title` and `tsk thread list` read it.

3. **A change of mission keeps the thread.** The `switch-mission` skill pauses on the old
   focus, runs `tsk thread set mission`, and sets the session title. The pause records the
   hand-over for the old mission. The next pause records the new focus.

4. **A thread is created only when a harness instance has no bound thread and no thread
   to join.** Neither a mission nor a session creates a thread. A new instance joins a
   thread it is given or names. A regenerated cloud session and a scheduled task do this.

5. **A request to work on a mission checks the binding first.** With a bound thread, it
   runs `switch-mission`. `start-thread` runs only when nothing is bound, and it offers
   existing threads to resume before it creates one.

6. **`switch-thread` moves a session to a different line of work.** The thread left behind
   stays for its actor to resume. A thread is stopped only when its line of work is
   finished.

## Consequences

- The ledger layout does not change.
- The session start context reads `The current focus of this thread is mission <id>
  (<status>): <name>.` The binary prints it after release 0.41.0.
- The skills change: `switch-mission` is new, and `start-thread`, `switch-thread` and
  `resume-thread` follow this ADR.
- Not built: the session start hook creates a thread, with no prompt, for an instance that
  has no bound thread and no thread to join (M-TSK-ADHOC T-48).
