# 20. Publish in the background with a detached one-shot, not a daemon

Date: 2026-10-22

## Status

Accepted. Decided by Jim on 2026-10-22 (mission M-TSK-PERF). Built in 0.25.0. It changes
decision 8 of [ADR 0019](0019-publish-less-often-by-rule.md), which had the `Stop` hook
ask the agent to run `tsk ledger push`. Nothing else in ADR 0019 changes.

## Context

ADR 0019 made the end of a turn a publish point. The `Stop` hook blocked with a request
that the agent run `tsk ledger push` whenever the store held unpushed changes. Each such
request costs a model turn and its tokens, on every turn that wrote a record. Jim ruled
that this was never desirable: the agent has nothing to decide, and the publish is
mechanical.

A hook cannot push in the foreground either. A push takes 0.8 s through a 330 ms network
shim and 1.7 to 1.9 s on the live origin, and the hook runs at the end of every turn.

## Decision

1. **`tsk ledger push --background [message]`.** The message defaults to `Background
   publish`. The command:
   - exits 0 at once, with no output, when the ledger store holds nothing that origin
     lacks (it reads the local store and asks origin nothing),
   - exits 0 at once, with no output, when a push of this clone already holds the push
     lock,
   - otherwise starts a detached one-shot child and returns. The child runs the normal
     push path with the message.
2. **The child is not a daemon.** It is `tsk ledger background-push-run`, started with
   `setsid`, with its output in `background-push.log`. It has no socket, no loop and no
   state beyond the lock and the result file. It ends when the push ends.
3. **The push lock.** `push.lock` in the state directory of the clone, taken with
   `flock`. The parent takes it and the child inherits the open file description, so the
   lock lives as long as the child. The kernel frees it when the child ends however it
   ends, including `kill -9` and a container that stops. The child marks the descriptor
   close-on-exec at once, so the Dolt server and git, which it starts, do not hold the
   lock. `tsk ledger push`, `thread start`, `pause` and `stop` take the same lock before
   they commit, and wait for it for up to 60 s (`TSK_PUSH_LOCK_SECS`). Two pushes of one
   clone therefore never write the store together.
4. **The result file.** `background-push.result` in the state directory holds JSON: the
   state (`running`, `ok` or `failed`), the pid, the time, the commit that origin holds
   after the push, the error text, and whether a warning was shown. A `running` result
   whose lock nobody holds belongs to a child that was killed. A reader treats it as a
   failure named `interrupted`.
5. **A failure shows once.** The first `tsk` command that opens the ledger store after a
   failure, which includes `tsk thread guard` and `tsk thread session-start`, prints one
   line on stderr: `warning: the last background publish failed at <time>: <first line of
   the error>; run tsk ledger push`. It then marks the failure shown. A later successful
   push, in the foreground or in the background, clears or replaces the failure. Standard
   output does not change. `tsk ledger status` prints `Background publish: ok at <time>`,
   `failed at <time>: <error>` or `running` when a background publish has run.
6. **The `Stop` hook publishes and never blocks for it.** `tsk thread guard` keeps the
   unbound-thread check and its block decision. When that check passes and the store holds
   unpushed changes, it runs the code of `tsk ledger push --background` in its own
   process, prints nothing, and exits 0 whatever happens. `stop_hook_active` no longer
   matters to the publish: the hook never blocks for it, so there is no loop to break.
   `TSK_BACKGROUND_PUBLISH=off` switches the publish of the hook off.
7. **The rest is synchronous.** `thread start`, `pause` and `stop`, and `tsk ledger push`
   push in the foreground as before. Jim decided that a pause stays synchronous, with a 2 s budget; its code is unchanged in 0.25.0.
   The `SessionEnd` hook stays a synchronous best-effort push under 8 s.
8. **`tsk ledger status` names the server.** One line, `Server  running (pid 123, idle
   stop 1800 s)` or `Server  not running (switch off)`. It uses the code of `tsk ledger
   server status`, asks origin nothing, and is not in the `--porcelain` output.

## Why a detached one-shot and not a daemon

A daemon would hold a loop, a socket or a queue, a protocol to start and stop it, and a
way to replace a stale or hung one. ADR 0018 pays that cost for the Dolt server because
the server saves 60 to 100 ms on every SQL batch. A publish needs none of it: it starts
from a hook once, does one job and ends. The lock and the result file are the whole
interface, and the kernel keeps the lock correct after a crash.

A daemon would be needed for a periodic publish (a push every few minutes, with no hook to
start it), for batching several requests into one push, or for a publish that must survive
the end of the session and retry. None of those is wanted now. If a periodic publish is
wanted later, it can use the same one-shot child.

## Limits

- **A container killed during a push loses that push.** The next turn's `Stop` hook
  starts a new background publish, and it carries the changes. A container that stops
  between turns loses the records written since the last publish, as ADR 0019 already
  says.
- **A failure reaches the agent only as a warning** on a later command, and only on
  stderr. A session that never runs another `tsk` command does not see it. The
  `SessionEnd` hook then publishes by itself.
- **A hook that starts the child costs a process start.** The measured figures are in
  [2026-10-22-background-publish.md](../arch/2026-10-22-background-publish.md).
- **A foreground push can wait** up to 60 s for a background push that holds the lock,
  then stops with an error that names the lock file.
- **The default idle stop of the Dolt server is 1800 s** from 0.25.0 (see the amendment
  in [ADR 0018](0018-a-local-dolt-server-per-ledger-store-behind-a-switch.md)). A working
  session should never see a restart of the server, and a forgotten server still ends
  after 30 minutes. The choice is to be revisited (an M-TSK-TIDY task).
