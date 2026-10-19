# Is Dolt the right store for tsk's ledger? Review, 2026-10-19

Written by Claude (Fable 5.1) at Jim's request. Every experiment ran on scratch copies
of the real ledger (a bare origin seeded from `refs/heads/tsk/ledger` and
`refs/heads/tsk/docs`, a fake session id, `XDG_STATE_HOME` and `HOME` inside the scratch
directory). Nothing was written to the live ledger or pushed to the real origin. The
binary is tsk 0.18.0 and `dolt` is 2.5.0 unless stated. Each external claim carries a
URL and a status: Verified (I read the source or ran it) or Not verified.

## Summary

- **A.** The remote holds many table files because each `tsk ledger push` adds two
  (one `.darc` archive with the new chunks and one 321-byte table holding the new root),
  and Dolt only merges them (conjoin) when the manifest names more than 256. The live
  remote is at 129, so conjoin has never run. Reproduced: 257 files became 3 at push 65
  on a scratch copy. `dolt gc` on a clone never changes the remote.
- **B.** Every `dolt fetch` opens every table file named in the manifest to read its
  index or footer, through two `git cat-file` processes per file, because the git
  blobstore has no ranged read and nothing caches a parsed index between processes.
  This is a design limit of the git blobstore, acknowledged in a TODO in the source,
  and it is partly mitigated upstream (2.2.x, 2.4.x) but not removed. tsk 0.18.0 already
  skips it when nothing is new.
- **C.** Multi-agent beads (Gas Town) runs **one `dolt sql-server` per machine**, and
  every agent is a client of it. The git remote is a backup channel pushed at session
  end. Beads' users see no problem because the git remote is not on their hot path.
  The users who do put it on the hot path report tsk's problems.
- **D.** Measured through a 330 ms per-operation shim: a Dolt publish costs 3.5 s and a
  thread start 3.8 s, of which about 3 s is nine network operations inside `dolt push`.
  The file count is no longer the cost: a conjoined remote of 43 files gave the same
  3.5 s. Plain git carrying JSONL costs 0.35 s per operation, but loses row merge for
  one-file-per-table layouts. A Dolt file remote carried on the branch by plain `git`
  keeps Dolt's merge and costs 0.2 s locally plus one git operation.
- **E.** Keep Dolt as the local store and the merge engine. Replace Dolt's git blobstore
  transport with tsk's own: a Dolt `file://` remote directory committed to
  `tsk/ledger` by `git`, one fetch and one push per publish. Adopt Jim's publish-less-often
  rule on top. Measure one supervised live run before cutting over. Do not move to JSONL.

## Evidence for A: why the remote holds so many files

Facts from the Dolt source at `main` (commit 72e0a00, 2026-10-19,
https://github.com/dolthub/dolt). Verified by reading the files named.

- A git remote is opened with `nbs.NewGitStore` (`go/libraries/doltcore/dbfactory/git_remote.go:255`),
  which builds the store with `singleBlobBSPersister` and `inlineConjoiner{defaultMaxTables}`
  (`go/store/nbs/store.go`, `NewGitStore`). `defaultMaxTables = 256` (`store.go:72`).
- `conjoinRequired` is `ts.Size() > c.maxTables && len(ts.upstream) >= 2`
  (`go/store/nbs/conjoiner.go:51`). Conjoin starts inside `updateManifestAddFiles`
  (`store.go:361` and `:459`), which is the step a push runs when it adds table files
  to the remote manifest. So the pusher runs conjoin, against the git remote, and only
  when the manifest names 257 or more files.
- A variant without conjoin exists (`NewNoConjoinGitStore`, `store.go:738`) and the
  factory does not use it.
- The git history of the branch is re-rooted every 64 commits:
  `maxParentedCommits = 64` (`go/store/blobstore/git_blobstore.go:803`) and
  `historyParent` returns no parent at that depth (`git_blobstore.go:1694`). The limit
  is `DOLT_GIT_REMOTE_MAX_HISTORY_COMMITS` (`git_remote.go:279`).

What was reproduced on a scratch copy of the live ledger (129 files, 64 commits,
971 KB). A writer clone ran 80 cycles of one `tsk task add` and one `tsk ledger push`
(`grow.sh` in Method):

| Push | Files on the remote | Git commits | Bytes |
|---|---|---|---|
| 0 | 129 | 64 | 971,335 |
| 1 | 131 | 2 | 980,173 |
| 33 | 195 | 2 (re-rooted at 64) | 1,275,917 |
| 64 | 257 | 64 | 1,556,544 |
| 65 | 3 | 1 | 1,535,244 |
| 80 | 33 | 31 | 1,648,557 |

- Every push adds exactly two files. I read one 321-byte file: its footer says 2
  chunks, and the chunks are an address map (`ADRM`, `refs/heads/main`, `STRT`), so it is
  the new root, and the `.darc` beside it holds the new data chunks. Verified by
  inspection of the blob.
- Conjoin triggered at push 65, when the manifest reached 257 files: 257 became 3 (one
  1.53 MB archive, one root table, the manifest). Growth then resumed at 2 per push.
  Conjoin concatenates, it does not collect garbage: the store grew from 971 KB to
  1.53 MB over 65 small pushes because every intermediate root is kept.
- `dolt gc` in the writer before each of 5 further pushes left the remote growing at 2 per
  push (35, 37, 39, 41, 43) while the local store stayed at 6 files. A local gc rewrites
  local files; the push sends only new chunks and never removes a remote file.
- The git re-root happened at push 33 (64 commits to 2), on its own, with the tree
  intact. The live branch sits at exactly 64 commits, so the next live push re-roots it.

Why Dolt does not keep the count lower: 256 is tuned for a local or S3 store, where
opening a table file is one cheap ranged read. On the git blobstore each open is two
processes (section B), so the threshold is 128 times too high for this transport. The
"+18 in a day" was 9 pushes.

The remote growth design (`docs/arch/2026-10-18-remote-growth-design.md`, Finding 3)
said "Does not shrink" for `gc` then push on the same branch. That is correct, and the
reason is now known: nothing shrinks a remote except conjoin at 257 or a fresh-ref swap.
Its Option C projection ("500 files is about three weeks away") is wrong: the count
cannot pass 257. The ceiling is 257 files, and the cost at the ceiling, on Dolt 2.5.0,
is about 514 `cat-file` launches per fetch that finds something new.

## Evidence for B: why a fetch re-reads every table file

Mechanism, from the source (Verified):

1. When Dolt loads a manifest it opens every table the manifest names
   (`go/store/nbs/table_set.go:460`, "Concurrently open all files that are not already"
   open). For a plain table `newBSTableChunkSource` reads the index and footer from the
   tail of the file; for an archive `newBSArchiveChunkSource` reads the footer
   (`go/store/nbs/bs_persister.go:375` to `:442`). That is what each file is read for:
   its chunk index, so the store can answer "which file holds this hash".
2. The git blobstore serves a ranged read as `git cat-file -s <oid>` then
   `git cat-file blob <oid>`, and slices the stream (`git_blobstore.go:1302`, `Get`,
   `getFromCache`, `sliceInlineBlob`). The TODO at `git_blobstore.go:1975` says: "This
   streaming implementation is correct but may be slow for workloads that do many small
   ranged reads (e.g. table index/footer reads). Consider caching/materializing blobs to
   a local file (or using a batched git cat-file mode)".
3. `RangeReadsWholeBlob()` returns true for the git blobstore (`git_blobstore.go:1294`),
   so the NBS layer spools each blob once to a temp file and serves ranges from it
   (`go/store/nbs/spooling_table_reader.go:33`). The temp file lives as long as the open
   chunk source and is removed at close. Nothing persists a parsed index across
   processes. The in-memory cache (`cacheObjects`, path to OID, built from one
   `git ls-tree -r -t`) is per process too.
4. The only persistent state is the fetch anchor ref
   `refs/dolt/remotes/origin/heads/tsk/ledger/last`, which fixes git negotiation (so a
   fetch moves few bytes), not the file reads.

Observed on a scratch clone with Dolt 2.5.0, a forced `tsk ledger fetch` with nothing
new (the `remote-tips` record deleted so tsk could not skip), every `git` launch logged by
the wrapper in Method:

| Remote | Files | `git cat-file` launches | Distinct blobs read | Time |
|---|---|---|---|---|
| Copy of live | 129 | 260 (129 `-s`, 129 `blob`, 2 manifest) | 129 | 1.3 s local |
| After conjoin | 33 | 68 | 33 | 0.73 s local |

Two launches per file: one size, one stream. The 1.88.2 figure of six per file in the
earlier document came from each archive being opened five times; 2.5.0 opens each once.

Upstream history (all Verified from the issue pages unless marked):

- https://github.com/dolthub/dolt/issues/10537: push of a 224 KB database took 45 to 80 s
  on 1.82.2; 15 s after 1.82.4.
- https://github.com/dolthub/dolt/issues/11087: a read-only fetch took 23 s with 772 to
  814 `cat-file` launches on 2.0.3; 3.6 s and 125 launches on 2.2.2; closed 2026-07-28.
  A contributor wrote that the launches "are reading lots of data which they are
  throwing away" and proposed a local disk cache with targeted reads.
- https://github.com/dolthub/dolt/issues/11916: a 56 MB table file took 40 minutes to
  pull with 127,753 git subprocesses; the maintainer attributed the fix to the July
  spooling change and closed it on 2026-09-30 after PR #11989 (in 2.4.x).
- https://github.com/dolthub/dolt/issues/11949: since 2.1.2 each process deleted its
  fetch ref, so a pull re-downloaded the whole database; fixed by PR #11991 (the `last`
  anchor) on 2026-10-01. The first pull after a 64-commit re-root still downloads the
  whole database.
- Release 2.5.0 (https://github.com/dolthub/dolt/releases/tag/v2.5.0, 17 Oct) lists
  #12061 and #12062 for git remotes (object format, FETCH_HEAD contention) and nothing on
  `cat-file`. Release 2.2.0 lists nothing on git remotes.
- `main` at 72e0a00 has a batched size read (`BlobSizes`, one `cat-file --batch-check`)
  for the parts of a chunked blob (`git_blobstore.go:1444`), not for the per-table open.
  Not verified whether 2.5.0 contains it; the observed 129 separate `-s` launches say the
  per-table path does not use it.

Verdict: a design limit, with the cheap part fixed. A table-index cache keyed by blob OID
on disk would remove the loop and is the fix to propose upstream. For tsk today it
matters only on a fetch that finds something new and inside every push (a push ran 113
`cat-file` launches against a 43-file remote), because T-09 skips it otherwise.

## Evidence for C: how beads runs with several agents

Sources read 2026-10-19. `beads.gascity.com` and `dolthub.com` are not reachable from
this sandbox (`ENOTFOUND`), so anything that rests on them is Not verified.

| Claim | Source | Status |
|---|---|---|
| Embedded Dolt is "Single-writer (one process at a time)" with a file lock; server mode is for "Multiple agents writing simultaneously" | https://github.com/gastownhall/beads/blob/main/docs/architecture/dolt.md | Verified |
| Sync is `bd dolt push` and `bd dolt pull`, manual, on `refs/dolt/data`; "The local Dolt database is the source of truth" | https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md | Verified |
| "Sync at session end - Run `bd dolt push` so other agents see your updates" | https://github.com/gastownhall/beads/blob/main/docs/multi-agent/coordination.md | Verified |
| `--claim`: "Atomically claim the issue (sets assignee to you, status to in_progress; idempotent if already claimed by you)"; "when multiple agents pull from the same ready queue, the first claim wins" | https://github.com/gastownhall/beads/blob/main/docs/CLI_REFERENCE.md and coordination.md | Verified |
| Gas Town: "All beads data is stored in a single Dolt SQL Server process per town", port 3307, "There is no embedded Dolt fallback"; polecats, refinery and crew use `.beads/redirect` files so "all agents in a rig share a single beads database via the Dolt server"; "all agents write directly to `main` using transaction discipline" | https://github.com/gastownhall/gastown/blob/main/docs/design/architecture.md | Verified |
| Gas Town's only named remote is DoltHub, for federation between towns | https://github.com/gastownhall/gastown (README) | Verified |
| Beads moved from SQLite plus JSONL in git to Dolt in February 2026, around v0.50; `bd migrate --to-dolt` removed in v0.58.0 | https://www.dolthub.com/blog/2026-04-02-restoring-beads-classic/ and https://pkg.go.dev/github.com/steveyegge/beads@v0.50.1 (search summaries) | Not verified |

So the claim is atomic because it is one SQL transaction on one server that every agent
on the machine shares. A claim is visible to the other agents at once. It is visible to
another machine only after `bd dolt push` and that machine's pull. The beads docs do not
say what happens when two machines claim the same issue before either pushes; by Dolt's
merge rules two writes to the same cell conflict at the second push.

What multi-agent users report (issue pages, Verified):

- https://github.com/gastownhall/beads/issues/4746 (open, p1): several Claude Code
  sessions on one shared store read data that was days stale; a bead closed by one
  session showed open to another for two hours. Suspected cause: a long-lived server
  connection at repeatable read never takes a new snapshot.
- https://github.com/gastownhall/beads/issues/5157 (open): nine sessions on one
  `dolt sql-server`, each write auto-pushed to a 117 MB DoltHub remote. Concurrent pushes
  failed with "checksum error" for 152 s. The reporter asks for a single designated
  pusher on an interval, because "Concurrent pushes racing on the remote's chunk
  store/manifest fits all the evidence".
- https://github.com/gastownhall/beads/issues/6117 (open, p1): against a remote server
  at 80 ms round trip, `bd create` fails 2 of 2 with a serialization conflict because the
  write transaction spans the whole command (108 round trips at loopback).
- https://github.com/gastownhall/beads/issues/6359 (open): a `git+ssh` remote reached
  2,259 table files and 10-minute pulls because conjoin at 256 failed on SSH connection
  limits; once it ran, 2,264 became 32.
- https://github.com/gastownhall/beads/issues/4770 (closed): the `cat-file` loop, fixed
  in Dolt 2.2.0. https://github.com/gastownhall/beads/issues/7082 (open): one `cat-file`
  per chunk on Windows exhausts memory.

Answer to Jim's question: yes. Beads' multi-agent deployments are one server per
machine, so the git remote is off the hot path and pushed at session end. tsk's
constraints (several machines, no server, HTTPS branches only) put the git remote on
every thread command. The beads users with the same constraints (#6359, #5157, #7082)
report the same growth and contention, and nobody has an answer beyond "update Dolt"
and "push less".

The earlier document's `What beads does` section
(`docs/arch/2026-10-18-remote-growth-design.md`) is right on mechanism and wrong on
one line: "beads has no remote compaction command and its own users hit unbounded
table-file growth (#6359)". The growth is bounded at 257 by conjoin; #6359's
unbounded case was conjoin failing on SSH limits.

## D: is Dolt still the right store?

Jim's goals, restated as the test: many actors on one ledger, cloud sessions over HTTPS
branches only, typed records through `tsk`, the action budget (write never waits for
origin, reads under 0.2 s, session start under 1.5 s, thread start and pause about 1 s),
and cheap in tokens against hand-edited Markdown.

Measured on 0.18.0 against a conjoined scratch origin (43 files) with 330 ms per network
operation, the shim that matched the live total within 1 % in the earlier document:

| Action | Time | Network operations | Note |
|---|---|---|---|
| `mission list` | 0.12 s | 0 | |
| `task add`, `task set ... done` | 0.31 s, 0.30 s | 0 | |
| `ledger status --local` | 0.27 s | 0 | |
| `ledger status`, `session-start` | 0.59 s, 0.59 s | 1 `ls-remote` | |
| `ledger push`, a change | 3.5 s | 4 fetch, 3 ls-remote, 2 push | 3.36 s at 129 files in the M-PERF results: the file count is not the cost |
| `thread start` | 3.8 s | 9 | |
| `thread pause` | 3.8 s (1.4 s local) | 9, 113 `cat-file` | |
| `thread resume` | 0.6 s | 1 | |

Nine network operations at 0.33 s are 3.0 s of the 3.5 s. Seven of the nine are
made by `dolt push` against its cache clone and the remote (three fetches, two
`ls-remote`, two pushes). That is the cost to remove, and no Dolt setting removes it.

### Option 1: Dolt as now

Gives: row and cell merge with a queryable conflict table, typed columns, enums and
foreign keys checked at write, `dolt log` and `dolt diff` per record, 0.08 to 0.13 s local
reads. Costs: 3.5 s per publish and 3.8 s per thread start or pause, live-equivalent;
growth to 257 files then a 1.5 MB archive that grows with every conjoin; a first fetch
after each 64-commit re-root that downloads the whole store (1 MB today). Several actors:
a stale push is rejected and merged, which works (M-PERF results, race test). Risk: none
new. Migration: none. It fails the 1 s thread budget by 3 s and will not reach it.

### Option 2: Dolt with the remaining mitigations

Compaction by a fresh-ref swap (3 files, measured in the earlier document), fewer
fetches, periodic re-root. Measured here: a conjoined 43-file remote changed the publish
from 3.4 s to 3.5 s. Compaction buys nothing on the push path, because the cost is
round trips inside `dolt push`, not files. It buys about 0.5 s on a fetch that finds
something new. Unproven for many actors: a forced branch update while another session
pushes (the lease covers it in theory; the cloud proxy's reaction is untested). Verdict:
not enough on its own.

### Option 3: git as the transport of a plain-text representation

Spike on the real data (`spike2.sh` in Method): all 16 tables exported as sorted JSONL
(268 KB, 571 rows), two layouts, 100 commits each, then fetch, push and merge tests.

| Measure | One JSONL file per table | One file per task row (161 files) |
|---|---|---|
| Branch after 100 commits and `gc` | 123 KB pack | 147 KB pack |
| `git fetch`, nothing new, 330 ms shim | 0.35 s | 0.35 s |
| `git pull` one commit, 330 ms shim | 0.37 s | 0.38 s |
| `git push` one commit, 330 ms shim | 0.37 s | 0.38 s |
| Two actors add one task each to one mission | **conflict** (adjacent lines) | clean |
| Two actors edit different tasks of one mission | clean | clean |
| Two actors change the same cell | conflict | conflict, whole row |
| Two actors mint the same new number | conflict | conflict (add/add), which is the right outcome until renumbering |

For comparison the Dolt scratch origin's pack was 3.73 MB after 85 pushes.

What is kept: one git operation per sync (0.35 s), a branch GitHub renders, `git log`
per row, a `git clone` that brings the ledger down, and the tsk commands unchanged on
the outside. What is lost: cell-level merge (a row file merges as a unit), typed
constraints and foreign keys as data (they become checks in the binary), `dolt diff` and
history queries, and the SQL seam. The one-file-per-table layout is unusable for many
actors: the common case, two actors adding to one mission, conflicts. The
one-file-per-row layout for the high-churn tables works. Migration: the export is a
script; the store is a new `LedgerStore` implementation of about the size of the files
backend removed in 0.16.0 (1,400 lines, ADR 0015), plus a merge step that resolves
add/add by renumbering. Risk: rebuilding what was deleted a day ago, and every write
path tested again.

### Option 4: Dolt locally, a git text log as the sync format

Beads classic was SQLite plus JSONL in git, and beads left it for Dolt in February
2026 (Not verified: `dolthub.com` is unreachable; the search summary of
https://www.dolthub.com/blog/2026-04-02-restoring-beads-classic/ says so, and a Rust
port `beads_rust` keeps the "classic" design). The hybrid would export JSONL at every
publish, merge text in git, and import on every fetch. Dolt's merge is then never used,
so it gives Option 3's semantics with two engines and an import on every fetch. Not
measured; not worth measuring.

### Option 5: Dolt as the store, plain git as the transport, under tsk's control

The mechanism: Dolt supports a `file://` remote, a directory holding a manifest and table
files. `tsk` keeps that directory as the content of `tsk/ledger` (a worktree beside the
documents one, exactly as `tsk/docs` is handled today), and moves it with its own
`git fetch` and `git push --force-with-lease`, one each. `dolt push` and `dolt fetch` then
run against a local directory and never spawn `git`. Spiked on the real data
(`filerem` in Method):

| Step | Time |
|---|---|
| `dolt push` to the file remote (after `gc`: 5 files, 676 KB) | 0.16 to 0.20 s |
| `git push` of the directory, one commit, 330 ms shim | 0.35 to 0.44 s |
| `git pull`, nothing new or one commit, 330 ms shim | 0.36 to 0.39 s |
| `dolt pull` or `dolt clone` from the checked-out directory | 0.15 to 0.19 s |
| Stale writer: `git push` rejected (non-fast-forward); `git pull` conflicts on `manifest`; reset to origin's tree, `dolt pull` merges the rows (0.17 s), `dolt push` (0.20 s), `git push` (0.39 s) | about 0.8 s plus one fetch |

A second reader then saw both rows. Projected key actions: publish about 0.9 s
(ls-remote skip, `dolt push` 0.2, `git push` 0.4), about 1.3 s when origin moved; thread
start and pause about 1.0 to 1.4 s; session start unchanged at 0.6 s. Kept: everything
Dolt gives (row merge, conflict table, types, history, local reads). Lost: Dolt's own
chunked transfer; each publish ships the changed table files whole. With a `dolt gc`
before each publish that is one 600 KB archive per publish, so the branch grows by about
0.5 MB per publish (509 KB pack after the spike's four publishes). Without `gc` it is
2 small files per publish and the same 257 conjoin, now cheap because opening a local
file is an `mmap`. The right rule is `gc` and a single archive on a schedule (the
fresh-ref compaction), small files otherwise. Several actors: the git-level conflict on
`manifest` is the signal that origin moved; resolution is the `dolt pull` merge tsk
already runs. Migration: a new transport in `fetch.rs` and `push.rs` (the documents
branch already has this shape), no schema change, no record migration, and every clone
re-roots once. Risk: the force-with-lease update on the cloud proxy is untested live;
Dolt's `file://` remote on a directory that git rewrites underneath it needs a test for
the manifest lock file.

### The publish-less-often variant, under each option

Jim's rule: record writes never push; publish at thread start, pause, session end and on
request. Beads does the same ("Sync at session end"). Effects:

| Option | Key actions with the rule | Many actors | Stale read and lost work |
|---|---|---|---|
| 1, Dolt as now | Record writes stay 0.3 s. Thread start and pause stay 3.8 s, now the only times the cost is paid | Unchanged: Dolt merges rows at the publish; same-cell edits conflict there. A claim is visible to others only after the start's publish and their next fetch (0.6 s check) | A container reclaimed before the pause loses every record since the start; a SessionEnd hook is best effort. Stale reads last until the next `ls-remote` |
| 2, with mitigations | Same | Same | Same |
| 3, git text | Publish 0.4 s, so the rule buys little | Row files merge per row; adjacent-row and same-number cases conflict at publish | Same loss window; the text is readable by hand from the container if it survives |
| 5, file remote on git | Publish about 0.9 s; thread start and pause about 1.2 s | Same as 1 | Same window |

The rule halves the number of publishes but not their cost. It makes Dolt's case
stronger, not weaker: fewer publishes means fewer chances for two sessions to collide,
and when they do, row merge and the renumbering at push (M-TIDY T-09) are what make the
collision cheap. Under a text transport the same rule leaves the add/add conflicts to
a merge driver that does not exist yet. The one risk the rule adds, for every option, is
the lost-work window. The cure is a cheap publish, so that the SessionEnd hook can afford
to run and a periodic background publish costs nothing a person notices.

## E: recommendation

Keep Dolt. The problem is not the store: it is that `dolt push` and `dolt fetch` over the
git blobstore spend nine network operations and two processes per table file on every
publish, and tsk's constraints put that on every thread command. The fix is to take the
transport away from Dolt: carry a Dolt `file://` remote on `tsk/ledger` with tsk's own
`git fetch` and `git push --force-with-lease`, the same mechanism `tsk/docs` uses today.
Then adopt the publish-less-often rule, and keep the fresh-ref compaction as the
scheduled `gc`.

| Action | Budget | Dolt as now | Dolt + mitigations | Git text (row files) | Dolt file remote on git (5) | Any option + publish less often |
|---|---|---|---|---|---|---|
| Mark a task done (write) | 0.3 s | 0.30 s | 0.30 s | about 0.1 s (est.) | 0.30 s | same |
| Add a task (write) | 0.3 s | 0.31 s | 0.31 s | about 0.1 s (est.) | 0.31 s | same |
| List missions | 0.2 s | 0.12 s | 0.12 s | under 0.1 s (est.) | 0.12 s | same |
| Start a session | 1.5 s | 0.59 s | 0.59 s | 0.35 s | 0.59 s | same |
| Publish (origin unchanged) | 1 s or background | 3.5 s | 3.5 s | 0.4 s | about 0.9 s (est. from parts) | paid at start, pause, end |
| Start or pause a thread | 1 s | 3.8 s | 3.8 s | about 0.8 s (est.) | about 1.2 s (est.) | same |
| Resume a thread | 1 s | 0.6 s | 0.6 s | 0.35 s | 0.6 s | same |
| Merge of two actors on one mission | | rows | rows | per row file; add/add on same number | rows | fewer merges |
| Migration | | none | a command | new store, export script, merge driver | new transport in fetch.rs and push.rs | a rule in the hooks |

Steps, in order:

1. Spike Option 5 inside `tsk` on a scratch origin: `tsk/ledger` becomes a worktree
   holding a Dolt file remote; `Session::refresh` does `git fetch` then `dolt fetch`
   from the directory; `push_data` does `dolt push` to the directory, `git commit`,
   `git push --force-with-lease`, and on rejection fetch, `dolt pull`, retry. Run the
   conformance suite and the race test. About two days.
2. Measure: publish and thread start through the shim, and one supervised live run of
   the lease push on the real origin, which also answers whether the proxy accepts the
   non-fast-forward update that compaction needs.
3. Cut over with one migration: the first 0.19.0 push writes the directory to the
   branch; older binaries stop on the missing git blobstore manifest, as they stop today
   on a newer layout.
4. Adopt the publish rule (T-08) with a background publish at pause and a SessionEnd
   hook, and set the `gc` schedule (a single archive at session end, small files otherwise).
5. File upstream: a per-OID table index cache for the git blobstore, citing #11087 and
   the TODO at `git_blobstore.go:1975`. It helps every beads user on a git remote and
   costs tsk nothing.

What Jim should decide: whether tsk may own the transport (step 1), the publish rule,
and the one supervised live run. What to measure before deciding: step 2's two numbers
(publish and thread start through the shim after step 1) against the 1 s budget, and
whether the lease push passes the proxy. If step 1 cannot bring a thread start under
about 1.3 s, the next candidate is Option 3 with row files, not a server.

## Method

Scratch root `$S`, built as in `docs/arch/2026-10-18-remote-growth-design.md`
(`Setup, repeatable`), with `HOME` also inside `$S` and `TSK_AUTO_UPDATE=never`. Dolt
2.5.0 first on `PATH`. The `git` wrapper `gw.c` is the earlier document's, extended to log
more verbs. Load on the machine: another agent ran `cargo test` during the timings, so
times are single runs or medians of two and carry noise of about 0.1 s; launch counts
are exact.

```bash
# A. growth: 80 cycles of task add + ledger push from clone c1; after each, count the remote tree
for i in $(seq 1 80); do tsk task add M-PERF "growth probe $i"; tsk ledger push "growth probe $i"
  git -C $S/origin.git ls-tree -r -l refs/heads/tsk/ledger | awk '{n++; s+=$4} END{print n, s}'
  git -C $S/origin.git rev-list --count refs/heads/tsk/ledger; done           # grow.sh, grow.csv
git -C $S/origin.git cat-file -p refs/heads/tsk/ledger:manifest | head -c 200
git -C $S/origin.git cat-file blob <321-byte blob> | tail -c 20 | od -An -tx1   # chunk count 2
for i in 1 2 3 4 5; do (cd $(tsk ledger path) && dolt gc); tsk task add M-PERF "gc $i"; tsk ledger push "gc $i"; done
# B. every git launch of a forced fetch with nothing new
find $XDG_STATE_HOME -name remote-tips -delete
GITWRAP_LOG=$S/lg.txt PATH=$S/bin2:$PATH tsk ledger fetch; awk '{print $3}' $S/lg.txt | sort | uniq -c
grep cat-file $S/lg.txt | awk '{print $NF}' | sort -u | wc -l
# Dolt source
git clone --depth 1 --filter=blob:none --sparse https://github.com/dolthub/dolt.git
git sparse-checkout set go/store/nbs go/store/blobstore go/libraries/doltcore/dbfactory
# D. key actions through the shim, clone c3 of the conjoined origin
GITWRAP_DELAY_MS=330 PATH=$S/bin2:$PATH tsk ledger push "x"      # and the other commands in the table
# D3. JSONL spike
dolt sql -r json -q "select * from <table> order by 1,2"          # per table, from a copy of the live store
bash $S/spike2.sh                                                  # two layouts, 100 commits, fetch, push, merge cases
# D5. file remote on a git branch
dolt remote add frem file://$F/remote-dir && dolt push frem main   # then git add/commit/push of the directory
dolt clone file://$F/w2 db2 ; dolt pull origin main                # reader
```

The live origin was read only: `git fetch origin refs/heads/tsk/ledger refs/heads/tsk/docs`
into `/home/user/tsk` (remote-tracking refs updated), `git ls-tree`, `git cat-file`, and
a copy of `$TSK_LEDGER_WT` for the export. The live ledger at the time: 129 files,
64 commits, 971 KB, last push 2026-10-18 08:51; 21 missions, 93 tasks, 67 decisions,
15 open questions.

## Not verified

- Whether Dolt 2.5.0 contains the batched `BlobSizes` read seen on `main`; the per-table
  path was observed not to batch.
- Beads' pre-Dolt design and the February 2026 switch: the DoltHub posts and
  `beads.gascity.com` are unreachable from this sandbox; the claims rest on search
  summaries.
- What a losing claim looks like in beads when two machines claim before either pushes.
- Any live time: every figure is a scratch origin with the 330 ms shim. The shim matched
  the live total within 1 % on 2026-10-18; the live origin has not been timed since
  0.18.0.
- Option 5's end-to-end times: the parts were measured, the sum is an estimate, and the
  lease push on the cloud proxy is untested.
- The M-PERF results document's `session-start` of 0.53 s was reproduced at 0.59 s;
  one run gave 1.31 s on a clone whose previous command was a push. Not investigated.
