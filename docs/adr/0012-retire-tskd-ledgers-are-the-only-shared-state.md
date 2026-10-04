# 12. Retire tskd: git ledgers are the only shared state

Date: 2026-10-04

## Status

Accepted. Supersedes the daemon parts of ADR 0002, ADR 0003 and ADR 0006 once the TUI
no longer depends on `tskd`.

## Context

`tskd` is a per-user daemon. It holds the original thread and task model in `~/.tsk/`
and serves it over a Unix socket with JSON-RPC (ADR 0002, ADR 0003). The TUI reads from
it by polling (ADR 0006). It also serialises writes: every write goes through one
process, so writes never run concurrently.

M-BOOT-04 moves missions, threads, continuation state and the external event queue
into ledgers held in git. A ledger is a branch, either `refs/heads/tsk/ledger` in the
managed repo or a namespaced branch in the nexus (`ledger-layout.md`). The nexus
records each managed repo in `nexus.json` and where its ledger lives. The `tsk` binary
reads and writes ledgers with the `git` executable and no daemon.

The state outside git is local to one machine and is not shared: `.git/tsk-clone-id`,
the thread binding markers in each code worktree, the user config that records the
attached nexus, and the ledger worktrees under `${XDG_STATE_HOME:-$HOME/.local/state}`.

## Decision

`tskd` is retired. Git ledgers, located through the nexus, are the only shared state.
The `tsk` binary performs every read and write without a daemon.

The TUI is rebuilt to read ledgers through the binary. A global view reads `nexus.json`
from the nexus, then fetches each managed repo's ledger from the location its entry
names.

## Consequences

- Writes from separate clones are already serialised at push time: `tsk ledger push`
  rebases onto the fetched tip and pushes with a compare and swap. Writes from two
  sessions in one clone have no guard. That gap is recorded in `future-missions-tbd.md`
  under "Concurrent writes to the shared ledger worktree", and the binary closes it with
  a lock file or a ledger worktree per session.
- The `daemon` crate and its tests stay in the workspace until the TUI no longer depends
  on `tskd`, then they are removed. The tests sit in `daemon/tests/` so that removal
  touches the daemon crate only.
- The thread and task data in `~/.tsk/` belongs to the original model. Its live usages
  in other projects are migrated by hand.
- A TUI that watches many ledgers fetches each on an interval. The cost of that is not
  measured yet.

## Potential future extension: a local cache index

A local process, or the binary itself, can populate a local index database from the
ledgers it fetches, so that a view over many ledgers reads one local store instead of
many git trees. This is an optimisation only. The ledgers stay the source of truth, and
the index is a projection that can be rebuilt from them at any time.

It is deferred until the full model and measured performance justify it. Cache
semantics, such as invalidation, staleness and the behaviour on a failed fetch, depend
on both.
