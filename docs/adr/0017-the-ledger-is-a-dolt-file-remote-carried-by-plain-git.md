# 17. The ledger is a Dolt file remote carried by plain git

Date: 2026-10-20

## Status

Accepted. Decided by Jim on 2026-10-20 after the review in
[2026-10-19-is-dolt-the-right-store.md](../arch/2026-10-19-is-dolt-the-right-store.md)
(Option 5) and the two spikes in
[2026-10-20-option-5-spikes.md](../arch/2026-10-20-option-5-spikes.md). It changes how the
Dolt data of a ledger moves between clones. ADR 0014 (Dolt is the store) and ADR 0016
(which ref holds the ledger) stand.

## Context

Until 0.20.0 the ledger ref held Dolt's own git blobstore. `dolt push` and `dolt fetch`
spoke to the git remote themselves. Each call ran many git processes and network round
trips, and a publish took 3.4 s through a 330 ms network shim. The cost came from the
round trips inside Dolt, not from the size of the data. tsk calls the publish at the start
and the pause of every thread, so the cost sat on the commands people wait for.

## Decision

1. **The ledger ref holds a Dolt file remote.** The tree of `refs/heads/tsk/ledger` or
   `refs/tsk/ledger` is a Dolt `file://` remote directory: `manifest`, the table files,
   and zero-byte `LOCK` files. It also holds `.tsk-transport` (the text `file-remote`) and
   a `.gitignore` that ignores `LOCK` and `oldgen/LOCK`.
2. **tsk keeps that directory as a linked worktree of the repository**, beside the ledger
   database and the documents worktree, at
   `${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger-remote`. The ledger
   store's Dolt remote `origin` is `file://<that directory>`. Dolt never starts git.
3. **tsk moves the directory with plain git.** A fetch is `git fetch` of the ledger ref,
   `git reset --hard` of the directory to the fetched commit, then `dolt fetch`. A publish
   is the documents push, then `dolt push` into the directory, a commit of the directory,
   and a plain `git push`. The push is never forced and carries no lease.
4. **A rejected push means origin moved.** The publish then repeats from the fetch:
   reset the directory to origin's tip, `dolt fetch`, the existing renumbering and
   `dolt merge`, `dolt push`, commit, `git push`. Conflicts stop the command with exit 1
   as before. The limit is 10 attempts, with a random wait of 50 to 250 ms between them.
   The limit was 5.
5. **The layout is named by the ref.** `.tsk-transport` marks a file remote. A ref without
   it holds the older layout, `git-remote`. A clone's own store names its transport by the
   URL of its `origin` remote: `file://` or `git+`. `.tsk-ledger.toml` belongs to the
   removed file ledger and plays no part. The layout version of the Dolt `manifest` table
   stays at 8: no table or column changed.
6. **A new ledger uses `file-remote`.** `TSK_LEDGER_TRANSPORT=git-remote` makes a new
   ledger use the older layout. The older code path stays as the fallback. A clone of an
   existing ledger takes the layout the ref holds.
7. **`tsk ledger migrate-transport` converts a ledger once.** See the migration below.
8. **Compaction is a tree-replacing normal commit.** `dolt gc` and a push into an empty
   directory give the same root hash in 3 files. A normal commit whose tree is that result
   replaces the old tree. Other clones see a pull that changes nothing. Nothing forces a
   ref. The spike measured it; tsk does not run it yet, because growth is small.

## Why plain pushes

The cloud proxy accepted a lease push in the spike, so a force was possible. A plain push
needs no force and no lease, so it works on every host, and a stale writer is rejected by
git itself. The cost is the retry loop, which the ledger already needed for the data.

## Growth

Each publish adds about 5 KB and 2 files to the ref, with or without a local `dolt gc`
(spike, on a copy of the live ledger). A 40-row publish added 7 KiB. Dolt conjoins the
directory above 256 files, which on local files is cheap. The branch grows slowly enough
that compaction is a later, scheduled job.

## Migration

`tsk ledger migrate-transport` runs in one clone:

1. It fetches the ledger into the store. It stops with a message that names
   `tsk ledger push` when the store holds unpushed commits, and names
   `tsk ledger migrate-docs` for a version 1 ledger.
2. It resets the directory to the ref's current tip and removes the old tree from it.
3. It pushes the history of the store into the directory through a temporary Dolt remote,
   adds the two marker files, and commits. The commit has the old tip as its parent.
4. It pushes that commit with a plain `git push`. The old content is replaced by a normal
   commit. If origin moved meanwhile, the push is rejected, nothing changed on origin, and
   the command is run again.
5. It points the store's `origin` at the directory and fetches.

Every other clone moves to the new layout on its next fetch that finds the ref changed. It
detects the marker, resets its directory to the commit, and points its store's `origin` at
it. It does not rebuild the store. Dolt commits have the same hashes on both sides, so the
local commits stay, and the next publish sends them.

## Consequences

- A publish costs one `git fetch` (skipped when the up-to-date check finds origin
  unchanged), a local `dolt push` of about 0.2 s, and one `git push`. The estimate in the
  review is about 0.9 s, from the measured parts. It is not timed end to end here.
- A binary older than 0.21.0 cannot read a migrated ref. It finds no git blobstore and is expected to
  stop with a Dolt error, not with "update tsk". (Not tested against an old binary.) The layout version did not change, so
  the version check cannot name the cause. Update every `tsk` (`tsk self-update`) before
  anyone runs `tsk ledger migrate-transport`. A binary older than 0.18.0 updates by hand.
- `tsk ledger migrate-docs` works only on a store that uses the older layout. A version 1
  ledger migrates its documents first.
- The ref holds one more kind of file than before, and a person who browses the branch on
  GitHub sees Dolt table files, as they did in the blobstore.
- A process that dies between `dolt push` and `git push` leaves the directory ahead of
  origin. tsk clears its record of the last fetched commit before the `dolt push`, so the
  next command fetches, resets the directory, and sends the push again.
- The live origin has not been used for this transport. Two writers that reset and push
  within the same second on the cloud proxy are untested, as are branches of 257 or more
  files.
