# M-PERF results: fewer round trips and processes, 2026-10-19

Tasks T-01, T-09, T-02, T-03, T-10 and T-04 of mission M-PERF. Written by Claude
(Sonnet 5.5). The numbers come from scratch ledgers. Nothing was written to the live
ledger and nothing was pushed to the live origin.

## Summary

- A command that finds nothing new on origin now makes one network operation, a
  `git ls-remote` that names both refs, and starts no fetch. `tsk ledger fetch` falls
  from 1.06 s to 0.16 s on a local origin and from 2.12 s to 0.51 s with 330 ms per
  network operation.
- A thread command reads in one `dolt` process and writes in one transaction. A thread
  start launches 6 `dolt` processes where it launched 19, a pause 5 where it launched 20,
  and `tsk thread list` 2 where it launched 38.
- `tsk ledger status --local` asks origin nothing and takes 0.29 s, in 2 `dolt`
  processes.
- Session start takes 0.18 s on a local origin and 0.53 s with 330 ms per network
  operation. The objective of 1.5 s on the live ledger is met on these figures.
- A thread command takes 1.3 to 1.5 s on a local origin and 3.5 s with 330 ms per
  operation. The objective of 1 s is not met. A write that pushes spends 0.75 s in
  `dolt push`, which Dolt runs as 3 fetches and 2 pushes and about 330 `git` processes.
  Only a smaller remote (T-05, T-06) reduces that.
- Output is the same, byte for byte, on a script of 90 commands, once ids, commit
  hashes, times and paths are masked (see Output comparison).

## What changed

| Task | Change |
|---|---|
| T-01 | The store keeps what its first fetch found. A push uses the fetched documents tip as its lease and the fetched Dolt data as its base. Origin rejects a stale push, and the retry fetches. |
| T-09 | One `git ls-remote` names the commits of the ledger ref and the documents ref. A small file in the clone's state directory (`remote-tips`) holds the commits last fetched. When both match, both fetches are skipped. `tsk ledger status --local` asks nothing. |
| T-02 | The refresh of a thread command reads a snapshot in one process (manifest, missions, bindings, threads, head, dirty). The commit stages, counts, lists links and commits in one process. The identity check reads the Dolt configuration file instead of starting `dolt`. |
| T-03 | `thread pause` reads the remote-tracking branches first and fetches the code repository only when none holds the commit. |
| T-10 | `thread list` reads every thread's lines in one query. |
| T-04 | `TSK_NO_BATCH=1` makes every batched call fail at once, so each takes its older path. Four conformance cases compare the output of the two paths. |

## Method

- The old binary is tsk 0.17.0 built from `main` at 643524d. The new binary is built from
  the last commit of this work. Both ran with Dolt 2.5.0.
- Each run uses a scratch world: a bare origin seeded with copies of this repo's
  `refs/heads/tsk/ledger` and `refs/heads/tsk/docs` from GitHub (125 table files at the
  time), a code clone, and its own `XDG_STATE_HOME`, `XDG_CONFIG_HOME` and `HOME`. A
  cloud session ID is set, so thread commands bind a session.
- "Local" runs the commands with no stand-in. "330 ms" puts a `git` stand-in first on
  `PATH`. It sleeps 0.33 s before `fetch`, `ls-remote`, `push` and `clone`, logs every
  `git` and `dolt` launch, and runs the installed program. The `git` processes that
  `dolt` starts find it too, so Dolt's own fetches and pushes pay the delay. The design
  note `2026-10-18-remote-growth-design.md` found that 330 ms reproduced the total
  measured against GitHub within 1 %.
- Each cell is the median of 3 runs. The spread is under 0.1 s, except for the thread
  writes and `ledger push` with a change, where it is up to 0.4 s.
- Launch and network counts come from the log of the 330 ms run. A network operation is a
  `git` run whose verb is `fetch`, `ls-remote`, `push`, `clone` or `pull`, including those
  that `dolt` starts.
- The commands run in this order in one ledger: `mission list`, `task add`,
  `task set ... status done`, `ledger status` (with unpushed changes), `ledger status --local`,
  `ledger push` (a change), `ledger fetch`, `ledger push` (nothing to do), `thread start`,
  `thread pause`, `thread detach` (not measured), `thread resume`, `thread list`,
  `thread session-start`, `thread guard`. The old binary has no `--local`.

## Results

Counts read "before > after". Network operations show the total, then the split of the
after count as ls-remote, fetch and push.

| Command | Local before (s) | Local after (s) | 330 ms before (s) | 330 ms after (s) | dolt launches | git launches | network ops |
|---|---|---|---|---|---|---|---|
| `mission list` | 0.16 | 0.13 | 0.13 | 0.12 | 1 > 1 | 1 > 1 | 0 > 0 |
| `task add` | 0.29 | 0.31 | 0.27 | 0.31 | 2 > 2 | 1 > 1 | 0 > 0 |
| `task set status done` | 0.27 | 0.29 | 0.30 | 0.32 | 2 > 2 | 1 > 1 | 0 > 0 |
| `ledger status` | 0.97 | 0.28 | 1.76 | 0.60 | 5 > 2 | 282 > 7 | 2 > 1 (1/0/0) |
| `ledger status --local` | n/a | 0.29 | n/a | 0.27 | n/a > 2 | n/a > 6 | n/a > 0 |
| `ledger push` (a change) | 1.95 | 1.23 | 4.81 | 3.36 | 9 > 4 | 606 > 332 | 7 > 6 (1/3/2) |
| `ledger fetch` (nothing new) | 1.06 | 0.16 | 2.12 | 0.51 | 6 > 1 | 289 > 9 | 3 > 1 (1/0/0) |
| `ledger push` (nothing to do) | 1.03 | 0.42 | 1.83 | 0.76 | 6 > 3 | 286 > 7 | 2 > 1 (1/0/0) |
| `thread start` | 3.67 | 1.52 | 7.39 | 3.63 | 19 > 6 | 904 > 344 | 10 > 6 (1/3/2) |
| `thread pause` | 3.81 | 1.28 | 7.94 | 3.47 | 20 > 5 | 920 > 351 | 11 > 6 (1/3/2) |
| `thread resume` (binds) | 3.29 | 1.43 | 7.14 | 3.56 | 17 > 6 | 940 > 356 | 10 > 6 (1/3/2) |
| `thread list` | 4.89 | 0.29 | 6.41 | 0.65 | 38 > 2 | 307 > 11 | 3 > 1 (1/0/0) |
| `thread session-start` | 1.45 | 0.18 | 2.62 | 0.53 | 10 > 1 | 308 > 12 | 3 > 1 (1/0/0) |
| `thread guard` | 0.13 | 0.13 | 0.15 | 0.15 | 1 > 1 | 3 > 3 | 0 > 0 |

## Reading the table

- The record commands (`mission list`, `task add`, `task set`) and `thread guard` are
  unchanged. They never asked origin, and each already used one or two `dolt` processes.
  `task add` and `task set` sit at 0.3 s, on the budget.
- Every command that only compares with origin now costs the `ls-remote`: 0.16 s local
  and 0.5 s at 330 ms. The 330 ms figure is the ls-remote (0.33 s) plus the local work.
- The thread writes and `ledger push` with a change still pay Dolt's push. In the
  330 ms run it makes 3 fetches and 2 pushes of its own (5 operations, 1.65 s of delay),
  and about 330 `git` processes read the table files of the remote. Local, it takes
  0.71 to 0.84 s. The other 5 `dolt` processes of a thread start take about 0.4 s. The
  `ls-remote` that starts each command adds 0.33 s at 330 ms.
- `tsk` starts a `git push` of its own only when documents changed. The 2 pushes in the
  `ledger push` and thread rows are Dolt's.
- A push is followed by a command that finds nothing new, because the push records the
  commit it made (see Risks).

## Budget

The action budget in `missions/operational/M-PERF/action-budget.md` against these figures,
local origin, then 330 ms per operation.

| Action | Budget | Now |
|---|---|---|
| Read what is open | 0.2 s | 0.13 s. Met. |
| Add a task, mark one done | 0.3 s | 0.31 s and 0.29 s. Met within the noise. |
| See what is unpublished | 0.3 s with a local-only mode | `status --local` 0.29 s. `status` 0.28 s local, 0.60 s at 330 ms. |
| Publish changes | 1 s, or in the background | 1.23 s with a change, 0.42 s with none. |
| Start, pause, resume a thread | 1 s | 1.5, 1.3 and 1.4 s. Not met. `dolt push` is 0.75 s of it. |
| Start a session | 1.5 s | 0.18 s local, 0.53 s at 330 ms. Met. |
| Stop hook, every turn | 0.15 s | 0.13 s. Met. |
| List threads | 0.3 s as a local read | 0.29 s local, 0.65 s at 330 ms. It still asks origin. |

Update for 0.22.0 (`2026-10-20-session-start-and-pause.md`), at 330 ms per operation on a
machine with a load average of 9 to 10: a pause 0.88 s (1.42 s with the 0.21.0 binary in
the same run), a push with a change 0.82 s (1.24 s), a push with nothing 0.27 s (0.70 s),
a resume 0.46 s (0.61 s), a session start 0.39 s with nothing new, 1.16 s when origin moved
and 0.88 s in a new clone (1.55 s). The pause and publish budgets are met. A pause after
another clone pushed is 2.4 s and is not.

## Output comparison

`compare.py` ran one script of 90 commands against the old and the new binary,
each on its own scratch ledger seeded from the same refs, and compared stdout, stderr and
the exit code of every command after masking the fixture path, clone ids, thread ids,
Dolt and git commit ids and UTC timestamps. 90 commands, 0 differ.

The script covers: fetch, status (plain and `--porcelain`), record reads (`mission list`
plain, `--porcelain` and `--json`, `task list`, `decision list`, `question list`), record
writes, a document and a link, push (a change, nothing to do), a second clone that
fetches and reads, `thread start` (twice, so the second stops with exit 2), `binding`, `pause` (with
and without a task that exists), `list`, `resume`, `set mission`, `session-start`,
`guard`, `discard` (listing and `--yes`), `stop`; a stale push whose merge conflicts in
the task table, a stale push whose merge conflicts in a field, a push stopped by a linked
document that is gone, and then origin moved away so that `status`, `status --porcelain`,
`fetch`, `thread list`, `push`, `session-start` and `discard --yes` fail the same way.

The four `TSK_NO_BATCH` cases in `cli/tests/conformance.rs` compare the batched run with
the query-by-query run on the same kinds of command inside the test suite.

## Tests added

- A second clone pushes both refs between a thread command's refresh and its push. Both
  pushes are rejected once, merged, and neither side loses a change (T-01).
- A logging `git` stand-in counts network operations: a refresh with nothing new makes
  one `ls-remote` naming both refs and no fetch; a change on origin is fetched once; a
  push is followed by a quiet fetch; `status --local` starts none (T-09).
- A logging `dolt` stand-in counts processes: a thread start at most 6, a pause 5, a
  resume that writes nothing 2, `thread list` 3 (T-02, T-10).
- A pause fetches the code repository when no tracking ref holds the commit, and does
  not when one does (T-03).
- `records_...`, `fetch_and_status_...`, `push_and_merge_...` and `thread_commands_...`
  `_read_the_same_without_batching` (T-04).

## Deviations and risks

- **T-03 accepts a stale tracking ref.** A branch deleted on origin, or rewritten so that
  it no longer holds the commit, keeps its tracking ref until a fetch runs. A pause in that
  window passes. Before, the pause fetched with `--prune` every time. The test for a
  deleted branch now fetches with `--prune` first, and a new test pins the new behaviour.
- **The record of the last fetch relies on one Dolt internal.** After a successful push,
  the commit that `dolt push` made is read from Dolt's cache of the remote
  (`refs/dolt/remotes/origin/heads/tsk/ledger/last`, the fetch anchor). Dolt 2.5.0
  writes it. If a Dolt release stops writing it, the record is cleared, the next command
  fetches once, and only the speed is lost. The conformance case that requires a quiet
  fetch after a push fails and shows it.
- **A record newer than the fetch would skip a fetch that is due.** The code never writes
  one: the Dolt record holds the head named before the fetch ran, the documents record holds
  the fetched commit, and a pushed commit is recorded only after the push succeeded.
  A store replaced by hand while `remote-tips` stays would defeat this. The commands that
  read the store then find no remote branch and fetch for real before they report an error.
- **`dolt_commit` runs in SQL.** The commit that follows staging is a SQL call in the
  same process, with `--skip-empty`. The author is the same global Dolt identity. The race
  test therefore acts before the first `git push`, not at the commit.
- **The identity check reads `config_global.json`.** It looks under `DOLT_ROOT_PATH`, or
  under `HOME`, and runs `dolt config` when the file names no user. A machine whose Dolt
  reads another directory is not served by the shortcut and takes the process.
- **`status --local --porcelain`** prints `not checked` on the two remote lines, where
  `status --porcelain` prints a state such as `up to date`. The human form prints
  `Origin  not checked (--local)`. This is new output, not a change.
- **Ledgers below layout 8** read through the older paths. The batches fail on a missing
  table and the older code runs, as in 0.17.0.
- **Nexus ledgers** use the same code. The conformance case for a nexus passes. The
  comparison script did not run against one.

## Not verified

- The live origin. All figures are scratch ledgers with a local origin and a stand-in that
  adds 330 ms. The stand-in reproduced the live total within 1 % in the earlier note.
- Dolt 1.88.2. The work ran on 2.5.0 only. The fetch anchor above is a 2.5.0 behaviour.
- A `.darc`-only remote tree, or a remote that Dolt re-roots at 64 commits. The scratch
  remote grew by a few commits during the work.
- Windows.
