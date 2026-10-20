# Conformance suite speed: where the time goes and what to change, 2026-10-20

Written by Claude (Fable 5.1), read-only analysis of `main` at 0.21.0. Nothing here is
built, and nothing was written to the repo tree, the live ledger or git. The scratch copies
and timing logs were under the session's scratch directory and are removed.

## Summary

- `cli/tests/conformance.rs` takes 207 to 216 s on this machine (101 tests, 4 test
  threads). Its serial time is 541 s. The machine is the limit: with 4 threads the CPU is
  86 % busy, a third of it in the kernel, and 4 threads give 2.6 times the serial speed,
  not 4.
- The time is in `dolt` process starts. The serial run starts 5,958 `dolt` and 10,405
  `git` processes. A `dolt sql` costs 60 to 90 ms on an idle machine and a `git` command 4
  to 9 ms, so `dolt` is about 400 of the 541 serial seconds and `git` about 50.
- Five tests that compare the batched and the unbatched read paths
  (`*_read_the_same_without_batching`) cost 152 of the 541 serial seconds (28 %), because
  each runs its script twice and the unbatched run starts 10 to 12 `dolt` processes per
  command instead of 1 to 3.
- The fixture is cheap and already shared: `Fixture::cached` builds 8 templates once per
  process and copies one per test (34 ms). The git part of `Fixture::new` costs 70 ms. A
  new ledger's first `tsk ledger fetch` costs 0.8 s, and 10 tests pay it on purpose.
- Measured changes: a shared seeded fixture for the unbatched tests takes 20 % off the
  three tests that use it, about 7 s of wall time; 8 test threads save nothing on 4 CPUs;
  running the six heaviest tests first gave no measurable gain over the noise.
- The workspace run (615 s under load) is serial across test binaries. `thread.rs` builds
  and publishes a ledger 62 times with no cache: the largest single saving after the
  five unbatched tests.
- `cli/tests/auto_update.rs` fails at random with `Text file busy`. It happened once in
  this session's three workspace runs. The fix is the retry that `self_update.rs` already
  has, or a hard link in place of the 48 MB copy.

## Method and caveats

- Dolt 2.5.0 was first on `PATH` for every run. Other agents used the machine: the load
  average was 1 to 5 during the baseline runs and 8 to 22 during the workspace and
  experiment runs. Wall times are medians of 2 runs where noted. A difference under 10 % is
  not a measured saving.
- Per-test times come from `--report-time` (`RUSTC_BOOTSTRAP=1` and `-Z unstable-options`
  on stable 1.97). Serial times come from one run with `--test-threads=1`.
- Process counts come from a C shim first on `PATH` for `git` and `dolt` (as in
  `docs/arch/2026-10-18-remote-growth-design.md`, Setup). It appends the time, name, pid,
  parent pid, directory and arguments to a log, then `execv`s the installed binary. Launches are
  attributed to the test whose serial time window holds them.
- The two measured changes were built in a scratch copy of the workspace and run from the
  binaries, interleaved with the unchanged binary, in two rounds.

## Where the time goes

### The suite

| Run | Wall time | Note |
|---|---|---|
| `cargo test --test conformance`, 4 threads, runs 1 and 2 | 216 s, 209 s | harness `finished in` 205.8 s and 209.3 s; median 212 s |
| Same, `--test-threads=1` | 541 s | one run, with the shim on PATH |
| Ideal with 4 threads and no contention (serial sum / 4) | 135 s | simulated |
| Alphabetical order on 4 workers, no contention | 149 s | simulated from the serial times: the tail of the alphabetical order costs 14 s |
| `cargo test --workspace --no-fail-fast` | 615 s | one run, load average 8 to 14 from other agents; conformance was 315 s of it |

CPU use during the 4-thread run, sampled four times over 12 s: 85 to 86 % busy, 45 to
48 % user, 30 to 33 % system, 4 to 5 % I/O wait, 4 `dolt` processes alive at each sample.
The machine has 4 CPUs and 15 GB of memory. The harness uses 4 threads (the CPU count).
The 30 % system time is process creation and page-cache work for a 100 MB Go binary that
starts 60 times a second.

### Per test

Serial time and launches from the one-thread run; parallel time is the median of the two
4-thread runs. The parallel figure is larger because four tests share the CPUs.

| Test | Serial s | Parallel s | `dolt` | `git` |
|---|---|---|---|---|
| `thread_commands_read_the_same_without_batching` | 42.0 | 51.5 | 539 | 506 |
| `every_kind_of_record_is_renumbered_at_the_second_push` | 36.7 | 52.8 | 386 | 576 |
| `push_and_merge_read_the_same_without_batching` | 29.8 | 43.6 | 369 | 252 |
| `fetch_and_status_read_the_same_without_batching` | 26.4 | 39.5 | 335 | 266 |
| `records_read_the_same_without_batching` | 17.4 | 25.9 | 238 | 38 |
| `a_version_four_ledger_reads_threads_and_upgrades_at_the_first_thread_start` | 14.7 | 23.4 | 179 | 222 |
| `thread_set_mission_attaches_changes_and_detaches_a_mission` | 10.8 | 14.2 | 106 | 395 |
| `a_version_six_ledger_reads_its_text_as_lists_and_the_first_write_moves_it` | 10.2 | 17.0 | 141 | 81 |
| Other 93 tests | 353.3 | 504.8 | 3,665 | 8,069 |
| All 101 | 541.3 | 772.8 | 5,958 | 10,405 |

The median test takes 4.1 s serial; 25 take under 2 s, 38 take 2 to 5 s, 30 take 5 to
10 s, and 8 take over 10 s. The top five are 28 % of the serial time, the top ten 38 %.

### Per process launch and per command

Costs of one launch on the machine at load average 1, median of 3:

| Launch | Time | Count in the serial run |
|---|---|---|
| `dolt sql -r json -q <one query>` | 63 to 92 ms | 5,061 `dolt sql` |
| `dolt version` | 55 to 61 ms | 199 (101 from `dolt_installed()`, one per test) |
| `dolt push`, `fetch`, `clone`, `commit`, `add`, `reset`, `merge` | more than `sql` | 261, 76, 74, 55, 55, 89, 24 |
| `dolt config` | about 60 ms | 36 (`ensure_identity`, once per fresh home) |
| `git rev-parse` | 4 ms | 4,059 |

`dolt version` and `dolt sql` cost about the same, so nearly all of a `dolt` launch is
process start. Of the 10,405 `git` launches, 1,382 came from `dolt` and 8,875 from `tsk`
or the test. At 4 to 9 ms each, `git` is about 50 serial seconds; `dolt` is about 400.

Launches per `tsk` command, counted with the shim on a ledger with a few records:

| Command | `dolt` | `git` | Time (load average 8) |
|---|---|---|---|
| `ledger fetch`, new ledger | 9 | 17 | 0.97 s (0.81 s at load 1) |
| `ledger fetch`, nothing new | 1 | 7 | 0.15 s |
| `mission create`, `task add`, `objective add` | 2 | 1 | 0.22 to 0.25 s |
| `mission list`, `task list`, `mission list --json` | 1 | 1 | 0.10 s |
| `ledger push` with changes | 6 | 14 | 0.79 s (0.41 s at load 1) |
| `thread start` | 6 | 18 | 0.84 s |
| With `TSK_NO_BATCH=1`: `mission list`, `task add`, `ledger status`, `thread list` | 11, 12, 9, 10 | | 0.8 to 1.3 s |

Batching already holds most commands at 1 to 3 `dolt` launches. The unbatched path, which
the five comparison tests run on purpose, is 4 to 10 times that.

### Fixture cost

| Step | Cost | How often |
|---|---|---|
| `Fixture::new`: two `git init`, a commit, a push, two clones (7 `git`) | 70 ms | 10 tests directly, plus 8 template builds |
| First `tsk ledger fetch` of a new ledger (`dolt init`, schema, commit, remote) | 0.8 s | the same 10 tests and 8 template builds |
| `with_briefing` template: the above plus a `doc put` and a push | 1.5 s | once per process; 31 tests copy it |
| `version-two` to `version-seven` templates | 1 to 4 s each | once per process; 19 tests copy them |
| Copy of a template (`cp -a`, 1.4 MB, about 200 files) and `rewrite_paths` | 34 ms plus a read of every file | once per test that uses a template |
| `numbered_fixture`: a `fetched` copy plus 8 record commands, a push and a fetch in the second clone | about 2.5 s | 8 tests, and 6 times inside `every_kind_of_record_is_renumbered_at_the_second_push` |
| `seed_records` (10 commands) inside `observe` | about 2 s | 10 times: twice in each of the 5 unbatched tests |

So the prebuilt-and-copied fixture exists already (`Fixture::cached`, line 94). What is
not shared is the set-up tests do after the copy (`numbered_fixture`, `seed_records`).

### Work that could be skipped in test mode

- The update check is off (`TSK_NO_UPDATE_CHECK=1` in `Fixture::command`), and the update
  only runs on an `unsupported ledger version` error. Nothing to save.
- Nexus work: a command on a ledger with no nexus starts no nexus process (`mission list`
  is 1 `dolt` and 1 `git`). Nothing to save.
- `dolt config`: `ensure_identity` reads `config_global.json` first; 36 launches in the
  run, about 2 serial seconds. A shared config would save under 1 s of wall time.
- `dolt version`: `dolt_installed()` runs once per test, 101 launches, about 6 serial
  seconds and 1.5 s of wall time. A `OnceLock` makes it one launch.

## Changes, ranked

Savings are wall time on this machine with 4 threads, from a 212 s baseline. "Measured"
means built and run in the scratch copy; the rest are estimated from the serial times and
launch costs above.

| Rank | Change | Saving | Effort | Risk to coverage |
|---|---|---|---|---|
| 1 | Share one seeded fixture across the unbatched tests: build `seed_records` once (batched) into a cached template, and run only the tail of each script twice. `records_read_the_same_without_batching` keeps the seed, so the seed commands are still compared once. | 7 to 8 s (measured: 20 % off the three tests) | small, test file only | none: the seed's batched and unbatched outputs are still compared in one test |
| 2 | Cache `thread.rs`'s fixture the way conformance does (`Fixture::cached`). 62 tests each build and publish a ledger: about 1.5 s idle, 2 to 3 s under load. | 40 to 60 s of the workspace run | small | none |
| 3 | Run the heaviest tests first. libtest runs tests in name order and the five unbatched tests sort late, so the tail runs with idle threads. Renaming them to sort first (or splitting `every_kind_of_record_is_renumbered_at_the_second_push` into six tests, one per kind) fills the schedule. | not separable from noise (measured with change 1); 14 s simulated | tiny | none |
| 4 | Cache `numbered_fixture` as a template (8 tests, plus 6 builds inside one test). | about 8 s | small | none |
| 5 | Run the test binaries concurrently (`cargo nextest`, or a script that starts the slowest two first). The CPUs are already 86 % busy during conformance, so the gain is the light binaries and the tails. | 30 to 60 s of the workspace run | small, a dev dependency or a `just` recipe | none |
| 6 | `dolt_installed()` once per process. | 1 to 2 s | tiny | none |
| 7 | More test threads. | none: 8 threads were 7 % slower in round 1 and 11 % faster in round 2, on 4 CPUs | none | none |
| 8 | A template Dolt database for the new-ledger fetch. The 10 tests that call `Fixture::new` test the first fetch itself, and the template builds already pay it once. | under 3 s | medium | some: a test of the first fetch that does not run it |
| 9 | Fewer `dolt` launches in the product: a resident `dolt sql-server` per store, or more statements per `dolt sql`. This is the only change that removes part of the 400 serial seconds of process start, and it changes the product, not the tests. | up to half the suite | large | none for the tests, product risk |

## Measured results

Four binaries, run one after another in two rounds, each from the same shell on the same
machine. Load average from other agents at the start of each run is in brackets. The
second round met a load average of 14 to 22 and is not comparable to the first.

| Variant | Round 1 wall | Round 2 wall | Median of 2 |
|---|---|---|---|
| Unchanged binary, 4 threads | 215.8 s (10.2) | 337.5 s (9.3, rising to 22) | 276.7 s |
| Unchanged binary, `--test-threads=8` | 230.3 s (9.1) | 298.9 s (15.3) | 264.6 s |
| Change 1: shared seeded fixture | 222.5 s (11.2) | 266.2 s (16.4) | 244.3 s |
| Changes 1 and 3: seeded fixture, heaviest six first | 212.2 s (9.7) | 272.4 s (14.6) | 242.3 s |

The wall times do not separate from the noise: change 1 was 3 % slower in round 1 and
21 % faster in round 2. The per-test times, which the harness reports for each test,
separate better. The three tests that take the seeded fixture (`thread_commands_`,
`push_and_merge_` and `fetch_and_status_read_the_same_without_batching`) summed to:

| Round | Unchanged | Change 1 | Changes 1 and 3 |
|---|---|---|---|
| 1 | 147.5 s | 117.8 s (-20 %) | 125.2 s (-15 %) |
| 2 | 240.4 s | 131.1 s (-45 %) | 172.1 s (-28 %) |

Taking the round-1 figure, change 1 removes about 30 s of thread time, which is 7 to 8 s
of wall time on 4 threads (3 to 4 % of the suite). That matches the estimate: 8 runs of
`seed_records`, 10 commands each, at 0.2 to 0.4 s a command under load. The heaviest-first
order showed no measurable gain of its own; its simulated gain is 14 s and the tail it
removes is real, so its one-line cost is justified, but not by a measurement.

Eight threads were slower in round 1 and faster in round 2. On 4 CPUs that are 86 % busy
with 4 threads, more threads add contention and nothing else. Keep the default.

The product-level change (rank 9) was not measured. Its ceiling is the 400 serial seconds
of `dolt` process start; a server would turn each into a socket round trip of a few
milliseconds.

## Flakiness

### `auto_update.rs`: `Text file busy`

`cli/tests/auto_update.rs` line 82 copies the 48 MB `tsk` binary into the fixture with
`std::fs::copy`, and line 197 spawns it. A fork on another test thread, between the open
and the close of that copy, inherits the write handle; until that child calls `exec`,
an `exec` of the file fails with ETXTBSY. The window is the copy's length, tens of
milliseconds, and the harness runs 4 tests at once, so it fires. It fired once in this
session's three workspace runs (`a_newer_ledger_warns_updates_and_runs_the_command_again`,
`cli/tests/auto_update.rs:197:41`, `ExecutableFileBusy`), and the first run of
`cargo test --workspace` stopped there because it is not `--no-fail-fast`.

Fix: the `OutputRetry` trait in `cli/tests/self_update.rs` (line 130) retries a spawn on
`ExecutableFileBusy` up to 200 times at 25 ms. Move it to a shared test helper and use it
for every spawn in `auto_update.rs`. Also replace the copy with `std::fs::hard_link`, with
`copy` as the fallback across file systems: a link opens no write handle, so the window is
gone on the common path.

`conformance.rs` has the same pattern in `race_shim`, `dolt_log_shim` and `git_log_shim`,
which write a shell script that `tsk` then executes. The write is microseconds long, so it
has not been seen, but the retry fits there too.

## Recommendation

In this order:

1. Fix the `auto_update.rs` race (retry on `ExecutableFileBusy`, hard link in place of the
   copy) and add `--no-fail-fast` to the `test` recipe. No time saved; one random failure
   gone.
2. Cache `thread.rs`'s fixture once per process, as `Fixture::cached` does in
   `conformance.rs`. Expected: 40 to 60 s off the workspace run. This is the largest
   saving per line changed.
3. In `conformance.rs`: the shared seeded fixture (change 1), the cached
   `numbered_fixture` (change 4), `dolt_installed()` once (change 6), and the heaviest
   tests first (change 3). Expected: 25 to 30 s off conformance, to about 185 s.
4. Run the test binaries concurrently (`cargo nextest run --workspace`, or a `just test`
   that starts `conformance` and `thread` first). Expected: 30 to 60 s off the workspace
   run.
5. Only then, and only if the suite's time still outweighs the cost of the product change: a
   resident Dolt server per ledger store. It is the one change that can halve the suite,
   and it belongs to a product decision, not a test one.

Expected totals on this machine with the load of the baseline runs (load average 1 to 5):
conformance from 212 s to about 185 s after step 3, and the workspace run from about 360 s
(Jim's figure; 615 s was measured here under a load of 8 to 14) to about 250 s after
steps 2 to 4. Step 5 would take conformance to about 100 s and the workspace run under
150 s, but that is an estimate from the launch counts, not a measurement.

What none of this changes: the suite is bound by `dolt` process starts on a machine whose
CPUs are already busy. Test-side changes move 10 to 30 % of the time. The rest is the
product's process model.
