# Publish by rule: which commands push, before and after 0.24.0

Written by Claude (Sonnet 5.5) for M-PERF T-08, on 2026-10-21. The rule is
[ADR 0019](../adr/0019-publish-less-often-by-rule.md). Dolt 2.5.0, release builds of `tsk`
0.23.0 and 0.24.0, one machine (4 CPUs). A difference under 10 % is not a measured saving.

## Summary

- Before 0.24.0 four thread commands pushed after a binding or a mission change:
  `resume`, `detach` and `set mission`, besides the three that must (`start`, `pause`,
  `stop`). After 0.24.0 only `start`, `pause`, `stop`, `tsk ledger push` and the event
  queue commands push.
- With a 330 ms network shim, `resume` takes 0.55 s where it took 1.18 s, `detach` 0.48 s
  where it took 1.10 s, and `thread set mission` 0.48 s where it took 1.11 s.
- What remains in those 0.5 s is the refresh: one `git ls-remote` (330 ms) and the local
  Dolt reads and writes. The commands still ask origin whether it moved.
- The publish moved to four points: thread start, thread pause or stop, the end of the
  session, and when asked. The end of the session has two hooks.

## Audit: every `tsk` command that pushes

"Pushes" means the command sends the ledger to origin (`git push` of the ledger or the
documents ref, through the code of `tsk ledger push`). Taken from `cli/src` at 0.23.0 and
0.24.0: the callers of `push_reporting` and `Session::push`.

| Command | Pushes now (0.23.0) | Pushes after (0.24.0) |
|---|---|---|
| `tsk ledger push` | yes | yes (when asked) |
| `tsk thread start` | yes | yes (the claim and the binding) |
| `tsk thread pause` | yes | yes (the hand-off) |
| `tsk thread stop` | yes | yes |
| `tsk thread resume` | yes, when the binding changes | no |
| `tsk thread detach` | yes | no |
| `tsk thread set mission` | yes | no |
| `tsk thread skip` | no (a file in the git directory) | no |
| `tsk thread prune-bindings` | no | no |
| `tsk thread list`, `binding`, `session-start`, `guard` | no (they fetch or read) | no |
| `tsk mission ...`, `tsk task ...`, `tsk decision ...`, `tsk question ...` | no | no |
| `tsk ledger doc put`, `rm`, `get`, `list`, `path` | no | no |
| `tsk ledger fetch`, `status`, `discard` | no | no |
| `tsk ledger migrate-docs` | no (commits locally) | no |
| `tsk ledger migrate-transport` | yes, once | yes, once |
| `tsk events append`, `append-batch`, `advance-watermark` | yes | yes (see below) |
| `tsk nexus register-repo` | pushes to the nexus repo, not the ledger | unchanged |
| `SessionEnd` hook (`plugin/hooks/session-end.sh`) | does not exist | yes, only when the store holds unpushed work |
| `Stop` hook (`tsk thread guard`) | no | no. It asks the agent to push |

The event queue keeps pushing. An event is a message for another process on another
machine, not progress on a task. The consumer reads the queue from origin, so a delay
would delay the delivery.

`tsk ledger push` also runs inside `thread start`, `pause` and `stop`. Each of them
commits every local change, so the records written since the last publish go out with it.

## The rule

1. A record write changes the local ledger store and never pushes.
2. Records are published at thread start, thread pause or stop, the end of the session,
   and when asked.
3. A stale read is acceptable. A container reclaimed before a publish loses the records
   written since the last one.

### What counts as a claim

The mission says "claims must be prompt so two actors do not take the same task". The
code and the skills give two writes that stake a claim:

| Write | How it publishes |
|---|---|
| `tsk thread start` | The command publishes by itself. The thread, the binding and every earlier local record go out at once. |
| A task or a mission set to `in-progress` to take it (`tsk task set <mission> <task> status in-progress`) | The command is a record write and does not push. The skills (`start-thread`, `tsk`) and the import guide tell the agent to run `tsk ledger push "Claim <mission> <task>"` straight after. |

The code has no marker that tells a claim from other progress, and a flag on `task set`
would make a record write push. The rule therefore lives in the skills. Every other write
is progress and waits: `done`, a task added, a decision recorded, a question asked or
answered, a mission field changed, a document written, a binding changed.

`thread resume` is not a claim. A resume does not stake a task. It binds a session to a
thread that another actor paused, and the takeover warning reads the continuation entries
that `pause` published, not the binding rows.

## Hooks

Claude Code supports the `SessionEnd` event. The plugin already used `SessionStart` and
`Stop` in `plugin/hooks/hooks.json`, and `docs/kb/claude-code-mods.md` lists `SessionEnd`
among the settings hooks. GitHub Copilot CLI has a `sessionEnd` event. The plugin
registers both.

- **`plugin/hooks/session-end.sh`.** It reads and discards the hook input, then runs
  `tsk ledger status --local`, which asks origin nothing. When the output has `not
  pushed` (changes or commits), it runs
  `tsk ledger push "End of session"` under `timeout` (8 s, set by
  `TSK_SESSION_END_TIMEOUT_SECS`). It prints nothing and always exits 0. A missing `tsk`,
  a failed status, a failed push and a slow origin all end quietly. The hook is
  registered with a 12 s timeout, so a hung push cannot hold the session open. A clean
  store costs the status call (0.18 s) and no network.
- **The `Stop` hook** (`tsk thread guard`). The unbound check is unchanged and comes
  first. When it passes, the command reads the local store as `tsk ledger status --local` does,
  and blocks with a request for `tsk ledger push` only when the store holds
  changes or unpushed commits. It does not block when the hook input has
  `"stop_hook_active": true`, so a push that fails cannot hold a session open. It never
  asks origin.

The `SessionEnd` hook is best effort. It does not run when the container is killed, and
that is the lost-work window the ADR names. The `Stop` hook runs at the end of every
turn, so with unpushed work it asks once at the end of each turn. An agent that follows it
pushes at the end of every turn that wrote records, which is the point of the rule's
fourth publish point and costs one extra turn.

## Timings

Method, as in `2026-10-21-local-dolt-server-results.md`: a bare scratch origin and a
clone, a ledger with 20 missions of 3 tasks, one thread on `M-01`, a cloud session ID, the
Dolt server off, and a C stand-in for `git` first on `PATH` that sleeps 330 ms before
`fetch`, `push`, `ls-remote`, `clone` and `pull` (the `gw.c` of
`2026-10-18-remote-growth-design.md`, Setup). Each command ran 7 times, with the publish
or the opposite command outside the timing. The table shows the median in ms. `set
mission` alternates `--none` and `M-01`, so it has 14 runs. The 0.23.0 binary was a
release build of `main` before this change.

| Command | 0.23.0 | 0.24.0 | Where the rest goes |
|---|---|---|---|
| `thread resume` | 1184 | 554 | `git ls-remote` 330 ms, the Dolt reads and the binding write |
| `thread detach` | 1101 | 476 | the same |
| `thread set mission` | 1111 | 477 | the same |
| `task add` (reference) | 190 | 216 | no network |
| `ledger push` with a change (reference) | 734 | 733 | unchanged |
| `thread pause` (reference) | 775 | 791 | unchanged |

Raw runs, 0.23.0 then 0.24.0:

- `resume`: 1206 1181 1161 1184 1168 1192 1197, and 621 544 554 574 546 552 572.
- `detach`: 1080 1090 1071 1167 1101 1128 1142, and 485 470 479 473 476 471 507.
- `set mission`: 1060 to 1172 over 14 runs, and 462 to 493 over 14 runs.

The saved 0.6 s is the push that the three commands no longer make: the same cost as
`ledger push` with a change. The remaining 0.48 s is the refresh. A command that cannot
tolerate a stale read needs it, and `detach` and `set mission` could skip it when the
store exists locally. That change is not in 0.24.0, because the refresh also sets up the
store on a fresh clone.

The saving moves the cost to the next publish. A session that resumes, detaches and sets
a mission once each saves 1.8 s of publishes and pays one publish at the next point, which
carries all three. A session that does only record writes already paid nothing, and
now the skills no longer tell it to push after each write.

## What the tests show

- `cli/tests/thread.rs`: `resume` (worktree and cloud), `detach` (worktree and cloud) and
  `set mission` leave origin's ledger ref where it was, write the row in the local store,
  show `not pushed` in `ledger status --local`, and publish with the next `ledger push`.
  `start` publishes the claim and a second clone sees it after its fetch. `pause` and
  `stop` publish with the records written before them. The Stop hook asks for a push
  only with unpushed work, not when `stop_hook_active` is set, and with origin away.
- `cli/tests/hooks.rs`: the `SessionEnd` script with a stand-in `tsk`: it pushes for
  changes and for a commit ahead, does not push a clean store, and exits 0 when the status
  fails, the push fails or the push hangs.
- `cli/tests/conformance.rs`: the existing cases that relied on `resume`, `detach` and
  `set mission` pushing now publish with an explicit `ledger push`, and one case checks
  the new rule against a second clone.

Full workspace suite with Dolt 2.5.0 first on `PATH`: 724 tests, 0 failures, with
`TSK_DOLT_SERVER=off` and with `TSK_DOLT_SERVER=on`. `cargo fmt --check` and `cargo clippy
--workspace --all-targets` report nothing.

## A consequence found in the tests

A clone that holds unpushed changes does not merge origin's state when a thread command or
a session start refreshes. It keeps the local changes as they are, as it did before. Before
0.24.0 a `resume` or `detach` pushed at once, so that state was rare. Now it is common:
after a `resume` the clone is dirty until the next publish point. Two effects:

- `tsk ledger fetch` and the event queue reads refuse a dirty store. Run `tsk ledger push`
  first.
- `tsk thread list` in a dirty clone shows origin as the clone last fetched it. The
  conformance cases that read another clone's change publish first.

A refresh that merges origin into a dirty store would remove both effects. It is a larger
change than T-08, and the ADR records the stale read as accepted.
