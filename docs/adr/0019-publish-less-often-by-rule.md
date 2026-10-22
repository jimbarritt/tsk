# 19. Publish less often, by rule

Date: 2026-10-21

## Status

Accepted. Decided by Jim on 2026-10-18 and 2026-10-20 (mission M-PERF, task T-08). Built
in 0.24.0. ADR 0014 (Dolt is the store) and ADR 0017 (the ledger is a file remote carried
by plain git) stand. This ADR changes when `tsk` pushes, and nothing about how.

## Context

A publish costs a fetch check, a Dolt push and a git push. Through a 330 ms network shim
that is 0.8 s, and on the live origin it is 1.7 to 1.9 s. Before 0.24.0 four thread
commands paid it on top of the work they did: `start`, `pause`, `stop` and the binding
changes of `resume`, `detach` and `set mission`. The record commands (`tsk mission`,
`task`, `decision`, `question`, `ledger doc put`) never pushed, but the skills told an
agent to run `tsk ledger push` after them, so a session paid the publish many times.

Jim ruled on 2026-10-18 that a write must never wait for origin and that tsk should push
less often. On 2026-10-19 he chose to push at the end of a session, as beads does, and
noted that conflicts come only from new numbers in the same mission and from edits to the
same field of the same row, because two actors do not work the same task. The cost of the
rule is a window: a container reclaimed before a publish loses the records written since
the last one.

## Decision

1. **A record write never pushes.** It changes the local ledger store and returns.
2. **Records are published at four points, and only there.**
   - Thread start: the claim and the binding. Claims must be prompt, so that two actors do
     not take the same task.
   - Thread pause, and thread stop: the hand-off.
   - The end of the session: a best-effort `SessionEnd` hook, and the `Stop` hook.
   - When asked: `tsk ledger push`.
3. **Every publish carries every local change.** `tsk ledger push`, `thread start`,
   `thread pause` and `thread stop` commit the whole local store, so the records written
   since the last publish go out with the next one.
4. **A claim is a write that another actor must see at once.** Two writes are claims:
   - `tsk thread start`, which publishes by itself.
   - Setting a task or a mission to `in-progress` to take it. The command is a record
     write and does not push, so the skills tell the agent to run
     `tsk ledger push "Claim <mission> <task>"` straight after it.

   Every other write is progress and waits: a task marked `done`, a task added, a decision
   recorded, a question asked or answered, a mission field changed, a document written.
5. **`thread resume`, `detach`, `set mission`, `skip` and `prune-bindings` do not push.**
   `resume`, `detach` and `set mission` print `note: saved in the local ledger store; it
   goes out with the next push ...` on stderr. Their stdout does not change. `skip` and
   `prune-bindings` already wrote locally.
6. **The event queue keeps pushing.** `tsk events append`, `append-batch` and
   `advance-watermark` hand an external message to another process. They are not progress
   on a task, and a consumer on another machine reads the queue from origin.
7. **The `SessionEnd` hook publishes if there is something to publish.**
   `plugin/hooks/session-end.sh` reads `tsk ledger status --local`, which asks origin
   nothing, and runs `tsk ledger push "End of session"` under a timeout of 8 s
   (`TSK_SESSION_END_TIMEOUT_SECS`) only when the store holds changes or unpushed commits.
   It exits 0 whatever happens. GitHub Copilot CLI gets the same script under its
   `sessionEnd` event.
8. **The `Stop` hook asks for a publish when there is something to publish.** (Replaced in
   0.25.0 by [ADR 0020](0020-publish-in-the-background-with-a-detached-one-shot.md): the
   hook now publishes in the background and does not ask the agent.)
   `tsk thread guard` blocks with a request for `tsk ledger push` when `tsk ledger status
   --local` shows unpushed changes, and says nothing otherwise. It does not block again
   when the hook input has `stop_hook_active` set, so an offline origin cannot hold a
   session open. The unbound-thread check comes first and is unchanged.
9. **Stale reads are acceptable.** A session sees another actor's work after that actor
   publishes and the session next fetches. A session start and `thread list` fetch, as
   before.

## Not decided here

Running `thread pause` in the background stays a future option. A periodic publish stays
open too.

## Consequences

- A `resume`, a `detach` and a `set mission` cost the fetch check and no push. Measured
  numbers are in [2026-10-21-publish-by-rule.md](../arch/2026-10-21-publish-by-rule.md).
- A session that never reaches a publish point loses its records if the container is
  reclaimed. The `SessionEnd` hook is best effort: it does not run when the container is
  killed. The `Stop` hook asks at the end of a turn, so the window is one turn when the
  agent follows it.
- A binding made by `resume` is not visible to other actors until a publish point. The
  takeover warning reads continuation entries, which `pause` publishes, so the warning
  does not depend on a binding being published.
- Two actors can take the same task if the second one does not see the first one's claim.
  The claim of a task is published by `tsk ledger push` straight after it is set, and the
  second actor sees it after its next fetch (a session start, or `tsk ledger fetch`).
- A session that holds unpushed changes reads origin's state only when it publishes. The
  refresh of a thread command, a session start and `thread list` keeps local changes and
  does not merge origin into them (`tsk ledger fetch` refuses them). A clone that wants to
  see another actor's claim while it holds unpushed work runs `tsk ledger push` first,
  which merges. This is the stale read that Jim accepted.
- An older plugin that tells an agent to push after every write still works. It spends
  publishes that the rule would save.
