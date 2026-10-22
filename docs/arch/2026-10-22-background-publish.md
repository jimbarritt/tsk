# Background publish: how the Stop hook publishes, and what it costs

Written by Claude (Sonnet 5.5) for M-PERF, on 2026-10-22. The decision is
[ADR 0020](../adr/0020-publish-in-the-background-with-a-detached-one-shot.md). Dolt 2.5.0,
release builds of `tsk` 0.25.0, one machine (4 CPUs). A difference under 10 % is not a
measured saving.

## Summary

- The `Stop` hook no longer asks the agent to push. When the ledger store holds unpushed
  changes, `tsk thread guard` starts a detached one-shot child that publishes, prints
  nothing, and returns. The agent spends no model turn on the publish.
- The hook with nothing to push takes 263 ms with the Dolt server off and 26 ms with it
  on. With unpushed work it takes 239 ms off and 31 ms on, which includes starting the
  child. The targets were 400 ms off and 100 ms on.
- `tsk ledger push --background` returns in 183 ms off and 25 ms on. The child then
  needs 0.44 s (server on) to 0.76 s (server off) through a 330 ms network shim, and the
  agent does not wait for it.

## The mechanism

```
tsk thread guard (Stop hook)
  unbound check (unchanged; may print a block decision and stop)
  background::start
    store missing                    -> nothing
    push.lock held by another push   -> skip (the other push carries the changes)
    store holds nothing unpushed     -> nothing
    otherwise: write background-push.result = running
               spawn "tsk ledger background-push-run" (setsid, log file, lock fd inherited)
               return
child
  keep the lock fd, set close-on-exec on it
  normal push path (trust the last fetch, commit, push documents, push data)
  write background-push.result = ok (commit) or failed (error)
  exit; the kernel frees the lock
```

### Files in the state directory of the clone

| File | Holds |
|---|---|
| `push.lock` | an `flock`, taken by every push of the clone. The background child holds it for its whole run |
| `background-push.result` | JSON: `state` (`running`, `ok`, `failed`), `pid`, `time`, `commit`, `error`, `shown` |
| `background-push.log` | standard output and error of the last child |

### Kill and hibernation safety

The parent takes the lock and the child inherits the open file description. The parent
exits, and the child keeps the lock. If the child is killed with `SIGKILL`, or the
container stops, the kernel closes the descriptor and frees the lock. A `running` result
with no lock holder is the trace of a killed child. The next reader rewrites it as a
failure whose error starts with `interrupted`, and shows the warning once. The child sets
close-on-exec on the lock descriptor at once, so neither the Dolt server it may start
nor git holds the lock after the child ends.

### What a failure looks like

```
warning: the last background publish failed at 2026-10-22T09:14:03Z: rebase onto origin's refs/heads/tsk/docs (...) conflicted; the rebase is aborted.; run tsk ledger push
```

The line goes to stderr, once, from the first command that opens the ledger store after
the failure: any `tsk` command that touches the ledger, `tsk thread guard` and `tsk
thread session-start`. Standard output does not change. `tsk ledger status` also shows
`Background publish: failed at <time>: <error>` until a push succeeds.

## Timings

Method: a bare scratch origin and a clone, a ledger with 20 missions, one thread on
`M-T01`, a cloud session ID, and `tsk` release builds. Each figure is the median of 7
runs, in ms, with the wall time taken around the command (`date +%s%N`). For the shim rows,
a shell stand-in for `git` first on `PATH` sleeps 330 ms before `fetch`, `push`,
`ls-remote`, `clone` and `pull`, as the C `gw.c` of
[2026-10-18-remote-growth-design.md](2026-10-18-remote-growth-design.md) (Setup) does
(the shell stand-in adds a few ms to each `git` call). The other rows use the plain
scratch origin on the same disk. The Dolt server idle time was 600 s. The script is kept
out of the repository; the method above reproduces it.

| Command | Server off | Server on |
|---|---|---|
| `tsk thread guard`, nothing to push | 263 | 26 |
| `tsk thread guard`, unpushed work (starts the child) | 239 | 31 |
| `tsk ledger push --background`, unpushed work | 173 | 25 |
| `tsk ledger push --background`, unpushed work, 330 ms shim | 182 | 33 |
| The child, start to end, 330 ms shim | 764 | 439 |
| `tsk ledger push` in the foreground, 330 ms shim (reference) | 569 | 404 |

Raw runs:

- guard, nothing to push: off 257 272 244 263 284 238 263; on 26 25 23 25 28 26 26.
- guard, unpushed: off 245 256 253 234 210 230 239; on 28 32 27 31 33 31 30.
- `push --background`: off 141 169 170 175 178 197 173; on 25 27 31 22 25 24 23.
- `push --background` on the shim: off 173 188 179 182 157 189 203; on 34 30 33 32 36 32 37.
- child end to end on the shim: off 760 764 774 764 729 782 777; on 448 439 438 438 439 456 455.
- foreground push on the shim: off 580 569 566 565 591 563 589; on 413 404 404 413 403 403 409.

### Reading the figures

- With the server off, every `tsk` command that opens the store starts `dolt` processes
  (60 to 100 ms each), so the hook costs about 240 to 260 ms whether or not it has work.
  The decision to publish adds the lock, the result file and the spawn: under 10 % of the
  cost. With the server on, the hook is one connection and a few queries.
- The child costs more than a foreground push (+195 ms off, +35 ms on). It starts a new
  `tsk` process, opens the store, and writes the result file. The agent does not wait for
  it. The extra time is the length of the window in which the records are not on origin.
- The publish rides on the next turn's `Stop` hook after a failure, and on the
  `SessionEnd` hook at the end. A turn that writes nothing starts nothing.

## What the tests show

`cli/tests/background.rs`, run with `TSK_DOLT_SERVER=on` and `off`:

- a background push publishes (origin's documents ref moves, the unpushed count goes to
  0, the result is `ok` with origin's commit) and returns at once;
- with nothing to push no process starts and no file is written;
- two simultaneous background pushes start one child (counted in `/proc`, with a
  stand-in `git` that sleeps in `push`);
- a push in progress makes a second one skip, and a foreground push stops with a named
  error when the lock stays held;
- a failure (origin's URL is bad) writes the result, shows the warning once on the next
  command with standard output unchanged, and a later push clears it;
- `kill -9` of the child frees the lock, and the next command shows `interrupted`;
- the guard prints nothing and returns with unpushed work, still blocks for a missing
  binding, ignores `stop_hook_active` for the publish, and honours
  `TSK_BACKGROUND_PUBLISH=off`;
- `tsk ledger status` names the server in both forms of the switch.

`cli/tests/hooks.rs` runs `plugin/hooks/stop.sh` with a stand-in `tsk`: it runs `tsk
thread guard` with the hook input on standard input, and exits 0 without `tsk`.

## Limits

- A container killed during the child's push loses that push. The next turn's hook
  starts another, and it carries the same changes.
- The warning reaches an agent only through a later `tsk` command.
- An older `tsk` (before 0.25.0) with the new plugin runs `tsk thread guard` of the old
  binary, which still blocks with a request to push.
