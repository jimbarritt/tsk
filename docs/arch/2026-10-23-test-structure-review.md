# Test structure review: where the suite spends its time, 2026-10-23

Written by Claude (Fable 5.1) for M-TIDY T-15. Read-only analysis of `main` at 0.25.0 plus
the test fixes. Nothing was written to the repo tree except this document, and nothing to
the live ledger or git. The scratch copies, logs and target directories are removed. It builds on
[2026-10-20-conformance-suite-speed.md](2026-10-20-conformance-suite-speed.md), which looked
at the conformance file only, and on the server measurements in
[2026-10-21-local-dolt-server-results.md](2026-10-21-local-dolt-server-results.md).

## Summary

- The workspace suite takes 258 to 260 s of harness time with the server on (two runs, within
  1 %): conformance 86 s, `thread` 28 s, `ledger` 22 s, `events` 19 s, `migrate_docs` 19 s,
  `transport` 17 s, `dolt_server` 16 s. A run starts 3,419 `dolt` and 27,994 `git` processes
  and 677 Dolt servers, about one server per test.
- The time is in fixtures, not in assertions. 236 of the 408 integration tests want the same
  thing, a published ledger with two clones, and 130 of them build it from nothing (two
  `git init`, a commit, two clones, `tsk ledger fetch`, a push) at 1.3 to 2.8 s each. Only
  `conformance` and `thread` copy a template. The uncached fixture builds are about 190
  thread-seconds, 45 s of wall time at four threads.
- A server lives 0.5 s at the median and under 2 s in 72 % of cases. 677 starts of a 94 MB
  process cost about 75 thread-seconds and much of the 34 % system time. The server is per
  store, and the suite makes a store per test.
- The 17 test binaries run one after another. The tails inside them cost 24 s. One binary
  that holds every test file as a module ran in 241 s against 260 s, measured, with no change
  to any test. It also builds in 22 s against 27 s clean and takes 562 MB of target against
  905 MB.
- Build time is not a problem: a clean `cargo test --no-run` is 27 s, an edited test file
  rebuilds in 0.4 s, an edited `src/main.rs` in 1.8 s. `lld` is already the linker. A
  debug-info change saves 3 to 6 s of the clean build and nothing of a run.
- Repeated work by design dominates the tail: the four `*_read_the_same_with_the_server` cases
  (14, 14, 12 and 5 s) repeat the batched run that the `*_without_batching` case of the same
  script makes; `every_kind_of_record_is_renumbered_at_the_second_push` builds
  `numbered_fixture` six times (16 s, the longest test); `migrate_docs` builds a layout-1
  store by hand nine times (6 s each, no server).
- Expected after the ranked changes: about 180 s with the server on, from 260 s, with the same
  assertions. The one measured change (one binary) is 19 s of that.

## Method and noise

- Dolt 2.5.0 first on `PATH`, `TSK_DOLT_SERVER=on` from `.cargo/config.toml`, stable 1.97 with
  `RUSTC_BOOTSTRAP=1` for `--report-time` and `--format json`. Per-test times are the harness's
  `exec_time`. Per-binary times are its `finished in`.
- Process counts come from a C stand-in first on `PATH` for `git` and `dolt` that appends the
  time, pid, parent pid and arguments to a log and `execv`s the real binary. It costs about
  1 ms per launch, so a run with it is 5 to 8 % slower than Jim's 239 s.
- A sampler read `/proc/stat`, `/proc/meminfo` and the `dolt sql-server` processes every
  0.5 s, so a server that lived under 0.5 s may be missing: it saw 550 of 677.
- The machine has 4 CPUs and 16 GB. The load average was 0.6 at the start of run 1 and 5 to 7
  during the later runs, most of it the suite itself. Two runs of the unchanged suite agreed
  within 1 % (257.6 s and 259.8 s), so a difference under 5 % is not a measured saving.
- Builds ran in a scratch copy of the tree with its own `CARGO_TARGET_DIR`, never in the repo.

## 1. Where the time goes

Harness time per binary, run 2 (run 1 in brackets where it differs by more than 0.5 s), with
the processes started while that binary ran. Binaries run one after another, so the counts
are exact per binary.

| Binary | Tests | Harness s | Sum of test s | Longest test s | `dolt` | `git` | Servers |
|---|---|---|---|---|---|---|---|
| `conformance` | 115 | 86.5 (84.3) | 340.6 | 15.4 | 913 | 11,751 | 272 |
| `thread` | 67 | 28.3 | 108.7 | 5.5 | 268 | 2,952 | 131 |
| `ledger` | 35 | 22.1 (21.0) | 79.2 | 5.8 | 346 | 1,998 | 59 |
| `events` | 23 | 19.5 (18.3) | 70.5 | 5.5 | 298 | 1,774 | 61 |
| `migrate_docs` | 9 | 18.9 (22.6) | 55.0 | 11.6 | 260 | 1,716 | 2 |
| `transport` | 12 | 16.6 | 52.2 | 8.5 | 259 | 1,550 | 20 |
| `dolt_server` | 17 | 16.3 | 60.0 | 8.1 | 280 | 980 | 30 |
| `ledger_refs` | 24 | 14.4 | 53.5 | 3.6 | 231 | 1,589 | 33 |
| `nexus` | 25 | 14.3 | 53.5 | 5.8 | 253 | 1,611 | 31 |
| `background` | 16 | 11.2 | 36.6 | 3.6 | 156 | 1,168 | 15 |
| `auto_update` | 8 | 4.2 | 16.4 | 2.2 | 95 | 226 | 15 |
| `git_protocol` | 3 | 2.1 | 6.4 | 2.1 | 36 | 165 | 6 |
| `nexus_cmd`, unit tests, `hooks`, `self_update`, `install_plugin`, `broken_pipe` | 392 | 5.3 | 13.5 | 1.6 | 24 | 514 | 2 |
| All | 746 | 259.8 (257.6) | 946 | | 3,419 | 27,994 | 677 |

The `dolt` launches by subcommand: `sql` 905, `sql-server` 677, `config` 559, `clone` 211,
`version` 208, `remote` 205, `add` 195, `commit` 195, `init` 187, `push` 48, `fetch` 17.
With the server on, the command line is left with store creation (`init`, `clone`, `remote`,
`config`, `add`, `commit`), the tests' own `dolt sql` reads and writes, the git-remote cases,
and one `version` per test that checks for `dolt`. The `git` launches: `rev-parse` 8,430,
`ls-remote` 1,669, `worktree` 1,515, `add` 1,514, `cat-file` 1,495, `fetch` 1,401, `diff` 1,352,
`push` 1,267, `commit` 1,029; 3,972 came from `dolt` (the git-remote transport), the rest from
`tsk` or the tests. A `tsk` process starts 3 `git` at the median and 18 at the 90th percentile.

The 20 slowest tests, mean of the two runs. They are 182 of the 946 thread-seconds.

| Test | Binary | Run 1 s | Run 2 s |
|---|---|---|---|
| `every_kind_of_record_is_renumbered_at_the_second_push` | conformance | 15.9 | 15.4 |
| `thread_commands_read_the_same_with_the_server` | conformance | 13.6 | 14.6 |
| `push_and_merge_read_the_same_with_the_server` | conformance | 13.9 | 13.6 |
| `a_version_one_ledger_is_refused_by_every_command_except_fetch_path_and_migrate_docs` | migrate_docs | 14.6 | 11.6 |
| `fetch_and_status_read_the_same_with_the_server` | conformance | 12.1 | 11.3 |
| `a_rejected_docs_push_leaves_the_dolt_data_unpushed_and_tsk_ledger_push_resumes` | migrate_docs | 10.9 | 8.0 |
| `migrate_docs_refuses_a_path_that_cannot_be_a_file_and_names_it` | migrate_docs | 9.5 | 8.4 |
| `a_clone_with_unpushed_changes_keeps_them_across_a_migration` | transport | 8.5 | 8.5 |
| `a_clean_clone_follows_a_migration_on_its_next_fetch_and_a_fresh_clone_takes_the_file_remote` | transport | 8.3 | 8.3 |
| `a_version_four_ledger_reads_threads_and_upgrades_at_the_first_thread_start` | conformance | 8.2 | 7.9 |
| `a_connection_that_stays_open_keeps_the_server_up_past_the_idle_time` | dolt_server | 8.0 | 8.0 |
| `a_push_that_conflicts_stops_with_the_same_message_and_leaves_origin_alone` | dolt_server | 7.8 | 8.1 |
| `a_command_with_the_switch_off_works_while_a_server_runs_even_a_merge` | dolt_server | 7.5 | 7.7 |
| `a_ledger_made_with_the_git_remote_transport_works_and_a_clone_follows_the_ref` | transport | 7.5 | 7.6 |
| `a_version_seven_ledger_reads_its_bindings_from_the_old_table_and_the_first_write_upgrades_it` | conformance | 6.8 | 6.7 |
| `migrate_docs_moves_every_document_byte_for_byte_and_a_fresh_clone_reads_them` | migrate_docs | 6.7 | 6.7 |
| `thread_set_mission_attaches_changes_and_detaches_a_mission` | conformance | 6.3 | 6.8 |
| `a_rerun_compares_the_files_the_docs_branch_already_holds_and_goes_on` | migrate_docs | 7.9 | 5.1 |
| `fetch_and_status_read_the_same_without_batching` | conformance | 6.6 | 6.4 |
| `migrate_transport_converts_an_old_ledger_with_a_normal_commit` | transport | 6.4 | 6.3 |

The median integration test takes 2.1 s. The fastest test of a file shows its fixture's cost:
0.1 to 0.3 s where a template is copied (`conformance`, `thread`); 1.3 s (`background`), 1.5 s
(`events`), 1.75 s (`auto_update`), 2.1 s (`git_protocol`), 2.8 s (`transport::published`) and
3.2 s (`migrate_docs`) where the fixture is built.

## 2. How the fixtures are organised

Every test file has its own `Fixture` type, clone names, environment and helpers;
`cli/tests/common/mod.rs` holds only `stop_servers`. 14 of 17 files make a bare `origin.git`,
a `seed` clone that pushes `main`, then one to three clones. What differs is what happens next
and whether the result is cached.

| File | Tests | Fixture | Cached | Clones | What a test gets |
|---|---|---|---|---|---|
| `conformance` | 115 | `Fixture` | yes, 9 templates (`fetched`, `briefing`, `seeded-records`, `version-two` to `-seven`) | 2 | 39 copy `fetched`, 31 `briefing`, 15 build `new`, 7 build `numbered_fixture` on a copy, 19 copy a version template, 4 copy `seeded-records` |
| `thread` | 67 | `Fixture` | yes, 2 templates (`main`, `trunk`) | 2 | a published ledger with a briefing; 62 copy it; 12 then seed rows by `dolt sql` from the other clone |
| `ledger` | 35 | `Fixture` | no | 2 (`work`, `publisher`) | 33 build `with_ledger`: publisher fetches and pushes, `work` has not fetched |
| `events` | 23 | `Fixture` | no | 2 | each builds, fetches and pushes a ledger; 12 then seed rows from the other clone |
| `background` | 16 | `Fixture` | no | 1 | each builds, fetches, puts a briefing and pushes |
| `transport` | 12 | `Fixture` | no | 3 | 6 build `published` (fetch, a mission, push), 5 start with no ledger |
| `dolt_server` | 17 | `Fx` | no | 2 | each builds and fetches; the switch is set per case |
| `ledger_refs` | 24 | `Fx` | no | 2 | git only, then 13 fetch and 11 push inside the test; 4 add a nexus |
| `nexus` | 25 | `Fixture` | no | 1 plus a nexus repo | git only; 21 attach the nexus, 9 fetch |
| `migrate_docs` | 9 | `Fixture` | no | 2 | git only, then a layout-1 store built by hand with `dolt` and pushed through the git-remote transport |
| `auto_update` | 8 | `Fixture` | no | 1 | fetch, then the manifest set to layout 9 by `dolt sql`, and a copy of the binary |
| `git_protocol` | 3 | `Fixture` | no | 2 | git only plus a logging `git`; the test fetches and pushes |
| `nexus_cmd` | 16 | `Fixture` | no | 1 plus a nexus repo | git only, no store |
| `hooks`, `install_plugin`, `self_update` | 38 | `Machine`, `Fixture` | no | 0 or 1 | shell stand-ins for `claude`, `dolt`, `tsk`; no store |
| `broken_pipe` | 1 | none | | | a pipe |

Grouped by what the test needs, with the thread-seconds of run 2:

| Group | Needs | Tests | Thread s | Mean s |
|---|---|---|---|---|
| A | No ledger: stand-in scripts, a nexus repo, a pipe | 55 | 10.7 | 0.19 |
| B | Git repos and no store (the first fetch, a missing `dolt`, a file ledger on the ref) | 5 | 12.6 | 2.5 |
| C | A fresh empty ledger in one clone, no origin ledger | 38 | 65.9 | 1.7 |
| D | A published ledger, two clones, a server per clone that runs a command | 236 | 560.1 | 2.4 |
| E1 | A ledger seeded with records in both clones, or an old layout | 15 | 100.7 | 6.7 |
| E2 | A layout-1 store on the git-remote transport (`migrate_docs`) | 9 | 55.0 | 6.1 |
| F | A nexus repo and a managed clone | 33 | 78.0 | 2.4 |
| G | A server in a given state (`dolt_server`) | 17 | 60.0 | 3.5 |

Seven groups, and one of them (D) is 58 % of the tests and 59 % of the time. Group D comes
from a template in `conformance` and `thread` (93 tests) and from nothing in `ledger`,
`events`, `background`, `transport`, `git_protocol`, `ledger_refs` and the pushing tests of
`nexus` (about 130 tests). A copy costs 34 ms plus `rewrite_paths`; a build costs 1.3 to
2.8 s. The caches are per process, so the `briefing` and `thread` templates, the same ledger
with a different briefing path, are built twice in a run, and `fetched` is built in one
binary and rebuilt by hand in six others.

## 3. Serial binaries and the long tail

Cargo starts each binary when the one before ends, so there is no gap between them (0.0 s in
both runs). The idle time is inside each binary, when the last tests of its 4-thread schedule
run alone. The per-binary harness times sum to 259.8 s and the ideal times (thread-seconds
divided by 4) to 232 s. The 27 s of tails sit where one test is far longer than the rest:
`migrate_docs` (18.9 s against 13.8 ideal), `transport` (16.6 against 13.1), `background`
(11.2 against 9.2), `ledger` (22.1 against 19.8), `events` (19.5 against 17.6).

CPU over a run: 36 % user, 34 % system, 23 % idle, 4 % I/O wait. The system share is process
creation, `execve` of a 100 MB `dolt` and a 51 MB `tsk`, and page-in of each new server. The
idle share is the tails and the waits inside tests (idle stops, background publishes, probes).

Measured: one test binary that holds every test file as a module (`autotests = false`, a
`tests/all/main.rs` of `#[path]` modules, and `use crate::common;` in place of `mod common;`).
No test changed. The harness ran all 409 integration tests in one 4-thread pool.

| Run | Harness s | Idle CPU | Servers | Peak memory in use |
|---|---|---|---|---|
| 17 binaries, 4 threads, run 1 and 2 | 257.6, 259.8 | 23 % | 678, 677 | 1,000 MB, 1,021 MB |
| One binary, 4 threads | 240.7 | 18 % | 677 | 1,193 MB |
| One binary, 6 threads | 245.1 | 10 % (13 % I/O wait) | 693 | 1,408 MB |

One binary is 19 s faster (7 %), which is the tail. Splitting the slowest tests would recover
part of the same time within the serial layout, but the one binary recovers all of it and is
one change. Six threads gain nothing: the harness time is 2 % longer, I/O wait triples, the
slowest test takes 55 s instead of 15, and 16 more servers start because the 3 s idle stop
fires between a test's commands. `cargo nextest` is not that change: it runs each test in
its own process, so the per-process template caches would build once per test.

## 4. Processes and servers

| Measure | Value |
|---|---|
| Servers started in a run | 677 (0.95 per test; 272 in conformance, 131 in thread, 61 in events, 59 in ledger) |
| Server lifetime | median 0.5 s, 90th percentile 1.5 s; 374 under 1 s, 113 of 1 to 2 s, 38 of 2 to 3 s, 11 of 3 to 10 s, 4 over 30 s (the `dolt_server` idle cases) |
| Server-seconds in a run | 800 |
| Servers alive at once | mean 2.5, peak 8 to 9 |
| Resident size of a server | median 94 MB, max 125 MB; 795 to 803 MB at the peak |
| Memory in use, whole machine | 630 to 660 MB before, peak 1,000 to 1,193 MB |
| Processes left after the run | none: `servers=0` in the last sample of each run, and no `dolt` process on the machine after |

A fixture stops its servers in `Drop` with `SIGKILL` and sets a 3 s idle time, so the count
stays near the thread count. What costs is the start: 677 starts at about 110 ms to the first
answer (the results document's figure) is 75 thread-seconds, plus the page-in of 94 MB each,
a part of the 34 % system time that the sampler cannot split out.

Could tests share a server? A server is per store, and a store is per test because nearly
every test writes, so groups C, D, E and G keep one server per test, and a template copied
per test is still one store and one server. A shared read-only store with one live server
would fit the tests that only read: about 25 in `thread` (`session_start_*`, `binding_*`,
`guard_*`, `list_with_no_threads`, `version`) and about 10 in `conformance` (`status_*`,
`session_start_*`, the stop hook cases). That is 35 of 677 servers and perhaps 5 s, for a
fixture that no test may write to. It is a small gain with a real risk, after the template work.

## 5. Repeated work

- The four `*_read_the_same_with_the_server` cases run their script with the switch off and
  on (14.1, 13.7, 11.7 and 5.1 s). The four `*_read_the_same_without_batching` cases run the
  same scripts batched and unbatched with the switch of the run, on (6.5, 6.3, 5.3 and 1.8 s).
  The batched run with the switch on is made in both. One test per script that observes three
  runs and keeps both comparisons removes one run in four: about 16 thread-seconds, and three
  of the top five tests shrink by a third.
- `every_kind_of_record_is_renumbered_at_the_second_push` builds `numbered_fixture` six times,
  once per kind of record, and seven other tests build it once: 13 builds of 8 record
  commands, a push and a fetch in the second clone. As a cached template (as `seeded-records`
  is), the 13 builds become one. About 20 thread-seconds, and the longest test falls from
  15.7 s to about 5 s, which is the tail of the whole suite at four threads.
- The `briefing` template of `conformance` and the `main` template of `thread` are the same
  ledger with a different briefing path. `ledger::with_ledger` is `fetched` with a push.
  `events`, `background`, `git_protocol` and `transport::published` are `briefing` with a
  different document or a mission. One template library in `common` with `fetched`,
  `published` and `seeded` templates, built once per process, serves all of them; a test that
  needs a particular document puts it after the copy.
- `migrate_docs` builds its layout-1 store nine times. Three tests pass the same
  `OLD_DOCUMENTS` and no extra SQL and could share one template; the others differ in their
  arguments. The build itself is on the git-remote transport, where the server does not apply
  (ADR 0018, decision 6), which is why this binary gained nothing from the switch.
- `dolt config` runs 559 times, once per fresh `HOME` (`ensure_identity`), and `dolt version`
  208 times, once per test that checks for `dolt`. A template `HOME` carries the Dolt config,
  and a `OnceLock` makes the check one launch per process: about 45 thread-seconds.
- Tests that overlap in what they prove: `ledger::fetch_*` (11 tests) and
  `conformance::fetch_creates_a_new_ledger_and_the_first_push_creates_the_branch`;
  `events::*` and `conformance::events_append_read_new_and_advance_the_watermark`; the
  conflict-at-push cases of `transport`, `dolt_server` and `conformance`. Each repeats a 1.5
  to 3 s fixture for a few assertions. The saving is the fixture, not the assertions, and the
  template library gives most of it with no merge. No test is proposed for removal.

## 6. Build time

Measured in the scratch copy under a load average of 4 to 7 from the test runs. `lld` is
already the linker (the `.comment` section of every binary names LLD 22.1.6).

| Build | Default profile | `debug = "line-tables-only"` | `debug = 0` | One test binary |
|---|---|---|---|---|
| Clean `cargo test --workspace --no-run` | 27.8 s, 26.6 s | 24.6 s | 20.8 s | 21.6 s |
| Touched test file, no edit | 0.3 s | 0.3 s | | |
| Edited test file (one line added) | 0.4 s | | | 1.3 s |
| Touched `src/main.rs`, no edit | 1.7 s | | | |
| Edited `src/main.rs` (one line added) | 1.8 s | | | |
| Target directory | 905 MB | 607 MB | 378 MB | 562 MB |
| Test binaries | 17 of 9 to 16 MB, 193 MB in all; `tsk` 51 MB | 221 MB of executables | 67 MB of executables | one of 34 MB |

The clean build is the dependency crates (`clap`, `serde`, `toml`, `serde_json`) and the
`tsk-bin` crate itself; the 17 test crates link against `std`, `serde_json`, `tempfile` and
`libc` only, because `tsk-bin` has no library target, and each links in about a second. The
one-binary variant saves 5 s of the clean build and 343 MB of disk, and costs 1 s on an edit
to a test file, because one crate holds 20,000 lines instead of 17 crates of 30 to 8,000.
`debug = 0` saves 6 s of the clean build and 527 MB, and loses the line numbers in a panic's
backtrace, which a failing end-to-end test needs. Neither matters next to the 260 s run; the
incremental case, the one a person meets, is already under 2 s. The 8.6 GB of stale
executables in the repo's `target/debug/deps` (470 files) is disk, not time; `cargo clean`
recovers it.

## 7. Changes, ranked

Savings are harness time on this machine at four threads from the 260 s baseline with the
process stand-in on (Jim's 239 s without it). "Measured" means built in the scratch copy and
run; the rest are estimated from the thread-seconds above divided by 3.5, which is what four
threads delivered on this run (946 thread-seconds in 260 s).

| Rank | Change | Saving | Effort | Risk to coverage | Basis |
|---|---|---|---|---|---|
| 1 | One integration test binary: `autotests = false`, `tests/all/main.rs` with `#[path]` modules, `use crate::common;` in each file. Keep the files where they are. | 19 s | small, one morning | none: every test ran unchanged | measured, 240.7 against 259.8 s; also 5 s off the clean build and 343 MB of target |
| 2 | A template library in `common`: `fetched`, `published` (briefing) and `seeded` (records in both clones), built once per process with the `cached` copy-and-rewrite that `conformance` has, and used by `ledger`, `events`, `background`, `transport`, `git_protocol`, `auto_update`, `dolt_server` and the pushing tests of `ledger_refs` and `nexus`. With change 1, once per run. | 40 to 50 s | medium, two to three days: each file keeps its `Fixture` and its clone names, and gets its root from the template | none for the tests that use the result; the 11 `ledger::fetch_*` tests and the 5 first-fetch tests keep building what they test | 130 builds at 1.3 to 2.8 s, floors measured per file; the two template files already show the floor at 0.1 to 0.3 s |
| 3 | Cache `numbered_fixture` as a template, and merge each `_with_the_server` case with its `_without_batching` case into one three-way comparison. | 9 to 11 s | small | none: every comparison is kept, one duplicate run is dropped | 36 thread-seconds from the per-test times; the longest test falls from 15.7 to about 5 s |
| 4 | A Dolt config in the template `HOME`, and `dolt_installed()` once per process. | 10 to 13 s | tiny | none | 767 launches at about 60 ms |
| 5 | Six test threads with one binary. | none: 2 % slower | none | none | measured, see section 3 |
| 6 | A shared read-only store and server for the tests that only read. | about 5 s | medium | some: a write by one test reaches the others | 35 of 677 servers |
| 7 | One layout-1 template for the three `migrate_docs` tests with the same arguments. | 3 to 5 s | small | none | 3 builds at 3.2 s |
| 8 | A debug-info profile for tests. | 3 to 6 s of a 27 s clean build, nothing of a run | tiny | none | measured |

Not proposed: `cargo nextest` (it defeats the per-process caches) and removing any test.
Recommended order: 1, then 2 with 3 and 4 in the same change, since they touch the same
helpers; 6 and 7 only if the suite is still too slow after them. Expected total with the
server on, on this machine: 260 s to about 180 s with the stand-in (about 165 s without it,
from Jim's 239 s), a third off, with every assertion kept. About 650 thread-seconds remain
after these changes, so a further cut needs more CPUs or fewer tests.
