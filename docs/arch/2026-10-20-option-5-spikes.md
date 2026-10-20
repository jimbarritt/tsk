# Option 5 spikes: lease push on the cloud proxy, Dolt file remote under git

Date: 2026-10-20. Two spikes named as untested in
`docs/arch/2026-10-19-is-dolt-the-right-store.md`, Option 5: Dolt as the store, a Dolt
`file://` remote directory carried as the content of the ledger branch, moved by `tsk`
with one `git fetch` and one `git push --force-with-lease`.

## Spike 1: force-with-lease on the cloud proxy

Live, against `origin`, on a throwaway branch `refs/heads/tsk/spike-lease`.

| Step | Result |
|---|---|
| Create the branch with one commit | accepted |
| Push an unrelated commit with a stale lease | rejected, `stale info` |
| Push the same commit with the correct lease | accepted, `forced update` |
| `git ls-remote` | the new commit |
| Delete the branch | refused by the proxy: `the remote end hung up unexpectedly` |

The proxy allows a non-fast-forward update of a branch when the lease matches, and git
rejects a stale lease before the proxy sees it. The proxy refuses a branch delete. The
branch `tsk/spike-lease` is left on `origin` and needs deleting from GitHub by hand.

## Spike 2: a Dolt file remote on a directory git rewrites

Local, Dolt 2.5.0. Two writers, A and B, each with a Dolt database and a git clone of a
bare origin on branch `ledger`. Each writer's clone directory is its Dolt remote.
Script: `scratchpad/sp2/run.sh`.

| Case | Result |
|---|---|
| A pushes, commits the directory, pushes the branch. B clones the branch, then `dolt clone` from it | B reads A's row |
| B adds a row, pushes to its directory, pushes the branch | 2 files added to the directory |
| A adds a row while stale, `git push` | rejected, `fetch first` |
| A: `git fetch`, `git reset --hard origin/ledger` (rewrites `manifest` and table files under the Dolt remote), `dolt fetch`, `dolt pull` | rows merged, 0.09 s; A has rows 1, 2, 3 |
| A: `dolt push`, commit, `git push` | accepted; B pulls and reads 1, 2, 3 |
| A pushes, then `git checkout -- . && git clean -fd` wipes the push from the directory | the next `dolt push` sends it again |
| Two `dolt push` into one directory at once | both exit 0; one uploads, the other reports up to date. A zero-byte `LOCK` file serialises them |
| `LOCK` removed by hand | recreated on the next push |
| Compaction: `dolt clone` the directory, `dolt gc`, push to an empty directory, replace the branch's tree with the result (3 files, same root hash) | B pulls: up to date; B pushes a new row: accepted; A resets to the new tree and pulls: 7 rows |

The directory holds `manifest`, the table files, a zero-byte `LOCK`, and `oldgen/` with
its own `LOCK`. `LOCK` and `oldgen/` can be ignored by git. A table file is named by its
hash, so git never sees a changed file, only added ones and a rewritten `manifest`.

### Growth, on a copy of the live ledger

Base after `dolt gc`: 3 files, 713 KB, git pack 506 KiB. Then one `questions` row per
push:

| Pushes | Files | Git pack | Directory |
|---|---|---|---|
| 0 | 3 | 506 KiB | 707 KB |
| 5 | 13 | 517 KiB | 727 KB |
| 8, the last 3 with `dolt gc` before each push | 19 | 522 KiB | 743 KB |
| 9, the last a 40-row insert | 21 | 529 KiB | |

Each publish adds 2 files and about 5 KB, with or without a local `gc`. A 40-row
publish added 7 KiB. The earlier estimate of 0.5 MB per publish after `gc` was wrong:
`gc` rewrites the local store, and the push still sends only the chunks the remote
lacks. Dolt conjoins the directory above 256 files, and on local files that is cheap.

## What this settles

- The lease push works on the cloud proxy. Branch deletes do not, which does not matter
  to the design.
- Git rewriting the remote directory under Dolt is safe in the cases run: Dolt reads the
  `manifest` on each command and holds no state between commands apart from `LOCK`.
- Compaction is a tree replacement with the same root hash. Other actors see it as a
  no-op pull.
- Growth is about 5 KB and 2 files per publish, so compaction is rarely needed.

## Not tested

- Two actors that each reset and push within the same second on the live proxy.
- A branch of 257 files on GitHub: the conjoin writes one large file and the git push
  then carries it whole.
- Recovery when a `git push` succeeds and the process dies before the local Dolt
  remote-tracking ref is updated.
