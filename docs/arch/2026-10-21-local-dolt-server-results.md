# The local Dolt server, measured: 0.23.0 with the switch off and on, 2026-10-21

Written by Claude (Sonnet 5.5) after building the server of
[ADR 0018](../adr/0018-a-local-dolt-server-per-ledger-store-behind-a-switch.md). Dolt 2.5.0,
a release build of `tsk` 0.23.0, and a 0.22.0 release build for the comparison. All figures
come from one machine (4 CPUs, 15 GB) with other work on it at times; the load average was
1 to 3 during the command measurements and is noted for the suites. A difference under 10 %
is not a measured saving.

## Summary

- With the switch on, a command makes no `dolt` process for its SQL. A `mission list`
  takes 15 ms where it took 99 ms, a `task add` 23 ms where it took 225 ms, and a thread
  pause 0.50 s where it took 0.82 s, through a 330 ms network shim.
- The network is the floor. `ledger status` and a session start spend 330 ms in `git
  ls-remote`, so the server saves 0.16 s on the first and nothing on the second.
- The workspace suite takes 275 s with the switch on and 509 s with it off. The
  conformance suite takes 91 s against 271 s, and starts 912 `dolt` processes against 6,213.
- Servers do not pile up: the most that ran at once in a full run was 7, using 727 MB.
- The `dolt` command line cannot be left able to reach a running server. Through the server
  it prints every number as a string, and a 0.22.0 `tsk` that read such an answer wrote
  duplicate rows. The server therefore removes Dolt's record of itself, and `dolt` opens the
  store read only while the server runs. See "What an older `tsk` does".

## Method

- Scratch origin and clones under the session's scratch directory, built with `git init
  --bare`. A ledger with 20 missions of 3 tasks, published, and one thread on `M-01`.
  Nothing touched the live ledger.
- The network cost is a C stand-in for `git` first on `PATH` (the `gw.c` of
  `2026-10-18-remote-growth-design.md`, Setup), which sleeps 330 ms before `fetch`, `push`,
  `ls-remote`, `clone` and `pull`.
- Each command ran 7 times in a row; the table shows the median. A second pass with a
  stand-in for `dolt` that logs its arguments counted the processes. The stand-in is a
  shell script, so the counts and the times come from different passes.
- Switch off: `TSK_DOLT_SERVER=off`. Switch on: `TSK_DOLT_SERVER=on` with a 600 s idle time,
  so the server stayed up between runs. The cold session start stops the server before each
  run; the dead one kills the supervisor and the server with `kill -9` and leaves the
  socket and pid files; the live one finds the server running.
- The suites ran with `TSK_DOLT_SERVER` exported for the whole run (`off` or `on`). The
  fixtures set `TSK_DOLT_SERVER_IDLE_SECS=3`. A sampler wrote the used memory, the number of
  `dolt sql-server` processes and their resident size every 2 s.

## Commands

Median wall time in ms of 7 runs, and the number of `dolt` processes of one run.

| Command | Switch off | Launches | Switch on | Launches |
|---|---|---|---|---|
| `mission list` | 99 | 1 | 15 | 0 |
| `task add` | 225 | 2 | 23 | 0 |
| `ledger status` | 545 | 2 | 381 | 0 |
| `ledger push` with a change | 732 | 3 | 455 | 0 |
| `thread pause` | 823 | 3 | 503 | 0 |
| `thread session-start`, cold (no server) | 394 | 1 | 394 | 1 (the server) |
| `thread session-start`, dead server (`kill -9`) | 390 | 1 | 394 | 1 (the server) |
| `thread session-start`, live server | 389 | 1 | 393 | 0 |

What the rows show:

- Local work gains the whole process start. `mission list` falls from 99 to 15 ms, of which
  the `tsk` start and the `git rev-parse` are most.
- A command that crosses the network gains the `dolt` starts only. `ledger status` and a
  push lose 0.16 and 0.28 s, and a pause loses 0.32 s (it writes, commits, pushes the
  documents and the data).
- A session start has the 330 ms question to origin in the way. The server starts in the
  background during that wait: the cold and dead rows cost the same as the live one,
  within the spread of the runs (385 to 403 ms).
- The start of the server itself (not in the table) takes 110 ms to the first answer, as in
  the spike. The cold `mission list` is 125 (median of 7; 109 to 138) ms.

## Suites

Both runs were `cargo test --workspace --no-fail-fast` with `TSK_DOLT_SERVER` exported
for the whole run and 4 test threads. Each ended with every test passing. The times are the
harness's `finished in` for each binary, in seconds. The machine had other work on it, so a
difference under 10 % is not a saving.

| Test binary | Tests | Switch off | Switch on |
|---|---|---|---|
| unit tests (`src/main.rs`) | 331, 332 | 0.6 | 3.0 |
| `auto_update` | 8 | 4.2 | 4.6 |
| `broken_pipe` | 1 | 0.0 | 0.0 |
| `conformance` | 115 | 270.7 | 91.3 |
| `dolt_server` | 17 | 17.0 | 17.3 |
| `events` | 23 | 26.8 | 19.5 |
| `git_protocol` | 3 | 2.8 | 2.2 |
| `hooks` | 5 | 0.4 | 0.4 |
| `install_plugin` | 17 | 0.1 | 0.2 |
| `ledger` | 35 | 27.4 | 23.1 |
| `ledger_refs` | 24 | 22.1 | 14.4 |
| `migrate_docs` | 9 | 18.4 | 19.0 |
| `nexus` | 25 | 20.5 | 14.3 |
| `nexus_cmd` | 16 | 1.7 | 1.6 |
| `self_update` | 10 | 0.5 | 0.5 |
| `thread` | 62 | 72.2 | 44.4 |
| `transport` | 12 | 23.1 | 16.2 |
| Sum of the binaries | 713, 714 | 508.4 | 271.9 |
| Wall time of the command, builds included | | 509 | 275 |

- The conformance suite is where the time goes: 3.0 times faster. It took 207 to 216 s
  with 101 tests at 0.21.0. This run has 115 tests: the 110 of 0.22.0, the four cases that
  run a script with the server off and on, and the comparison with a baseline binary, which
  returns at once when `TSK_BASELINE_BIN` is not set.
- `migrate_docs` and `dolt_server` do not change. `dolt_server` sets the switch for itself in
  each case, so the exported value does not reach it. The reason for `migrate_docs` was not
  examined.
- The unit tests gain 2.5 s from the timeout case of the client, which waits for a silent
  fake server.
- The first exploratory run of the workspace suite with the switch on failed 18 conformance
  cases, 17 `thread` cases, 10 `events` cases, 8 `auto_update` cases and 1 `nexus` case.
  Each was a test that wrote to the store with `dolt sql` while a server held it. The
  helpers now stop the servers of the fixture before they write (`common::stop_servers`).
  The same failure is what a person meets who writes by hand while a server runs.

### `dolt` processes in the conformance suite

A stand-in for `dolt` first on `PATH` appended one line to a file for each process, for the
whole run, the tests' own `dolt` calls included.

| `dolt` subcommand | Switch off | Switch on |
|---|---|---|
| `sql` | 5,329 | 328 |
| `sql-server` | 7 | 272 |
| `push` | 326 | 13 |
| `fetch` | 87 | 4 |
| `clone` | 87 | 87 |
| `commit` | 60 | 15 |
| `add` | 60 | 15 |
| `reset` | 46 | 0 |
| `merge` | 28 | 1 |
| `clean` | 6 | 0 |
| `version` | 114 | 114 |
| `config` | 39 | 39 |
| `remote` | 12 | 12 |
| `init` | 12 | 12 |
| **All** | **6,213** | **912** |

The suite (115 tests, 4 threads) took 274 s with the stand-in and the switch off, and
87 s with it on.

- The 7 `sql-server` starts with the switch off are the cases that turn the switch on for
  themselves.
- With the switch on, 272 servers started for 115 tests. A server starts for each store
  that a case uses, and a case with two clones has two.
- The 328 `dolt sql` processes that remain come from three places: the tests' own reads and
  writes of the store (`-r csv -q ...` calls), the comparison cases that run each script
  with the switch off, and the creation of a new ledger, which runs on the command line
  before the store has a remote. They were not counted by place.
- `clone`, `init`, `config`, `remote` and `version` are the steps that stay on the command
  line.

## Memory and processes

A sampler took the memory in use (`free -m`), the number of `dolt sql-server` processes
and their resident size every 2 s during each workspace run.

| Run | Samples | Peak memory in use | Peak servers at once | Resident size of those servers |
|---|---|---|---|---|
| Switch off | 249 | 1,156 MB | 4 | 373 MB |
| Switch on | 135 | 1,217 MB | 7 | 727 MB |

- The 4 servers of the switch-off run belong to the cases that turn the switch on for
  themselves. Memory in use rose 429 MB above the first sample in the switch-off run and
  240 MB in the switch-on run. The figures are one run each and the machine had other
  work on it, so they do not rank the two.
- A server holds about 104 MB resident (727 MB for 7). That is the cost of a server per
  store. The 3 s idle time and the stop in each fixture's `Drop` keep the count at 7 for 4
  test threads, not at 115.
- A server that outlives its test is stopped by its supervisor, which stops the server when
  the store is gone. No `dolt sql-server` was left after the last run.

## What an older `tsk` does

Measured with a 0.22.0 release build against a ledger that a 0.23.0 `tsk` had written and
whose server was running.

| Step | With Dolt's record of the server left in place | With it removed (as shipped) |
|---|---|---|
| `mission list` | correct | correct |
| `mission list M-A --json`, `task list M-A` | tasks `T-00` and `T-00` (the sequence numbers read as 0) | `T-01`, `T-02` |
| `ledger status` | `Origin  not published yet`, for a ledger that is published | `Origin  up to date` |
| `task add M-A "three"` | printed `T-01` and rewrote the table: task `one` was lost and `two` appeared twice | `error: dolt sql failed (exit status 1)`, `cannot update manifest: database is read only`, nothing written |
| `ledger push` | `note: nothing to commit`, though the add had changed rows | `error: dolt add -A failed (exit status 1)`, `cannot update manifest: database is read only` |
| `thread start M-A` | `error: refs/heads/tsk/ledger on origin holds no Dolt branch 'main'; it is not a Dolt ledger` | `error: dolt sql failed (exit status 1)`, `database is read only` |

The cause is in Dolt. When a server runs, the command line finds `.dolt/sql-server.info`
and hands the query to the server. That answer prints every column as a string, so `"c":"1"`
replaces `"c":1`, and 0.22.0 reads counts and sequence numbers with `as_i64`. Without the
file the command line opens the store itself, and a store that a server holds for writing
opens read only. Reads print numbers, and a write fails with an error that stops the older
`tsk`. The older `tsk` works again as soon as the server stops, which happens at most 120 s
after the last command of 0.23.0, or at once with `tsk ledger server stop`.

The 0.23.0 `tsk` reads numbers from strings as well (`integer`, `flag` and `first_count`),
and with the switch off it stops the server of a store when its first write is refused.

## What did not move to the server

| Step | Why |
|---|---|
| `dolt init`, `clone`, `remote add`, `remote remove`, `config` | Rare, run before a server exists or while it is stopped, and `remote` needs a write. |
| Everything on a store whose `origin` is Dolt's git blobstore | `dolt fetch` and `dolt push` start git. The server would start it with the environment of the command that started the server. Run `tsk ledger migrate-transport` first. |
| `dolt gc` | `tsk` does not run it. |

`fetch`, `push`, `reset --hard`, `clean`, `add -A`, `commit -m`, `merge`, `merge
--no-commit` and `merge --abort` run as procedures. Two checks showed they behave as the
commands do: the error text of a rejected `dolt_push` is the text of `dolt push` (the retry
loop reads `rejected`, `behind` and `non-fast-forward` in it), and a merge run with
`dolt_allow_commit_conflicts` and `dolt_force_transaction_commit` set leaves a conflict in
`dolt_conflicts`, so the code that reads the table and aborts needed no change. The
conformance cases for conflicts, renumbering and the merge fixup pass with the server on.

## What was not verified

- A server under a Claude Code cloud session that hibernates. The tests kill the processes
  and leave the files, which is what a stopped container leaves; no cloud session was used.
- macOS. The code has paths for it (`ps` for the command line of a process, `SO_NOSIGPIPE`,
  a 100 byte socket path limit) and none ran.
- A mid-statement hang of a real server. The limit is the socket read timeout of the
  client, which a unit test checks against a fake server; the end-to-end case that the
  tests run is a hung server at the first check.
- A store of the size of the live ledger. The measurements use 20 missions.
