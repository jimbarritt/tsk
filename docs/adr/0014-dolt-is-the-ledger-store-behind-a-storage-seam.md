# 14. Dolt is the ledger store, behind a storage seam

Date: 2026-10-09

## Status

Accepted. Reverses the "Dolt is not adopted" decision in
`docs/domain/persistence-and-sync.md`. Amends ADR 0012: git stays the only shared
transport, and Dolt becomes the store that git carries. Amends ADR 0007: the markdown
projection is deferred, and Dolt replaces the NDJSON event log as the source of truth.
ADR 0010 stands: the ledger stays a branch.

## Context

The ledger is a git branch of Markdown and JSONL files, edited in the ledger worktree
and pushed by `tsk ledger push` with a compare and swap. It has no dependency beyond
git. Agents read `index.md` and the briefings straight from the worktree.

Two things changed on 2026-10-09:

- Jim stated the direction: move the ledger to a binary store, with markdown as a
  projection published for humans, and build tsk on platforms that already have
  momentum rather than compete at the storage layer.
- An experiment (`docs/kb/orchestration-ecosystem/beads-as-backing-store-analysis.md`)
  showed Dolt 1.88.2 pushing a database to this repository over git from a Claude Code
  cloud session, once its data ref was a branch under `refs/heads/*`. Two writers
  merged rows and cells, a stale push was rejected, and a same-cell edit stopped with a
  queryable conflict table.

`persistence-and-sync.md` rejected Dolt on 2026-09-14 for three reasons: a Go core
reached from Rust, SQL tables against an append-only NDJSON design, and a per-actor
log that avoids write contention. The first is answered by calling the `dolt` binary as
tsk already calls `git` (M-BOOT-04 T-01). The second is withdrawn by Jim's direction.
The third was a design, not a measured problem: the one recorded concurrency gap is a
missing lock on same-clone writes, which neither design fixes.

Beads was also considered as a substrate. It is a separate decision from Dolt, and it
is not taken now. The reasons are in `tsk-market-position-analysis.md` under
"Strategy: adopt Dolt, not beads".

## Options considered

| Option | What it is | Finding |
|---|---|---|
| A. Native Dolt behind a seam | A storage trait in the `tsk` binary. The git ledger stays one backend. Dolt is another, run as an external binary and synced over git on a `refs/heads/*` branch | Tested from the cloud. Keeps tsk's distributed model. Reversible through the seam |
| B. Beads for Navigation only | IDs, atomic claims and the ready queue from beads. tsk owns the rest beside it | Waits on a fork merging upstream. Two models in one store |
| C. Propose tsk's concepts to beads and BDP | A Mission, Objective or Report Type as a BDP proposal | Costs days. Tests whether beads' users want tsk's ideas. Not a storage decision |
| D. Run the token-saving experiment first | Decide the store after the four-dimension claim is tested | The prior question, and it does not block A |
| E. Full beads substrate | tsk as a layer over beads and BDP, "built on beads" | BDP serves reads only. One writer per Scope. Beads cannot push from a cloud session. tsk's concepts become JSON metadata in beads' schema |
| F. Keep the git ledger as it is | Markdown and JSONL files, merged by git | Zero dependency, readable in any git viewer. No row or cell merge, no query, no abstraction over remotes |

## Decision

1. **Dolt is the ledger store.** Missions, threads, continuation entries, bindings and
   external events are rows in a Dolt database. A mission briefing's body is a text
   column holding Markdown.
2. **A storage seam in the `tsk` binary.** One trait names every read and write the
   ledger supports. The current git-file ledger is the first implementation. Dolt is
   the second. The seam is kept so the store can be swapped back, and so a beads
   backend can be a third implementation later.
3. **Dolt runs as an external binary**, as `git` does. tsk does not link Dolt.
4. **Git stays the transport.** The Dolt database syncs to the repository's
   `tsk/ledger` branch, or a branch beside it, with `dolt remote add --ref
   refs/heads/<branch>`. No custom ref, no server, no account.
5. **No projection layer in the first step.** Agents read and write the ledger through
   `tsk` commands, which print Markdown and JSON. A projection for humans follows when a
   human needs one.
6. **Beads is not adopted.** Revisit when BDP writes and user-installed Types are in
   upstream beads, and after the token-saving experiment has run.

## Consequences

- A new mission, M-DOLT, builds the seam and the Dolt backend. Its first open question
  is scope: move every record at once, or move the JSONL records first and keep
  briefings as files until the seam holds both.
- The `SessionStart` hook installs the `dolt` binary as it installs `tsk`. Measured:
  a 44 MB download, 127 MB unpacked, 2.5 seconds in a cloud session.
- `index.md` and the briefings stop being files an agent opens. `CLAUDE.md`, the
  skills and the user guide change to name the `tsk` commands that print them.
- `tsk ledger push` becomes a Dolt commit and push with the same compare and swap. A
  same-cell conflict surfaces through Dolt's conflict tables, and the binary reports it.
- The ledger's history in git becomes Dolt's history. Git shows one commit per push,
  with opaque table files. `dolt log` and `dolt diff` show the record-level history.
- `docs/domain/persistence-and-sync.md` and `docs/domain/ledger-layout.md` are
  rewritten for the new store.
- The dependency on the `dolt` binary is the first dependency beyond git since ADR
  0012. The seam is what makes that reversible.
