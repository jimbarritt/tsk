# 35. The ledger state is in one home per repo, outside the code repo

Date: 2026-11-18

## Status

Accepted. Built in 0.70.0. It amends ADR 0009 (the documents worktree is a worktree of a
bare clone that tsk owns, not of the code repo, and not per clone), ADR 0016 (the names of
the nexus refs), ADR 0017 (the directory of the file remote), ADR 0018 (one Dolt server per
home), ADR 0025 (the hook scope is unchanged, the worktrees are elsewhere) and ADR 0034 (one lock
per ledger home, not per clone). ADR 0023 (a tsk older than the ledger updates itself) is
used unchanged. [The ledger layout](../domain/ledger-layout.md) has the rules and the
step lists.

## Context

A field test on a machine with a local nexus showed that tsk writes ledger data into the code repo:

- The documents directory (`docs`) and the file remote directory (`ledger-remote`) were
  detached linked worktrees of the code repo's clone. `git worktree list` in the code repo
  showed them.
- Every fetch wrote the ledger's git data into the code repo's object database, under
  `refs/tsk-fetch/*`.
- Each clone of a repo had its own copy of the ledger, in `repos/<repo name>-<clone id>`.
  Two clones, or a clone and a linked code worktree, on one machine had two stores that
  a push had to merge.

Jim decided to place both worktrees on a bare clone that tsk owns, to keep one copy of a
ledger per repo on a machine, and to make the upgrade automatic.

## Decision

### 1. A ledger home, outside the code repo

```
<state>/ledgers/<key>/
  .tsk-home      marker: layout, the nexus the home belongs to, the clones migrated in
  git/           tsk's bare clone: the ledger and documents git data and refs
  ledger-db/     the Dolt database
  ledger/        worktree of git/ with the Dolt file remote (was ledger-remote)
  docs/          worktree of git/ with the documents
  scratch/ push.lock ledger-ref remote-tips dolt-server.* remote-move.last ...
```

The code repo keeps `.git/tsk-clone-id`, `.git/tsk-repo-id` and the new
`.git/tsk-ledger-home`, the name of its home. It has no ledger worktree, no ledger ref
and no new ledger git data. Git data that an older tsk fetched stays in its object database
until git's own garbage collection drops them (two weeks after they are unreachable). tsk
does not run `gc` or `prune` in a repo it does not own.

### 2. Naming a home

- A ledger in a nexus: the nexus entry id. Every clone and every linked code worktree of the
  repo on a machine shares the home.
- The same entry id in a second nexus: `<id>~<hash8>`, where `hash8` is the first 8 hex
  characters of the SHA-256 of the identity of the nexus URL. `~` cannot appear in a repo id,
  so the names never collide. `.tsk-home` records the identity of the first nexus.
- A ledger in its repo: `_<clone-id>`. It has no identity that is stable across clones (an
  origin URL can be a local path), so its home stays per clone. `_` cannot start a repo id.

`.git/tsk-clone-id` stays as the identity of a clone inside the ledger (actor names, the
binding instance, `harnesses/<clone-id>`).

When the ledger of a clone resolves to a key other than the home the clone used, the home
that the clone used is inspected. Unpushed Dolt commits, uncommitted rows, uncommitted
documents or documents commits not pushed stop the command and name the way out
(`tsk nexus move-ledger`, or `tsk ledger discard`). A store with no rows and only its creation
commits, and a pushed home, are left as they are with a note.

### 3. One writer, one server, one working set per home

`push.lock` and the Dolt server of ADR 0018 belong to the home. Two clones of one repo
serialise their writers, and one server serves both. The Dolt working set is shared:
an uncommitted change in one clone is visible in the other, and `tsk ledger discard` in
either drops it. This is what one copy of the ledger means.

The per-clone files that git configuration names by absolute path stay in
`<state>/repos/<clone-id>/`: `harness-names`, `robot-signing-key`, `robot-allowed-signers` and
`hooks/`. `config.worktree` of every code worktree contains `user.signingkey` and
`gpg.ssh.allowedSignersFile` as absolute paths, so relocating them would break signing.

### 4. New ref names in a nexus

| | custom | branches |
|---|---|---|
| data | `refs/tsk/ledgers/<id>/ledger` | `refs/heads/ledgers/<id>/ledger` |
| documents | `refs/tsk/ledgers/<id>/docs` | `refs/heads/ledgers/<id>/docs` |
| old data | `refs/tsk/ledgers/<id>` | `refs/heads/ledgers/<id>` |
| old documents | `refs/tsk/ledgers/<id>-docs` | `refs/heads/ledgers/<id>-docs` |
| backup, while a migration runs | `refs/tsk/ledgers-backup/<id>/{ledger,docs}` | `refs/heads/ledgers-backup/<id>/{ledger,docs}` |

The `-docs` suffix was the way around git's rule that a ref and a folder of one name cannot
exist together. With both refs under `ledgers/<id>/` the rule no longer bites, and the
documents ref is named for its content. The refs of a ledger in its repo
(`tsk/ledger`, `tsk/docs`) do not change.

### 5. No tombstone ref

A ref at the old data name would stop an older tsk from creating a new ledger. Git cannot contain
`ledgers/<id>` and `ledgers/<id>/ledger` together: a push of the second while the first
exists fails with `cannot lock ref`. The old ref has to go. Tested on real bare
repos in both prefixes (git 2.43.0):

- One `git push --force-with-lease=<old>:<tip> <remote> :<old> <tip>:<new>` succeeds. The
  server deletes first and then creates. A stale lease changes nothing for that ref.
- `git push --atomic` with the same refspecs fails. tsk does not use it. Leases and creates
  apply per ref, so a push can succeed for one pair of refs and fail for the other.
- After the migration, a push that creates `ledgers/<id>` is rejected by the server in both modes.
  A push that creates `ledgers/<id>-docs` succeeds.

### 6. Older tsk binaries stop, then update themselves

- The nexus entry has a marker in `ledger_refs`:
  `<mode>-v2`, a newline, and `error: unsupported ledger version 16 in the nexus entry (ledger ref
  layout 2); this tsk supports up to ledger version 15. Update tsk`. An older tsk reads
  the value with a strict parse that accepts only `branches` and `custom`, so it fails before
  any ledger work, in every command, and embeds the whole value in its error. The second
  line starts as the unsupported-version error of ADR 0023, so the older tsk updates itself
  and runs the command again. With `TSK_AUTO_UPDATE=never`, and in the hook commands, it
  stops with the error. The new tsk reads only the text before the first newline.
- The number 16 is the generation of the ref layout of the entry. It is not a Dolt
  manifest version. The manifest stays at layout 15, `DOLT_SUPPORTED_VERSIONS` is
  unchanged, and the Dolt data has no change. A manifest bump would also have forced every
  older tsk that uses a ledger in its repo to update, and it would not have protected a
  nexus ledger, because an older tsk never reads the new refs.
- The server rule is a second protection. With `ledgers/<id>/ledger` present, no client can create
  `ledgers/<id>`. An older tsk that bypasses the marker (`TSK_LEDGER_REFS` in its
  environment returns before the entry is read) creates a local, unpublished ledger,
  and its push is rejected. The remote is never forked. A documents ref that it creates under the old
  name is found by `tsk ledger fetch`, merged into the new one and deleted.
- A new tsk does not create a ledger, or a first documents commit, while an old or a
  backup ref of that ledger exists on the remote.

### 7. The migration of the refs is per pair and best effort

It runs where the layout is not known: a store recorded under the old names, a clone with no
store and an entry with no marker, and after a failed migration. A marked entry, and a store at the
new names, skip it. It reads the remote with one `git ls-remote`, decides each pair (data,
documents) from the table below, and migrates what it finds:

| new | old | backup | action |
|---|---|---|---|
| yes | no | no | settled |
| yes | no | yes | delete the backup when the new ref contains it |
| yes | yes | any | documents: delete the old ref, or merge it first. Data: error |
| no | yes | no, or equal to old | migrate |
| no | yes | behind old (old contains the backup commit) | advance the backup to the old tip, then migrate |
| no | yes | diverges from old | stop, name both tips, change nothing |
| no | no | yes | resume: create the new ref from the backup |
| no | no | no | no ledger for this pair |

A migration pushes the backup first, then the old tip to the new name in one push with a lease
on the old tip, then deletes the backup. A server that rejects the combined push gets the
delete and the create one after the other. At every point one ref has the tip. A rejected
delete, an unreachable remote or a lost lease keeps the ledger usable on the old names for that
process, with one note. The migration repeats once an hour and at every `tsk ledger push`
(`remote-move.last` in the home). The marker is written last, when both pairs are at the
new names, and a failed write is retried at the next publish point. The data pair
is migrated before the documents pair.

### 8. The local migration is automatic and loses nothing

The first ledger command of the new tsk (`DoltStore::open`) finds a store of 0.69.0 or earlier in
`<state>/repos/<clone-id>/` and migrates it, with no network:

1. Stop the Dolt servers of the old and the new place.
2. Make the bare clone.
3. `rename` the Dolt database into the home (atomic; unpushed commits and the working set
   keep their state).
4. Take the documents and the file remote worktrees: import their git data with one
   `repack` through the object directory of the code repo as an alternate, add a worktree of the
   bare clone at the same commit, and copy every file over it, so uncommitted and unpushed
   documents stay as they are.
5. Rewrite the Dolt `origin` URL to the new `ledger` directory.
6. Rename `scratch` into the home. Drop the caches that tsk rebuilds.
7. **Verify** every copied worktree against its source, byte for byte. A difference stops the
   migration before anything is unregistered.
8. Rename the old directories into `repos/<clone-id>/legacy-leftovers/`. Remove only the
   administrative directories under the code repo's `.git/worktrees/` that point into the old
   state directory (a plain `git worktree prune` would also drop a worktree of the user's
   own that no longer exists). Delete `refs/tsk-fetch/*`.
9. Write the marker, the journal end and `.git/tsk-ledger-home`.

Nothing is deleted before its copy is verified, and the old directories are renamed, never
removed. Each step probes its own state, so a run that stopped is finished by the next one
(`TSK_TEST_MIGRATE_STOP_AFTER=<step>` exits after a step, for the tests). Three locks, always in
one order, make two processes safe: `<state>/migrate.lock`, the push lock of the old store,
the push lock of the home.

A second copy of one repo's ledger (two clones on one machine) is merged into the first:
both working sets are committed, the copy passes through a Dolt file remote into the home's
store, and `merge` runs through the renumbering of ADR 0022. A conflict, or a copy of a different
ledger, is set aside. In every case the copy is kept in `<home>/preserved/<clone-id>/`.
A second copy of the documents becomes a commit that is merged into the shared worktree.
A key that cannot be read (a store with no recorded ref) is not migrated by a path-only
command. The first command that has the ledger location finishes the migration.

### 9. The bare clone of a ledger in its repo

A bare clone has no `origin`. For a ledger in its repo, tsk sets `remote.origin.url` and
`pushurl` from the code repo (`git remote get-url`, so `url.*.insteadOf` applies, with a relative path
made absolute), and copies the effective values of `core.sshCommand`, `credential.*`,
`http.*` and `protocol.*`, read in the code repo so that `includeIf` blocks apply. A value
set for one command (`-c`, the environment) is not copied. `url.*` is not copied: the resolved
URL already includes the rewrite. The copied keys are listed in `tsk.copiedkeys`, and a key that the
repo drops is dropped. The copy is read again when a git configuration file of the repo or the user
changes in size or time. A setting outside this list does not follow.

## Consequences

- One copy of a ledger per repo per machine. Writers in two clones, or in a clone and a
  linked code worktree, share one lock, one server and one working set.
- `git worktree list` in the code repo lists no ledger directory. `git for-each-ref` lists no
  ledger ref. The code repo's object database contains no new ledger git data.
- `tsk ledger path` creates nothing unless there is state of 0.69.0 or earlier to migrate.
- Running two tsk versions on one machine is not supported: an older tsk starts a separate
  copy in `repos/<clone-id>` for a ledger in its repo.
- A ledger in its repo that `tsk nexus move-ledger` copies into a nexus has its home renamed
  to the id of the entry, and its store points at the new names.
- A tsk that updates itself because of the marker needs the release that has the new names. The
  cloud `SessionStart` hook installs it when `plugin/tsk-version` names it.

## Rejected alternatives

- **A tombstone ref at the old name.** Impossible: git cannot contain both names.
- **A Dolt manifest bump to layout 16.** Forces an update on every user of a ledger in a repo for
  nothing, and does not protect a nexus ledger.
- **Relocating the signing key and the signers file into the home.** Breaks `config.worktree` of
  every code worktree. Rewriting all of them edits user git configuration in many places.
- **Failing a migration that the server rejects.** A cloud proxy or a protected branch can reject a
  delete. The ledger keeps working on the old names, and a machine with direct access finishes the migration.
- **One shared home for all nexus URLs.** A second nexus with the same entry id gets its own
  home, so a changed URL cannot mix two ledgers.
