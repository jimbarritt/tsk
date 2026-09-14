# Persistence and sync

Status: settled on approach; several mechanics still open. Source: 2026-09-14
bootstrap design session.

## Decided

- tsk needs state that persists outside the local machine.
- The sync mechanism is built from scratch, in Rust. Dolt is not adopted.
- Event data is stored under a custom git ref, for example `refs/tsk/data`.
- Sync happens by pushing and fetching that ref, using `--force-with-lease` for
  atomic writes.
- SQLite stays the embedded local cache for query projections. The cache is
  disposable; it can be rebuilt from the event log (see
  [ADR 0007](../adr/0007-event-log-as-source-of-truth.md)).

## Why not Dolt

Dolt is a general-purpose, version-controlled SQL database from DoltHub, not a beads
component; beads adopted it. Its git remote feature was added later, partly to
support beads and Gas Town moving from SQLite to Dolt.

- Dolt's core is written in Go. Using it from Rust means running a separate process
  or connecting over the MySQL wire protocol.
- Adopting Dolt would mean modelling tsk's event data as SQL tables, which changes
  the settled append-only NDJSON design.
- tsk writes one log file per actor, so writers rarely contend on the same file. This
  lowers the compare-and-swap collision risk that Dolt's shared-table model faces.

## How the git ref mechanism works

- A git ref is a named pointer to a commit. Branches and tags are refs; a ref can be
  created under any path.
- A custom ref is not fetched by a normal clone or fetch; it must be fetched
  explicitly.
- Data is stored as git blobs using `git hash-object`. A blob has no filename;
  filenames come from listing blobs in a tree object.
- The sequence: blob, then tree, then commit, then point the ref at the commit.
- No working copy is needed. A bare repository holds only the git internals.
- Concurrent writes use compare and swap: fetch the current remote ref state, build a
  new commit on top of it, then push with `--force-with-lease`. If another writer got
  there first, the push is rejected; fetch again, rebuild, retry.
- Custom refs do not conflict with protected branches.

## Language choice

Rust is confirmed. Go was considered for portfolio value and rejected.

- Rust has no garbage collector: better on CPU-bound work and allocation patterns.
- Rust uses less memory at runtime; Go's collector needs heap headroom.
- Rust has more predictable tail latency; Go's collector pauses are small but
  present.
- Go goroutines are simpler to use; Rust matches this with an async runtime or OS
  threads, with more manual work.
- Rust binaries are smaller and start slightly faster.
- For this specific layer the difference is small: the work is dominated by git
  operations and I/O, not by the calling code.

## Open

- The data ref tree layout: how per-actor NDJSON logs sit in the tree.
- The manifest format: what it lists, how entries are keyed.
- The push and pull protocol: the retry loop, lease handling, conflict cases.
- Rust git library choice: `git2`, `gitoxide`, or shelling out to the `git` binary.
- Daemon addressing across projects: one `tskd` serving many projects, or one per
  project.

These are decided by mission M-BOOT-04, the official data ref mission (not yet broken
into its own briefing at the time of writing).
