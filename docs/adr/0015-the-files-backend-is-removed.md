# 15. The files backend is removed

Date: 2026-10-18

## Status

Accepted. Supersedes decision 2 of ADR 0014, which kept a storage seam so the store could
be swapped back to the git-file ledger. The rest of ADR 0014 stands: Dolt is the ledger
store, it runs as an external binary, and git is the transport.

## Context

ADR 0014 made the git-file ledger the first implementation of the `LedgerStore` trait and
Dolt the second. The files backend was kept for two reasons: the seam stayed real, and the
store could be swapped back.

Two things changed by 2026-10-18:

- Jim decided that tsk is committed to Dolt (M-TIDY, question Q-01). The ledger of this
  repo moved to Dolt on 2026-10-12, and no ledger of any user holds live data in files.
- The 2026-10-17 architecture and performance review measured the price. Every feature
  since M-TSK-SCHEMA T-02 was built twice, once for each backend, with a conformance case
  for each. About a third of each task went on parity. The trait grew to 50 methods, and
  several name mechanism of the files store, such as `pending_inspect_hint`.

Every release up to and including v0.15.1 contains the files backend and is tagged. A user
who needs a file ledger can build or install that release.

## Options considered

| Option | What it is | Finding |
|---|---|---|
| A. Keep it first class | Every feature ships on both backends | Pays the parity cost on every task for a backend nobody runs |
| B. Freeze it as a reference | New features may skip it | Leaves two behaviours behind one trait, and a conformance suite that cannot run one set of cases against both |
| C. Remove it | Delete the store and everything only it used | The tags keep the code. The seam stays as a trait with one implementation |

## Decision

1. **The files backend is removed in 0.16.0.** The store (`store/files.rs`), `tsk ledger
   migrate-to-dolt`, the `[ledger] backend` setting and `TSK_LEDGER_BACKEND`, and the
   parts of `fetch.rs` and `push.rs` that only it used are deleted. `tsk ledger
   migrate-docs`, which moves the documents of a version 1 Dolt ledger to `tsk/docs`, stays.
2. **A `tsk` that finds the setting prints one line and exits 1.** If `[ledger] backend`
   in the user config, or `TSK_LEDGER_BACKEND`, is `git`, `tsk` prints `error: the files
   ledger backend was removed in 0.16.0; use tsk 0.15.1 to read an old file ledger`. Any
   other value is ignored.
3. **Release v0.15.1 is the last release that has the files backend.** The history of
   this repo's file ledger is on the branch `archive/tsk-ledger-files`.
4. **The `LedgerStore` trait stays.** It has one implementation, `DoltStore`, and
   the command modules still reach the ledger only through it. Trait methods that only the
   files store needed are deleted. Merging the trait's mechanism-named methods into fewer
   ones is a later job.
5. **The conformance suite runs each case once.** The cases check behaviour of the Dolt
   ledger. The `git` parameterisation and the cases that only the files store could run
   are deleted. A conformance case checks that a ledger held in a nexus repo
   (`ledgers/<id>` and `ledgers/<id>-docs`) round-trips between two clones, which was the
   one thing the files backend did for nexus ledgers that needed an explicit check.
6. **The worktree machinery stays where Dolt uses it.** The compare-and-swap push and the
   worktree fetch of `fetch.rs` and `push.rs` serve the documents branch `tsk/docs`, and
   stay with their tests, now run against that branch.

## Consequences

- A feature is built once. Its conformance case runs once.
- The beads backend that ADR 0014 named as a possible third implementation would be the
  second implementation. The trait is what keeps that open.
- A user with a file ledger must use tsk 0.15.1 to run `tsk ledger migrate-to-dolt`
  before updating. A `tsk` 0.16.0 that fetches a ref holding a file ledger stops and says
  so.
- `docs/domain/ledger-layout.md` no longer describes the file layout. It has a short
  section, "File ledger (removed)", that says what it was and where its history is.
- ADR 0014 is not edited. Its decision 2 is read together with this ADR.
