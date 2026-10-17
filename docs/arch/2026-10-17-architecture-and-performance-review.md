# Architecture and performance review, 2026-10-17

A review of tsk at 0.15.1, after mission M-TSK-SCHEMA moved missions, tasks, decisions,
questions and bindings into typed Dolt records and migrated this repo's own ledger.
Written by Claude (Fable 5.1) at Jim's request, from the code, the ledger and
measurements taken in this cloud session on 2026-10-17.

## Method

- Code read: `cli/src/ledger/**`, `cli/src/thread/**`, `cli/src/mission/**`,
  `cli/src/tui`, `core/src`, `daemon/src`, the ADRs, `docs/domain/*.md`.
- Live measurements, read-only, on this repo's ledger at layout 8 (19 missions, 70 tasks,
  40 ledger commits, origin on GitHub through the cloud proxy). Wall time with `date`,
  process launches with `strace -f -e trace=execve`, counting successful launches only.
- Write-path measurements on a scratch copy of the real ledger with a local bare
  repository as origin, so they exclude network time.
- The 0.15.1 numbers come from the release binary built from `f966550`.

## Summary

1. The read path is fixed. A `mission list` is 0.08 s, from 0.81 s in 0.15.0. The
   Stop hook check is 0.09 s.
2. Every command that touches origin pays for a `dolt fetch` over the git remote:
   2.3 s on the live ledger, 0.5 s on a local origin. The cost is two `git cat-file`
   processes per Dolt table file on the remote, 107 files today, and nothing in the
   design shrinks that number. Session start is 3.5 s live. This is the next
   performance problem.
3. A thread command runs two Dolt fetches, two documents fetches, one code repository
   fetch and about 13 `dolt sql` processes. `thread pause` is 3.2 s on a local origin.
   Most of that is removable without a design change.
4. The binary still ships the daemon-era product: `tsk` with no arguments starts a TUI
   that needs `tskd`, the published `tsk-core` crate is the old model, and the `daemon`
   crate runs 43 tests on every build. ADR 0012 retired `tskd` on condition that the
   TUI stop depending on it. It still does.
5. Per-mission sequence numbers make two clones that add to one mission conflict at
   push. That is fine for one person and will not hold for several agents on one
   mission.
6. The ledger layout moved from version 2 to 8 in five days, and the first write of any
   newer binary upgrades a shared ledger for everyone. That suits a single user and
   needs a policy before other people run tsk.

## Architecture

### The storage seam

`LedgerStore` (`cli/src/ledger/store/mod.rs`) is the one seam ADR 0014 asked for, and
it held through eight layout versions: no command module shells out to `dolt` or `git`.
Two things have grown with it.

- The trait has 50 methods. Several name mechanism rather than meaning:
  `refresh_keeping_local`, `refresh_for_fetch`, `pending_inspect_hint`,
  `describe_thread`, `describe_threads`, `describe_document`. The dolt implementation
  (`store/dolt.rs`) is about 2,100 lines and the files implementation about 1,400.
- Every feature since T-02 was built twice, once per backend, with a conformance case
  for each. The files backend exists so the seam is real and so beads could be a third
  implementation later (ADR 0014, decision 2). That is a sound reason, and it has a
  price: about a third of each task's build went on parity, and the files ledger holds
  no live data anywhere.

Proposal: decide whether the files backend stays a first-class backend, becomes a
frozen reference implementation that new features may skip, or goes. Any of the three
is defensible. Leaving it undecided means paying the parity cost on every task by
default.

### Records and documents

The split is clean and the commands enforce it: records are typed rows written only
through `tsk` commands, documents are Markdown on `tsk/docs`, and a record names a
document by path only after the document exists (`tsk mission link`). The push order,
documents first and data second (`store/dolt.rs`, `commit_and_push`), means a pushed
record never names a document the remote lacks. The T-05 migration linked every old
briefing to its mission as `intelligence`, so the records are the truth and the
briefings are a dated snapshot. That is the right end state. One consequence to watch:
an agent that reads the linked briefing can take its plan table for current state. The
guide says the records win; a projection of the mission over the briefing would remove
the ambiguity (T-14).

### Threads, missions and bindings

The model settled this week is coherent: a mission is state, a thread is execution, a
binding is an actor's link to a thread, and all three are rows. Two limits are recorded
and worth repeating here because they will surface in use.

- `written_by` on a continuation entry is `urn:tsk:worktree:.git` for every main
  worktree, so two clones' main worktrees read as one writer in the takeover warning
  (`cli/src/thread/entry.rs`, noted in the 0.14.0 release). The `bindings` table now
  holds a stable actor key, `<clone-id>/<worktree-name>`. The entry should carry the
  same key.
- A linked worktree's actor key uses the worktree's basename, so a recycled name
  replaces the old binding.

### Identifiers and concurrency

Tasks, questions, objective outcomes, constraints and out-of-scope items are keyed
`(mission_id, seq)` with a per-mission counter on the mission row (`next_task` and so
on, `cli/src/ledger/dolt.rs`). Decisions use `max(seq) + 1`. Dolt merges rows by key, so
two clones that each add a task to the same mission before pushing produce the same key
and the second push stops with a conflict. The design doc accepts this. It is correct for
one person with several sessions on different missions, which is today's use. It is the
first thing that breaks when two agents work one mission at once, which is the use case
the threads and bindings model was built for.

Options, in order of least change: mint the sequence inside the push after the merge,
so the second clone's rows renumber; or give each actor a range; or use a random short
id and keep `T-nn` as a display number assigned at push. Each needs a decision before
multi-agent use on one mission, and none is urgent before then.

### Layout versions and upgrades

The Dolt layout went 2, 3, 4, 5, 6, 7, 8 between 2026-10-15 and 2026-10-17
(`DOLT_SUPPORTED_VERSIONS` in `cli/src/ledger/manifest.rs`). Each step is sound on its
own: chained `upgrade_sql`, reads work before the upgrade, a write upgrades, an older
binary stops with one error that names `tsk self-update`. The policy underneath is that
the first write of any newer binary upgrades the shared ledger for every clone. With one
user who updates every session that is a convenience. With a team it is a surprise: one
person's `tsk task add` stops everyone else's sessions until they update.

Proposal: keep the forced stop, because a stale binary writing an old layout is worse,
but make the upgrade an explicit `tsk ledger upgrade` that the session start context
offers, as `migrate-docs` is offered today. Batch schema changes into fewer layouts once
other people depend on the ledger.

### The daemon-era product is still in the binary

- `tsk` with no arguments starts the TUI (`cli/src/main.rs:126`), which reads
  `tsk_core::socket_path()` and reports "tskd is not running. Start it with: tskd"
  (`cli/src/tui/mod.rs:125`).
- `tsk-core` on crates.io is the pre-ledger model: `Thread` with a `u32` id,
  `ThreadState`, `Priority`, JSON-RPC request and response types, event log paths
  (`core/src/lib.rs`). The ledger code uses none of it. Only the TUI and `main.rs` do.
- The `daemon` crate is a workspace member and its 43 end-to-end tests run on every
  `cargo test --workspace`, 25 s per run.
- `docs/index.md` lists three architecture documents and all three describe the
  client-daemon design.

ADR 0012 says `tskd` is retired "once the TUI no longer depends on it". The condition
was never met, so the retirement is not complete. The `events` module is not part of
this: `tsk events` is live, it feeds the M-ADMIN-03 daily check from a GitHub Action,
and it uses the ledger.

Proposal: `tsk` with no arguments prints help; remove `cli/src/tui`, the `daemon` crate
and the old types from `tsk-core`, or stop publishing `tsk-core` and fold what the CLI
needs into `tsk-bin`. Mark the three architecture documents as historical in
`docs/index.md`. This cuts the published surface, 25 s from every test run, and a
confusing first run for a new user.

### The sync design

The ledger is three git-synced things: the Dolt database on `refs/heads/tsk/ledger`
through Dolt's git remote, the documents worktree on `refs/heads/tsk/docs`, and, in
`thread pause`, the code repository itself, fetched to check that the code commit is on
origin (`cli/src/thread/ops.rs:319`, refspec `+refs/heads/*:refs/remotes/origin/*`).
Every one is a round trip to origin, and from a cloud session origin is GitHub through
a proxy. The performance section counts them. The design is sound for correctness:
compare-and-swap on the documents, fetch, merge and conflict-table check on the data,
documents before data. It is expensive because each layer refreshes on its own.

### Tests

The shape is right: one body of conformance cases, run on both backends, with the
Dolt-only upgrade tests beside them. Before 0.15.1 the whole suite took 13 to 16
minutes, 500 to 770 s of it in the 139 conformance tests, and the sum of per-test times
was 1,979 s. Almost all of it was process launches: the slowest test spawned 828 `dolt`
processes. Fixing the product (fewer launches per command) and caching the fixtures
brought the conformance binary to 203 s and the suite to 4.5 minutes. The remaining
time follows the same rule: the next product speed-up shrinks the tests again.

### Command surface

The command set is consistent: one noun per record, the same verbs, printed IDs,
tab-separated lines for scripts. Two inconsistencies: `tsk ledger status` has a human
layout with spaces and a `--porcelain` form, while `mission list`, `task list`,
`decision list` and `question list` print tabs only, which Jim's terminal drops. And
`decision list` with no mission and no `--adr` is an error where `question list` with
no mission lists everything.

## Performance

### Read path, live ledger

| Command | 0.15.0 | 0.15.1 | `dolt` processes 0.15.0 to 0.15.1 |
|---|---|---|---|
| `tsk mission list` | 0.81 s | 0.08 s | 11 to 1 |
| `tsk task list M-TSK-SCHEMA` | 0.76 s | 0.08 s | 11 to 1 |
| `tsk mission list <id> --json` | 0.70 s | 0.07 s | 11 to 1 |
| `tsk decision list`, `tsk question list` | 0.75 s | 0.08 s | 11 to 1 |
| `tsk thread binding --no-fetch` | 0.07 s | 0.07 s | 1 |
| `tsk thread guard` (Stop hook) | 0.08 s | 0.09 s | 1 to 2 |

The 0.15.0 cost was one `dolt sql` process per table at about 80 ms each. 0.15.1 runs
every table query and the manifest check in one process (`Dolt::rows_each`,
`cli/src/ledger/dolt.rs:396`) and parses the results in order. Output is byte for byte
identical over a scripted flow of about 110 commands.

### Anything that fetches, live ledger

| Command | Time | Launches |
|---|---|---|
| `tsk ledger fetch` | 2.3 to 2.5 s | 423 `git cat-file` |
| `tsk ledger status` | 1.75 s | |
| `tsk thread list` | 4.1 s | |
| `tsk thread session-start` | 3.5 s | |

The mechanism: Dolt's git remote stores each Dolt table file as a blob on the branch.
The remote manifest on the live ledger names 107 table files after 40 commits. A
`dolt fetch` reads each one with `git cat-file -s` and then `git cat-file blob`, two
processes per file, so 423 launches, about 2.3 s over the network and 0.5 s on a local
origin. This is inside `dolt`, not in tsk.

Two experiments on the scratch copy. A local `dolt gc` cut the local table files from
109 to 4, and a `dolt fetch` afterwards still launched 440 processes, because the cost
is the remote's file count. Pushing the compacted database did not shrink the remote
either: the remote manifest still named 119 files. So the number grows with pushes and
nothing in the current design brings it down. Every refresh pays it, and a thread
command pays it twice.

### Write path, scratch copy of the real ledger, local origin

| Command | 0.15.1 | `dolt` and `git` launches |
|---|---|---|
| `tsk mission create`, `tsk task add` | 0.2 s | 2 |
| `tsk ledger push` with a change | 1.9 s | |
| `tsk ledger push` with nothing to do | 0.9 s | |
| `tsk thread start` | 3.2 s | |
| `tsk thread pause` | 3.2 s | 2 `dolt fetch`, 2 docs `git fetch`, 1 code `git fetch`, 1 `dolt reset`, 1 `dolt commit`, 1 `dolt push`, 13 `dolt sql` |
| `tsk thread list` | 3.4 s | |

Inside `thread pause` every `dolt` launch is 0.17 to 0.23 s apart, so the 13 `dolt sql`
processes alone are about 2.5 s. They are: the refresh's manifest and dirty checks, the
thread existence check (twice, once in `pause` and once in the push path), the mission
load for the task check, the mission load again at push for the document-link check,
the staging count, and the push loop's merge-base query.

### Proposals, in order

1. **One fetch per command.** `Session::refresh` fetches, then `push_data`
   (`store/dolt.rs:258`) fetches again a second later. Keep the remote head from the
   first fetch and skip the second when the remote has not moved. The documents side
   does the same: `docs::fetch` in the refresh, then `fetch_tip` twice in
   `push_worktree` (`cli/src/ledger/push.rs:63` and `:72`). Saves one Dolt fetch and
   one or two git fetches per thread command: 0.6 s on a local origin, 2.5 s or more
   live.
2. **One or two `dolt sql` processes per thread write.** Fold the thread existence
   check, the mission load and the staging into the read batch and the write
   transaction, and drop the second mission load at push when no record links a
   document. Saves about 2 s per thread command. Together with 1, `thread pause` on a
   local origin goes from 3.2 s to about 0.7 s.
3. **Fetch the code repository in `pause` only when the code commit is not already
   known on origin.** `git branch -r --contains` against the local remote-tracking refs
   first, and the network fetch only when that fails.
4. **Compacting the remote.** This is the one that needs a design, and it decides
   whether the git remote scales. Options: find out whether Dolt's git remote can
   replace the remote manifest's file set after a `gc` (the experiment above says a
   plain push does not); a `tsk ledger compact` that writes a compacted database to a
   fresh branch tip, which rewrites the ledger branch's tree and is acceptable only if
   every clone fetches before its next push; or the server ADR 0014 names as the
   future evolution, which removes the git remote from the hot path and which the
   `future-missions-tbd` ideas already raise. The nexus-entry design from T-07 is the
   natural place to say which a repo uses.
5. **Force the fallback paths in tests.** 0.15.1 keeps the old query-by-query path as a
   fallback when the batch fails or the layout is not current. A fallback that no test
   exercises rots. One conformance case per batched command that runs with the batch
   disabled (an environment variable the test sets) keeps both paths honest.
6. **Session start.** After 1 to 3 it is one Dolt fetch plus one documents fetch plus
   one `dolt sql`: about 2.6 s live, bounded by the fetch, and under 1 s once 4 lands.

### Risks noted in 0.15.1

- The manifest check is remembered for the rest of the process once it finds the
  current layout, and refresh, discard and push clear it (`store/dolt.rs:48`). A second
  process that upgrades the ledger between two checks in one run would hit a failed
  transaction, not corruption. Narrow, and acceptable.
- `rows_each` fails when Dolt prints a different number of result sets than queries
  sent. That is the right failure mode; the fallback then runs the old path.

## Proposed order of work

1. Performance proposals 1, 2, 3 and 5: one pass, small and behaviour-neutral, with
   the same byte-for-byte comparison 0.15.1 used. Thread commands under 1 s on a local
   origin.
2. Complete ADR 0012: remove the TUI, the daemon and the old `tsk-core` model, and make
   `tsk` with no arguments print help.
3. A design task for proposal 4, remote compaction or a server, joined with T-07 (the
   custom ref) and the nexus entry, since all three decide where a repo's ledger lives
   and how it syncs.
4. A decision on identifiers under concurrency before two agents work one mission.
5. An upgrade policy: `tsk ledger upgrade`, offered at session start, and fewer layout
   versions once other people depend on the ledger.
6. A decision on the files backend: first class, frozen, or removed.
7. The small things: `written_by` carries the actor key; aligned output with
   `--porcelain` on the list commands; `decision list` with no arguments lists
   everything.
