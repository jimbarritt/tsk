# Ledger layout

Reference for the tsk ledger: the Dolt backend (the default) and, for the `git` backend, every file in it: where the ledger lives, the ref that holds it,
where it is checked out, its tree, and the format and fields of each file. The scripts
in `ops/local/` defined these formats before this document existed. This document is
taken from them, and `tsk ledger fetch`, `tsk ledger push`, the `tsk thread` commands
and the external event commands (M-BOOT-04, T-03 to T-07) build against it.

Depends on: `docs/domain/session-continuation-design.md` (threads, bindings and
continuation state), `docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md`,
`docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md`,
`docs/adr/0010-ledger-stays-a-branch-not-a-directory-on-main.md`.

## Contents

- [Dolt backend](#dolt-backend)
- [Mission commands](#mission-commands)
- [Task, decision and question commands](#task-decision-and-question-commands)
- [File layout (`git` backend)](#file-layout-git-backend)
- [Location and ref](#location-and-ref)
- [Ledger worktree location](#ledger-worktree-location)
- [Tree](#tree)
- [`.tsk-ledger.toml`](#tsk-ledgertoml)
- [`index.md` and `future-missions-tbd.md`](#indexmd-and-future-missions-tbdmd)
- [`missions/`](#missions)
- [`threads/`](#threads)
- [`external-events/`](#external-events)
- [Local-only files](#local-only-files)
- [Common value formats](#common-value-formats)
- [Fetching and printing the path](#fetching-and-printing-the-path)
- [Writing to the ledger](#writing-to-the-ledger)
- [External event commands](#external-event-commands)
- [Session start command](#session-start-command)
- [Script outputs](#script-outputs)
- [Differences from `tsk/bootstrap`](#differences-from-tskbootstrap)

## Dolt backend

The default ledger backend stores the ledger's data in a Dolt database and its documents
as Markdown files on a second branch, both synced over git. The decision is in
[ADR 0014](../adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md). The rest of
this document, from [Location and ref](#location-and-ref) on, describes the file layout of
the `git` backend. The Dolt backend keeps the same records in tables, and the same ledger
refs, nexus rules and local-only files apply to both. The documents are the exception:
they are files on the branch `tsk/docs` (see [Documents](#documents)).

### Backend selection

A `LedgerStore` trait in the `tsk` binary names every read and write the ledger supports.
It has two backends: `dolt` (the default) and `git` (the file ledger).

- The user config, `${XDG_CONFIG_HOME:-$HOME/.config}/tsk/config.toml`, selects the
  backend:

  ```toml
  [ledger]
  backend = "dolt"
  ```

  The accepted values are `"dolt"` and `"git"`.
- The environment variable `TSK_LEDGER_BACKEND` overrides the config. Its accepted values
  are the same.
- With neither set, the backend is `dolt`.

### Location

The Dolt database is a `dolt clone` at:

```
${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger-db
```

- `tsk ledger path` prints this path. `$TSK_LEDGER_WT` names it.
- The clone has one remote, added with `--ref refs/heads/tsk/ledger` for an in-repo
  ledger, or `--ref refs/heads/ledgers/<repo-id>` for a nexus ledger. Its URL is
  `git+<remote url>`, with the remote URL resolved as in
  [Location and ref](#location-and-ref).
- The database needs the external `dolt` binary, version 1.88.2 or later. The `SessionStart` hook
  installs it.
- No file in `ledger-db` is edited by hand. The `tsk ledger` commands are the interface.
- The documents have their own worktree, `docs`, beside `ledger-db` (see
  [Documents](#documents)).

### Tables

| Table | Columns | Content |
|---|---|---|
| `manifest` | `k` varchar(64), primary key; `v` text | The `.tsk-ledger.toml` keys as rows, for example `version` and `repo_id`. The `version` row is `8`. |
| `threads` | `thread_id` varchar(64), primary key; `briefing_path` varchar(1024), null; `mission_id` varchar(64), null | One row per thread. `mission_id` is the mission the thread works, and is null for a thread with no mission. `briefing_path` is the mission's briefing document, stored only when the mission has no record. Together they replace `threads/<thread-id>/index.md`. There is no foreign key from `mission_id` to `missions`, because a thread made before mission records names a mission with no record. A thread made before version 5 has a null `mission_id`: the reader derives it from the file name of `briefing_path` (`M-BOOT-04` from `M-BOOT-04-official-ledger.md`) and never writes it back. |
| `continuation_entries` | `thread_id` varchar(64); `seq` int; `entry` longtext. Primary key (`thread_id`, `seq`) | One row per continuation state entry, `seq` from 1. `entry` is the JSON line, in the format of [`continuation-state.jsonl`](#threadsthread-idcontinuation-statejsonl). |
| `bindings` | `actor` varchar(255), primary key; `kind` enum(`cloud`, `worktree`); `thread_id` varchar(64); `registered_at` varchar(32) | One row per bound actor, of either kind. The actor key of a cloud session is the session ID (`cse_...`). The actor key of a local code worktree is `<clone-id>/<worktree-name>`: the ID in `.git/tsk-clone-id`, a slash, and `main` for the main worktree or the name of the linked worktree. A bind sets or replaces the actor's row. `thread stop` deletes every row that names the stopped thread. There is no foreign key to `threads`. It replaces `threads/lookup-by-cloud-session.json` and the `tsk-thread-id` marker files. A ledger below version 8 holds the cloud rows in `cloud_session_bindings` (`session_id`, `thread_id`, `registered_at`) instead. |
| `external_events` | `position` bigint, primary key; `envelope` longtext | The external event queue, `position` from 1. `envelope` is the JSON line, in the format of [`queue.ndjson`](#external-eventsqueuendjson). |
| `watermark` | `id` int, primary key; `processed_through` bigint; `updated_at` varchar(32) | One row, `id` 1, the processed position of the queue. No row reads as `0`. It replaces `watermark.json`. |
| `missions` | `id` varchar(64), primary key; `name` varchar(255); `category` enum(`operational`, `administrative`); `status` enum(`todo`, `in-progress`, `done`), default `todo`; `parent` varchar(64), null, foreign key to `missions(id)`; `purpose`, `decision_authority` longtext, null; `next_objective`, `next_task`, `next_question`, `next_constraint`, `next_out_of_scope` int, default 1 | One row per mission. Empty text is stored as null. Each `next_` column is the number the next objective outcome, task, question, constraint or out-of-scope item gets. It only grows, so a number is never reused. A ledger below version 7 has two more columns, `constraints_text` and `out_of_scope` (longtext, null), in place of the last two counters. |
| `mission_constraints` | `mission_id` varchar(64), foreign key to `missions(id)`; `seq` int; `text` longtext. Primary key (`mission_id`, `seq`) | One row per constraint of a mission, one line each. |
| `mission_out_of_scope` | `mission_id` varchar(64), foreign key to `missions(id)`; `seq` int; `text` longtext. Primary key (`mission_id`, `seq`) | One row per out-of-scope item of a mission, one line each. |
| `objective_outcomes` | `mission_id` varchar(64), foreign key; `seq` int; `outcome` longtext; `met` boolean, default false. Primary key (`mission_id`, `seq`) | One row per objective outcome. |
| `mission_blocks` | `mission_id` varchar(64), foreign key; `blocked_by_id` varchar(64), foreign key to `missions(id)`. Primary key of both | One row per block. Whether a block still holds is derived, never stored. |
| `mission_documents` | `mission_id` varchar(64), foreign key; `path` varchar(1024); `role` enum(`intelligence`, `report`, `design`). Primary key (`mission_id`, `path`) | One row per document a mission links. The path names a document on `tsk/docs`. |
| `tasks` | `mission_id` varchar(64), foreign key to `missions(id)`; `seq` int; `description` longtext; `status` enum(`todo`, `in-progress`, `done`), default `todo`. Primary key (`mission_id`, `seq`) | One row per task. The ID `T-nn` is `seq` zero padded to two digits. |
| `task_blocks` | `mission_id` varchar(64); `task_seq` int; `blocked_by_seq` int. Primary key of all three. Foreign keys (`mission_id`, `task_seq`) and (`mission_id`, `blocked_by_seq`) to `tasks(mission_id, seq)` | One row per task block. Both tasks belong to one mission. Whether a block still holds is derived, never stored. |
| `decisions` | `mission_id` varchar(64), foreign key to `missions(id)`; `seq` int; `text` longtext; `decided_by` varchar(255); `decided_on` date; `task_seq` int, null; `adr_path` varchar(1024), null. Primary key (`mission_id`, `seq`). Foreign key (`mission_id`, `task_seq`) to `tasks(mission_id, seq)` | One row per decision. `seq` orders the decisions and has no label. `decided_on` is the UTC date of the command. `adr_path` is the path of an ADR in the code repository, relative to its root, or null. It has no foreign key: the ADR is a file in another repository's tree. |
| `questions` | `mission_id` varchar(64), foreign key to `missions(id)`; `seq` int; `text` longtext; `status` enum(`open`, `answered`), default `open`; `answer` longtext, null; `answered_by` varchar(255), null; `answered_on` date, null; `asked_on` date. Primary key (`mission_id`, `seq`) | One row per question. The ID `Q-nn` is `seq` zero padded to two digits. |

There is no `documents` table. A version 1 ledger had one, and
[`tsk ledger migrate-docs`](#moving-the-documents-out-of-dolt) moves its rows to files.

The binary writes every text value as a hex literal, `CONVERT(X'...' USING utf8mb4)`,
and sends SQL to `dolt sql` on stdin. It sets two environment variables on every `dolt`
call:

- `DOLT_DISABLE_EVENT_FLUSH=1`, so Dolt sends no telemetry.
- `DOLT_REMOTE_INFO_BRANCH` to an empty string. Without it, each `dolt push` to a git
  remote also writes a branch `__dolt_remote_info__` holding one file, `DOLT_REMOTE.md`,
  with the data ref, its head commit and a timestamp. No tsk command reads that branch.
  The variable is not in Dolt's documentation. Measured with Dolt 1.88.2: a name sets
  the branch's name, and an empty string writes no branch. The conformance suite checks
  that a push leaves no branch on the remote other than the ledger ref.

`git.ledger.commit` in a continuation entry holds the Dolt commit hash with the Dolt
backend.

### Documents

Documents are plain Markdown files on a second branch, `tsk/docs`. The rule: structured
data goes in Dolt as typed rows, and free text goes on `tsk/docs` as files. Dolt owns the
whole tree of the ledger branch, and each `dolt push` writes a git tree of Dolt's own
files. A Markdown file on that branch would be dropped at the next push, so the documents
need a branch of their own. Any tool can open them: GitHub, an editor, `marq`.

| Item | Value |
|---|---|
| Ref, in-repo ledger | `refs/heads/tsk/docs` on the same remote as `refs/heads/tsk/ledger` |
| Ref, nexus ledger | `refs/heads/ledgers/<repo-id>-docs` on the nexus remote |
| Local checkout | `${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/docs`, a detached linked worktree of the managed clone |
| First commit | An orphan commit holding `README.md` ("Documents of the tsk ledger. Written by tsk ledger doc put.") and the seed `index.md`, made with git plumbing |
| Manifest | None. The documents branch has no `.tsk-ledger.toml` |

The nexus ref ends in `-docs` and not `/docs`. Git cannot hold both `ledgers/<repo-id>`
and `ledgers/<repo-id>/docs` as refs, because one would be a file and the other a
directory.

The worktree, the refresh and the compare-and-swap push are the ones the `git` backend
uses for a whole ledger, run with the documents ref. The path rules are those of a file in
git: no absolute path, no `..`, no `.git` component. The reserved names of the file ledger
(`threads/`, `external-events/` and the manifest) do not apply here.

Five commands read and write the documents:

| Command | Effect |
|---|---|
| `tsk ledger doc get <path>` | Prints the body of the document at `<path>`. |
| `tsk ledger doc put <path> [<file>]` | Stores the content of `<file>`, or stdin when no file is given, as the document at `<path>`. Writes the documents worktree only. |
| `tsk ledger doc list [<prefix>]` | Prints the document paths, limited to those under `<prefix>` when given. |
| `tsk ledger doc rm <path>` | Removes the document. Writes the documents worktree only. |
| `tsk ledger doc path <path>` | Prints the absolute path of the document's file, for opening it in another tool. Exits 1 with `error: no such document: <path>` when it is absent. |

With the `git` backend, `doc path` prints the file in the ledger worktree. The other four
commands behave the same with both backends.

`doc put` and `doc rm` do not reach the remote. `tsk ledger push "<message>"` does.
`tsk thread start` takes a mission ID, or none. It refuses an ID that has no mission record. With no ID it mints a thread with no mission. `tsk thread set mission` applies the same check. A ledger from before the mission records also accepts a briefing document at `missions/<category>/<ID>-<slug>.md`.

`tsk ledger fetch` refreshes both stores: the Dolt database, then the documents
worktree. It creates the worktree when it is absent. It refuses a worktree with
uncommitted changes, as it does for the database, and notes a local commit that is not on
the remote. It still prints the path of the Dolt database.

### Push

`tsk ledger push "<message>"` with the Dolt backend:

1. Runs `dolt add -A`, then `dolt commit` with the caller's message when anything is
   staged. With nothing staged it prints a note and continues.
2. Pushes the documents. It commits the changes in the documents worktree, then pushes
   `HEAD` to the documents ref with a compare and swap, and rebases onto a newer remote
   tip, up to 5 attempts. A rebase conflict exits 1 and names the files both sides
   changed. A side that has no change and no unpushed commit sends nothing.
3. Makes up to 5 attempts at the Dolt data:
   1. `dolt fetch origin`. If `HEAD` already equals `origin/main` or is its ancestor,
      the remote holds this state: exit 0.
   2. `dolt merge origin/main`, then read `dolt_conflicts` and
      `dolt_constraint_violations`. Any row aborts the merge with `dolt merge --abort`,
      and the command exits 1 and names the conflicting tables.
   3. `dolt push origin main`. A rejected push prints a note and goes to the next
      attempt. Any other failure stops with exit 1.

The documents go first, and the Dolt data only after them. A record must never name a
document the remote lacks. A document that no record names is harmless. So when the
documents push fails, because the remote rejects it after the retries or the rebase
conflicts, `tsk ledger push` exits 1 and does not push the Dolt data. Both sides keep
their local commits, and the next `tsk ledger push` resumes.

The local commit stays in the database after a conflict or a failure, and a later
`tsk ledger push` sends it. Dolt merges by row and cell, so two writers that change
different rows, or different cells of one row, do not conflict. Two writers that append
an external event at the same moment take the same `position` and conflict, as two
appends to `queue.ndjson` conflict in a rebase.

On success, stdout carries the Dolt commit hash, or the documents commit when only the
documents changed. Notes go to stderr.

### Status and discard

`tsk ledger status` prints what the local store holds that origin does not. It is
read-only. It runs a fetch of the ledger and documents refs (`dolt fetch origin`, and a
`git fetch` of the documents ref) and never merges, resets or changes the working set. It
exits 0 whatever it finds. When the ledger store does not exist, it exits 1 and names
`tsk ledger fetch`.

By default it prints two answers, `Local` and `Origin`, with the labels padded to the same
width. The output has no tab and no trailing whitespace. Everything in step:

```
Local   nothing to push
Origin  up to date
```

Changes not pushed, origin ahead:

```
Local   3 changes not pushed
          data  missions  1 added
          data  tasks     1 added
          docs  index.md  modified
Origin  2 data commits and 1 document commit to fetch (tsk ledger fetch)
```

- The `Local` headline counts the detail lines: `nothing to push`, `1 change not pushed`
  or `N changes not pushed`. A detail line is the store (`data` or `docs`), the name (a
  table, or a file path) and what changed, in columns aligned across all detail lines. A
  table prints its non-zero counts as `N added`, `N modified` and `N deleted`, joined by
  `, `; `schema changed` stands alone for a table whose rows did not change and follows the
  counts when both did. A document prints `added`, `modified` or `deleted`.
- A schema upgrade has its own detail line. A write on a ledger below the newest layout
  upgrades it, which changes the `manifest` row and the schema of some tables. When the
  local manifest version differs from the version on origin (as of the last fetch), the
  first detail line is `data  upgrade  schema upgraded to version N (origin has M)`. The
  `manifest` line and every table line whose only change is its schema go. A table whose
  schema changed keeps its line only for rows that differ in a column the table had before,
  so a row the upgrade touched is not counted as an edit. The upgrade line counts as one
  change in the headline. Nothing is labelled when origin holds no ledger yet, or when the
  versions are the same.
- Commits made and not pushed (ahead) are counted over data and documents together. After
  the changes they print as `        plus N commits not pushed` (8 spaces); with no changes
  the headline is `Local   N commits not pushed`.
- The `Origin` line is one of: `up to date`; `N data commits and M document commits to
  fetch (tsk ledger fetch)`; `N data commits and M document commits not on origin (tsk
  ledger push)`; `diverged: N data commits to push, M to fetch (tsk ledger push merges
  them)` (the parts are named in full when more than one kind of commit is involved); `not
  published yet (tsk ledger push creates it)`; `unknown (could not reach origin)`. A part
  with a zero count is left out, and one commit is singular. The `git` backend keeps data
  and documents on one branch, so it counts the commits once and calls them `ledger
  commits`.
- When the fetch fails, one line on stderr says so (`note: could not fetch ...; reporting
  from what is local`), and `Origin` says `unknown`.

`tsk ledger status --porcelain` prints one fact per line, tab-separated where a line has
columns, with no header. This is the stable form for scripts and agents:

```
data	clean
docs	clean
data	remote	up to date
docs	remote	up to date
```

With local changes:

```
data	local changes
data	missions	added 1	modified 0	deleted 0
data	tasks	added 1	modified 0	deleted 0
docs	local changes
docs	M	index.md
docs	A	notes.md
docs	D	missions/operational/M-OLD.md
data	remote	ahead 1
docs	remote	up to date
```

In the porcelain form:

- A schema upgrade prints `data<TAB>upgrade<TAB>schema upgraded to version N`, first among
  the `data` lines, with no origin version.
- `data` and `docs` each print `clean` or `local changes`, then one line per change. A
  Dolt table line is `data`, the table, and the counts of rows added, modified and
  deleted. A table whose schema changed but whose rows did not prints `schema changed` in place of the counts (`data<TAB>missions<TAB>schema changed`); one that changed both prints the counts, then `schema changed`. A document line is
  `docs`, `A`, `M` or `D`, and the path, sorted by path.
- A change (in both forms) is a difference between the working state and the merge base of the local and
  remote branches. A local commit that origin lacks is listed as well as an uncommitted
  change. What only origin changed is not listed.
- `data<TAB>remote` and `docs<TAB>remote` count commits of the local committed state
  against origin: `up to date`, `ahead N`, `behind N` or `diverged ahead N behind N`. A
  local commit that is not pushed counts as ahead. `unknown` means origin could not be
  fetched. `unpublished` means origin holds no such branch yet.
- When the fetch fails, one line on stderr says so, as above. Changes are then read from the remote-tracking state the last
  fetch left for the data (none for the documents: they compare with `HEAD`), and both
  `remote` lines say `unknown`.

The `git` backend keeps data and documents on one branch. `status` sorts the changed files
into `data` (`records/`, `threads/`, `external-events/` and `.tsk-ledger.toml`) and `docs`
(everything else), prints a file line for each (`data<TAB>A<TAB>records/missions/M-X.json`),
and prints the same remote state for both.

`tsk ledger discard` drops every local change that is not pushed, in the data and the
documents, so a trial mission can be undone without `dolt reset`. It is destructive.

- Without `--yes` it changes nothing. It prints the `Local` block of `status` (the headline
  and detail lines, no `Origin` line) and exits 1 with
  `error: this would discard the local changes above. Rerun with --yes to discard them.`
  With `--porcelain` it prints the `data` and `docs` lines of the porcelain form instead
  (no `remote` lines). When both sides are clean it prints `Local   nothing to push` (or the
  two `clean` lines with `--porcelain`) and exits 0.
- With `--yes` it fetches both remote states first. If a fetch fails it exits 1 with
  `Nothing was discarded` and changes nothing. Otherwise, with the Dolt backend, it runs
  `dolt reset --hard origin/main` and `dolt clean` (which drops a table that was never
  committed), and in the documents worktree `git reset --hard <fetched tip>` and
  `git clean -fd`. The `git` backend resets its one worktree the same way. When origin
  holds no such branch, the reset target is the local `HEAD`. It prints `discarded`.
- It does not touch thread state that lives outside the ledger: `tsk-thread-skip` in the
  git directory. A binding is a ledger row, so `discard` drops a binding that is not
  pushed, like any other local change.

Both are methods on the storage seam (`LedgerStore::status` and `LedgerStore::discard`).

### Manifest version

The `version` row of the `manifest` table is `8` for a ledger whose bindings, of cloud
sessions and of code worktrees, are rows of `bindings`, `7` for a ledger whose constraints
and out of scope are tables of their own with the bindings still in `cloud_session_bindings`, `6` for a ledger whose `decisions` table has
`adr_path`, `5` for a ledger whose `threads` table has
`mission_id`, `4` for a ledger with the mission, task, decision and question tables and the
older `threads` table, `3` for a ledger with the mission tables only, and `2` for a
ledger whose documents are files on `tsk/docs` and that has no mission tables. The Dolt
backend accepts versions 1, 2, 3, 4, 5, 6, 7 and 8. Version 1 is a ledger that still keeps its
documents in a `documents` table.

- A version 1 ledger is refused by every command except `tsk ledger fetch`,
  `tsk ledger path` and `tsk ledger migrate-docs`. The refusal reads: `error: this ledger
  stores its documents in Dolt (version 1). Run tsk ledger migrate-docs to move them to the
  tsk/docs branch.` `tsk ledger fetch` refreshes the database only and prints a note.
- A version 2 ledger works for everything it did before. The mission read commands behave
  as an empty mission set and write nothing. The first mission write command creates the
  mission, task, decision and question tables, adds `mission_id` to `threads` and makes
  `briefing_path` nullable, and sets the `version` row to `8` in the same local Dolt
  change as the mission, so `tsk ledger push` publishes both. The tables are created in
  their newest shape.
- A version 3 ledger works for everything it did before. The task, decision and question
  read commands behave as if the mission had none, and write nothing. The first write
  command of any of the four kinds (`mission`, `task`, `decision` or `question`) adds the
  columns `next_task` and `next_question` to `missions`, creates the four new tables, alters
  `threads` as above and sets the `version` row to `8` in the same local Dolt change as the
  record, so `tsk ledger push` publishes both. A write that fails a check upgrades nothing.
- A version 4 ledger works for everything it did before. `thread list` derives each
  thread's mission ID from its briefing path. The first write that needs the new column
  alters `threads` and sets the `version` row to `8` in the same local Dolt change:
  `tsk thread start`, `tsk thread set mission`, and a mission, task, decision or question write.
  Reads, `thread skip` and a `thread pause` upgrade nothing, and neither does a write that is refused.
  A version 2, 3, 4, 5 or 6 ledger takes the whole chain of changes up to version 8 at its first
  such write.
- A version 5 ledger works for everything it did before. `decision list` prints `-` as the
  ADR path of every decision, and `mission list --json` prints `adr` as null. The first
  write of any of the four kinds adds `decisions.adr_path` with
  `alter table decisions add column` and the later steps, and sets the `version` row to `8`
  in the same local Dolt change as the record. Reads, and a write that is refused (a path that is not a
  file included), upgrade nothing. `tsk ledger status` labels the upgrade; see
  [Status and discard](#status-and-discard).
- A version 6 ledger works for everything it did before. `mission constraint list` and
  `mission out-of-scope list`, and the `constraints` and `out_of_scope` arrays of
  `mission list --json`, read the two old text columns of `missions` and split them by the
  line rule below. Reads, and a write that is refused, upgrade nothing. The first write of
  any kind that reaches a mission or a thread (`mission`, `task`, `decision`, `question`,
  and `thread start` or `thread set mission`) does these in one local Dolt change with the
  record:
  1. adds `next_constraint` and `next_out_of_scope` to `missions`, and creates
     `mission_constraints` and `mission_out_of_scope`;
  2. turns each non-empty line of `constraints_text` into a `mission_constraints` row, and
     of `out_of_scope` into a `mission_out_of_scope` row. The line rule: split at line
     breaks, trim each line, skip a line that is empty, number the rest from 1 in order;
  3. sets each counter to the number after the last item, or leaves it at 1 for a mission
     with no item;
  4. drops `constraints_text` and `out_of_scope`;
  5. sets the `version` row to `8`, after the bindings step below.
  The same step runs for a ledger at version 3, 4 or 5, in the same chain. `tsk ledger
  status` labels the upgrade, and shows the new list rows as added rows of the two tables.
- A version 7 ledger works for everything it did before. The bindings are read from
  `cloud_session_bindings` as cloud rows: `thread binding`, `thread guard`,
  `thread session-start`, `thread resume` and `thread list` see them, and a read upgrades
  nothing. A worktree has no row there, so the older `tsk-thread-id` marker file still
  binds it until a write. The first write that reaches a binding (`thread start`,
  `thread resume`, `thread detach`, `thread stop`, or the marker migration in
  [`tsk-thread-id`](#tsk-thread-id)) does these in one local
  Dolt change with the write:
  1. creates `bindings`;
  2. copies every `cloud_session_bindings` row into it as kind `cloud`;
  3. drops `cloud_session_bindings`;
  4. sets the `version` row to `8`.
  The same step ends the chain for a ledger at version 2 to 6, and for the first write of
  any other kind that upgrades it. A write that is refused upgrades nothing. `tsk ledger
  status` labels the upgrade and does not show the copied rows as changes.
- A `tsk` binary from before version 8 does not know the version. It stops at its manifest
  check with `unsupported ledger version 8`, before it writes. The binary needs updating.
  The file ledger's `.tsk-ledger.toml` stays at version 1.

### Migration

`tsk ledger migrate-to-dolt` moves a file ledger to the Dolt backend. It runs with
either backend selected. The new ledger is version 8: it has no `documents` table, it has the mission, task, decision and question tables, and
its documents are written as files on `tsk/docs`. It:

1. Fetches the file ledger ref and checks its manifest. It stops when the ref is absent
   or already holds Dolt data.
2. Reads every record from the fetched commit's tree, not from the ledger worktree.
3. Builds a new Dolt database beside `ledger-db`, writes the schema and every record in
   one Dolt commit after `dolt init`'s own initial commit, and reads every record back
   through the `LedgerStore` trait to compare it with the tree. It also writes every
   document as a file in a temporary worktree of the documents branch, commits it, and
   reads each file back from the commit. A difference stops the command before anything
   is pushed.
4. Pushes the file ledger's commit to `refs/heads/archive/tsk-ledger-files`, or
   `refs/heads/archive/ledgers/<repo-id>-files` for a nexus ledger. The push only
   creates the branch or moves it forward. A branch at an unrelated commit stops the
   command.
5. Checks that the ledger ref still holds the migrated commit, then pushes the documents
   branch, then pushes the database to the ledger ref with `dolt push`. A moved ref stops
   the command with `another writer pushed`. Run it again.
6. Moves the new database to `ledger-db` and the temporary worktree to `docs`. An existing
   `ledger-db` is renamed to `ledger-db.pre-migrate-<timestamp>` first. An existing `docs`
   stops the command in step 1.
7. Prints `migrated:<file-commit>:<dolt-commit>`, with the record counts on stderr.

`--dry-run` runs steps 1 to 3 in temporary directories, deletes them, prints the counts and
`dry-run:ok`, and pushes nothing.

Dolt's first push to a git branch replaces the branch with an orphan history. The archive
branch keeps the file ledger's history.

Mission records of a file ledger (`records/missions/`) are not converted. They stay on the
archive branch, and the command lists them as files that are not records.

An older `tsk` binary that fetches the Dolt branch finds no `.tsk-ledger.toml` and stops
before it writes. The binary needs updating.

### Moving the documents out of Dolt

`tsk ledger migrate-docs` moves a version 1 Dolt ledger's documents from the `documents`
table to files on `tsk/docs`. It steps through:

1. Refreshes the database without the version 1 refusal. A version 2 ledger stops here
   with `the documents are already on the tsk/docs branch`.
2. Reads every row of `documents` with `select path, body`. A path that cannot be a file
   (absolute, `..`, `.git`), two rows that name one file, a file that is also a directory
   and a path or body that is not valid UTF-8 stop the command, naming the path. Nothing
   is changed.
3. Creates or refreshes the documents worktree and writes each row as a file at its path.
   A file that already holds the body is left alone. One that holds a seed file of a new
   branch is replaced. One that holds anything else stops the command before any file is
   written, so a rerun after a partial failure compares and then goes on.
4. Commits on the documents branch with the message `Move the documents out of Dolt`, and
   reads every file back from that commit to compare it with the rows. A difference stops
   the command before any push.
5. Checks the ledger ref has not moved. In the database, drops the `documents` table,
   creates the mission, task, decision and question tables and sets the `version` row to `4` in one Dolt commit, `Move the documents to the tsk/docs
   branch`.
6. Pushes the documents branch, checks the ledger ref again, then pushes the Dolt data.
7. Prints `migrated-docs:<count>`, with counts on stderr.

`--dry-run` does steps 1 to 4, removes the documents worktree it created, prints
`dry-run:ok`, and drops and pushes nothing. When a push fails after step 5, the drop stays
committed locally, and `tsk ledger push` publishes both branches in the right order.

## Mission commands

A mission is data: typed rows written only through `tsk mission` commands. An agent never
writes a mission row any other way. Free text for a reader stays a document on `tsk/docs`,
linked from the mission by path and role. Every command changes the local store only, and
`tsk ledger push` publishes. A command that fails exits 1, prints `error: ...` on stderr and
writes nothing. After a write, `tsk ledger fetch` refuses until the change is pushed, as it
does for a document written with `doc put`.

```
tsk mission create <id> <name> [--category operational|administrative] [--parent <id>]
tsk mission set <id> <field> <value>
tsk mission block <id> <by-id>
tsk mission unblock <id> <by-id>
tsk mission objective add <id> <outcome>
tsk mission objective done <id> <n>
tsk mission objective undone <id> <n>
tsk mission objective remove <id> <n>
tsk mission objective list <id>
tsk mission constraint add <id> <text>
tsk mission constraint remove <id> <n>
tsk mission constraint list <id>
tsk mission out-of-scope add <id> <text>
tsk mission out-of-scope remove <id> <n>
tsk mission out-of-scope list <id>
tsk mission link <id> <path> --role <intelligence|report|design>
tsk mission unlink <id> <path>
tsk mission list [<id>] [--json]
```

Rules:

- A mission ID matches `^M-[A-Z0-9]+(-[A-Z0-9]+)*$` and is at most 64 characters. A name is
  one line of at most 255 characters.
- `set` takes the fields `name`, `status`, `purpose` and `decision-authority`. The value
  comes from the argument, or from stdin when the argument is `-`. Stdin text is stored
  exactly, except that `name` and `status` drop the final line break. `set constraints` and
  `set out-of-scope` are errors: `error: constraints are a list now; use tsk mission
  constraint add|remove|list`, and `error: out-of-scope is a list now; use tsk mission
  out-of-scope add|remove|list`.
- `constraint` and `out-of-scope` are lists of one-line items, numbered per mission like
  objective outcomes. `add` assigns the next number and prints it. `remove` of a number the
  mission does not have is an error (`M-X has no constraint 3`, `M-X has no out-of-scope
  item 3`). Numbers are never reused: a counter on the mission (`next_constraint`,
  `next_out_of_scope`) only grows. An item is not empty and has no line break. `list` prints
  one item per line, tab-separated: the number, then the text, ordered by number. The two
  lists count apart.
- Status is `todo`, `in-progress` or `done`. Any other value is an error that names the
  three. An administrative mission is created `in-progress`, and `set status` to `todo` or
  `done` on it is refused, because it has no end.
- `block` needs both missions to exist. It refuses a mission that blocks itself, a pair
  that is already blocked, and any block that makes a cycle. `unblock` of a pair that is
  not blocked is an error.
- Blocked is never stored. A mission is blocked when any mission it names in `block` is not
  `done`. `list` derives it, so setting a blocker `done` frees the missions it blocked.
- `objective add` assigns the next number and prints it. Numbers are never reused after a
  `remove`.
- `link` needs the path to exist as a document on `tsk/docs`, else
  `error: no such document: <path>`. Linking a path again changes its role. At
  `tsk ledger push`, after the documents are pushed and before the Dolt data, tsk checks that
  every linked path still exists. A missing one stops the push with exit 1 and names the
  mission and path. Both local commits stay and the next push resumes.

`list` with no ID prints one line per mission, ordered by ID, tab-separated: `id`, `status`,
`blocked-by` (the IDs of the blockers that are not done, comma-separated, or `-`), `parent`
(or `-`) and `name`. With an ID it prints that mission's line. `list <id> --json` prints the
full record as one JSON object, and `list --json` prints an array of them:

```
{"id":"M-J","name":"Json","category":"operational","status":"todo","parent":"M-P","blocked":true,"purpose":"Why","decision_authority":"","constraints":[{"n":1,"text":"No new crates"}],"out_of_scope":[{"n":1,"text":"Projections"}],"objective":[{"n":1,"outcome":"One","met":true},{"n":2,"outcome":"Two","met":false}],"blocked_by":[{"id":"M-Q","blocking":true}],"documents":[{"path":"m/design.md","role":"design"}],"tasks":[],"decisions":[],"questions":[]}
```

`blocked_by` lists every blocker. `blocking` is true for each one that is not done, and
`blocked` is true when any is. `objective list <id>` prints `n`, `met` or `open`, and the
outcome, tab-separated. There is no `show` command and no Markdown output.

The `git` backend stores each mission as one pretty-printed JSON file with the keys in a
fixed order, `records/missions/<ID>.json` in the ledger tree. Separate files let git merge
two missions without a conflict, and a line per field lets it merge two fields of one
mission. The file ledger's `records/` directory is managed by tsk, so it is not a document
path. The validation, the derived blocked state and the output are the same code for both
backends.

In that file `constraints` and `out_of_scope` are arrays of `{"seq": 1, "text": "..."}`, with
the counters `next_constraint` and `next_out_of_scope` after `next_question`. A file written
before the lists existed holds a string for each and has no counters. The reader accepts
both forms: a string becomes items by the line rule (split at line breaks, trim, skip empty
lines, number from 1), and a counter that is missing or behind the rows follows the last
item. A read writes nothing, and the next write of the mission stores the list form.

## Task, decision and question commands

Tasks, decisions and questions belong to one mission. They are data like the mission: typed
rows written only through these commands, never by editing Markdown. Every command changes
the local store only, and `tsk ledger push` publishes. A command that fails exits 1, prints
`error: ...` on stderr and writes nothing.

```
tsk task add <mission> <description>
tsk task set <mission> <task> <field> <value>
tsk task block <mission> <task> <by>
tsk task unblock <mission> <task> <by>
tsk task list <mission>
tsk decision add <mission> <text> --by <who> [--task <T-nn>] [--on <YYYY-MM-DD>] [--adr <path>]
tsk decision list <mission> [--adr <path>]
tsk decision list --adr <path>
tsk question add <mission> <text>
tsk question edit <mission> <question> <text>
tsk question answer <mission> <question> <text> --by <who>
tsk question list [<mission>]
```

Rules:

- `task add` and `question add` mint the next ID of the mission and print it. IDs are
  `T-nn` and `Q-nn`: the number zero padded to two digits, and wider past 99. A counter on
  the mission (`next_task`, `next_question`) only grows, so a number is never reused. Task
  and question IDs are unique within a mission, not across missions.
- A description, a decision, a question, an answer and the name after `--by` are one line.
  A line break or a tab, or an empty value, is an error.
- `task set` takes the fields `status` and `description`. Status is `todo`, `in-progress`
  or `done`. Any other value is an error that names the three.
- `task block` needs both tasks to exist in the mission. It refuses a task that blocks
  itself, a pair that is already blocked, and any block that makes a cycle. `unblock` of a
  pair that is not blocked is an error.
- Blocked is never stored. A task is blocked when any task it names in `block` is not
  `done`.
- `decision add` records the UTC date of the command, or the date given to `--on`. `--on`
  takes `YYYY-MM-DD` and must be a real calendar date: `2026-02-30` is an error, and so is a
  date written any other way. `--task` must name a task of the mission. A decision has no
  label.
- `--adr` names the ADR the decision follows, by its path from the root of the code
  repository, for example `docs/adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md`.
  The root is the top level of the working tree of the repository the command runs in, found
  with `git rev-parse --show-toplevel`, from any directory inside it. That holds for a
  ledger kept in a nexus too: the path is checked against the repository you are in, not
  the one that holds the ledger. A path that is absolute, holds a `..` component, holds a
  tab or a line break, is longer than 1024 characters, or is not a file in the working
  tree is an error, and nothing is written. A leading `./` is dropped, so one ADR has one
  spelling. A decision names at most one ADR. A later change to the ADR file does not
  touch the record.
- `decision list --adr <path>` takes the path as `--adr` does, without the check that the
  file exists, so a decision of an ADR that was since renamed is still found by its old
  path. With no mission it lists the decisions of every mission that name the ADR. A call
  with neither a mission nor `--adr` is an error.
- `question edit` rewrites the text of an open question and nothing else: the ID and the
  date asked stay. An answered question cannot be edited:
  `error: Q-nn of M-X is answered; it cannot be edited`.
- `question answer` records the answer, the name after `--by` and the UTC date, and the
  question is no longer open. A question is answered once: a second answer is an error.
- An unknown mission, task or question is an error that names the ID.

Each `list` prints one record per line, tab-separated, with no header:

| Command | Columns |
|---|---|
| `task list <mission>` | `T-nn`, `status`, `blocked-by` (the IDs of the blockers that are not done, comma-separated, or `-`), `description`. Ordered by number. |
| `decision list <mission>` | `decided_on` (`YYYY-MM-DD`), `decided_by`, `T-nn` or `-`, `text`, `adr` (the ADR path, or `-`). Oldest first. With `--adr`, only the decisions that name that ADR, in the same five columns. |
| `decision list --adr <path>` | `mission`, then the five columns above. Ordered by mission and then oldest first. |
| `question list [<mission>]` | `mission`, `Q-nn`, `asked_on` (`YYYY-MM-DD`), `text`. Open questions only, ordered by mission and number. All missions when none is given. |

`tsk mission list --json` carries the records. Each mission object has three more keys after
`documents`: `tasks` (`id`, `description`, `status`, `blocked`, and `blocked_by` as a list
of `id` and `blocking`), `decisions` (`text`, `decided_by`, `decided_on`, `task` or null, and `adr` or null)
and `questions` (`id`, `text`, `status`, `asked_on`, and `answer`, `answered_by` and
`answered_on`, null while open). There is no `show` command and no Markdown output.

The `git` backend keeps the records in the mission's file, `records/missions/<ID>.json`,
as the keys `next_task`, `next_question`, `tasks`, `decisions` and `questions`. A decision
holds `adr` only when it names an ADR; a file written before that field existed reads as
none. A file written before this release has none of them and reads as a mission with no records. The
validation, the derived blocked state and the output are the same code for both backends.
Two decisions added at the same time by two clones get the same `seq` and conflict at push,
as do two tasks added to one mission.

## File layout (`git` backend)

The rest of this document describes the file ledger, the `git` backend. Where it names the ledger worktree, the Dolt backend has the ledger store (`ledger-db`) instead.

## Location and ref

A ledger is a git branch. It is never a custom ref: a Claude Code cloud session can
write only `refs/heads/*` (ADR 0008). It is never checked out in the managed repo's main
working copy.

| Location | Remote | Ref |
|---|---|---|
| In-repo (default) | the managed repo's `origin` | `refs/heads/tsk/ledger` |
| Nexus (option) | the nexus repo | `refs/heads/ledgers/<repo-id>` |

The binary passes the full ref name to every git command. It never passes `tsk/ledger`
or any other unqualified name. On the tsk repo's `origin`, an orphaned custom ref
`refs/tsk/bootstrap` shadows the unqualified name `tsk/bootstrap`, and an unqualified
fetch returns that ref's content with exit status 0 (ADR 0008, amended 2026-09-16). The
full name makes the same trap impossible for `tsk/ledger`.

The nexus location is for a managed repo the operator cannot push a ledger branch to,
such as a work repo. The ledger objects live in the nexus repo. The managed repo's clone
fetches and pushes them against the nexus URL as the remote, so the ledger worktree stays
a linked worktree of the managed repo's clone (see
[Ledger worktree location](#ledger-worktree-location)).

### User config

`tsk nexus add <url>`, or `tsk config attach-nexus <url>`, records the nexus repo URL in
the user config:

```
${XDG_CONFIG_HOME:-$HOME/.config}/tsk/config.toml
```

```toml
[nexus]
url = "https://github.com/jimbarritt/tsk-nexus"
```

- `XDG_CONFIG_HOME` set to an empty string counts as unset.
- The command creates the file and its directory when they are absent and keeps any
  other key in the file.
- Attaching the URL already recorded changes nothing and prints
  `nexus already attached: <url>`. Attaching a different URL replaces it and prints
  `replaced nexus <old> with <new>`. A first attach prints `attached nexus <url>`.
- `tsk config show` prints the config path and the attached URL. `tsk nexus list` prints
  the same, then the territories and repos in the nexus's `nexus.json`.

### Choosing the location

The choice is per managed repo:

1. With no nexus attached, the ledger is in-repo.
2. With a nexus attached, tsk reads the nexus's `nexus.json` (see
   [Reading the nexus](#reading-the-nexus)) and finds the managed repo's entry:
   1. If `$(git-common-dir)/tsk-repo-id` holds an ID (see
      [Local-only files](#local-only-files)) and a visible entry has that `id`, that is
      the entry.
   2. Otherwise the entry is the first visible entry whose `url`, normalised, equals the
      normalised value of the managed repo's raw `remote.origin.url`. The raw value is
      read with `git config --get remote.origin.url`, not `git remote get-url`, because
      the latter applies `url.<base>.insteadOf` rewrites.
   3. A visible entry is one with no `local` field, or with `local` equal to this
      machine's name. The machine name is the value of `TSK_MACHINE_NAME` when set,
      otherwise the output of `hostname`. An entry whose `local` names another machine
      is skipped in both steps.
3. No entry means in-repo. An entry with no `ledger` field, or `"ledger": "repo"`, means
   in-repo. `"ledger": "nexus"` means the ref `refs/heads/ledgers/<repo-id>` in the nexus,
   where `<repo-id>` is the entry's `id`. Any other `ledger` value stops with an error.
4. When an entry is found and `tsk-repo-id` does not already hold its `id`, tsk writes the
   `id` to `tsk-repo-id`.

A managed repo whose `origin` is a local path or a `file://` URL has no normalised form.
It keeps an in-repo ledger unless `tsk-repo-id` names an entry.

Every command that uses the ledger (`tsk ledger fetch` and `push`, `tsk thread`,
`tsk events`, the session start) resolves the location this way. `tsk ledger path` does
not: it needs only the clone ID.

### The nexus entry

`nexus.json` lists repos under territories:

```json
{
  "version": 1,
  "territories": [
    {
      "id": "agentic-engineering",
      "name": "Agentic Engineering",
      "repos": [
        { "id": "tsk", "url": "https://github.com/jimbarritt/tsk" },
        {
          "id": "work-api",
          "url": "https://github.com/acme/work-api",
          "ledger": "nexus"
        },
        { "id": "scratch", "local": "laptop", "ledger": "nexus" }
      ]
    }
  ]
}
```

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | The repo ID. It names the ledger branch, `ledgers/<id>`, and is stored in the ledger's manifest. `[a-z0-9][a-z0-9-]*`. |
| `url` | no | The repo's clone URL, in any form that normalises to the same name (see below). Absent for a repo with no remote yet. |
| `local` | no | A machine name. The entry is visible only on that machine, and is found only through the cached repo ID. Used with no `url`. |
| `ledger` | no | `"repo"` or `"nexus"`. Absent means `"repo"`. |

`tsk nexus register-repo`, run in the managed repo, adds its entry. It fetches the
nexus's default branch, adds the entry to `nexus.json`, and commits and pushes it with the
machine's own git credentials. On a rejected push it fetches again and retries once.

- Without `--local`, the entry's `url` is the repo's raw `remote.origin.url`. An origin
  that does not normalise (a local path, for example) stops with an error.
- With `--local`, the entry carries `local` with this machine's name in place of `url`,
  and the command writes the ID to `tsk-repo-id`.
- The ID is `--id`, or the repo name (the last segment of the normalised origin URL, or
  the working tree's directory name with `--local`) in lower case, with each run of other
  characters as one `-`.
- The territory is `--territory`, required when `nexus.json` has more than one. With one
  territory, that one is used. With none, `--territory` creates it, named
  `--territory-name` or, without that option, its ID.
- `ledger` is `--ledger`, `nexus` by default, and is always written.
- An entry with the same normalised `url` and the same ID, or the same ID and the same
  `local`, means the repo is registered: the command prints `already registered` and
  commits nothing. The same `url` under another ID, or the same ID for another repo, stops
  with an error.
- The command rewrites `nexus.json` from its parsed form, pretty printed with a trailing
  newline. Every field is kept. Object keys come out in alphabetical order.

Entries can still be written by hand. Automatic transition of an entry from `local` to
`url` on the first push is not implemented.

### Origin URL normalisation

Normalisation maps the HTTPS and SSH forms of one repo to the same
`<host>/<owner>/<repo>`. tsk uses it to match the managed repo's `origin` against the
`url` of nexus entries. It does not name a ref.

| Form | Example |
|---|---|
| HTTPS | `https://github.com/jimbarritt/tsk.git` |
| HTTPS with user | `https://jim@github.com/jimbarritt/tsk` |
| SSH URL | `ssh://git@github.com/jimbarritt/tsk.git` |
| SSH URL with port | `ssh://git@github.com:22/jimbarritt/tsk.git` |
| scp-like SSH | `git@github.com:jimbarritt/tsk.git` |

Each maps to `github.com/jimbarritt/tsk`.

Rules, applied in order:

1. Remove the scheme (`https://`, `http://`, `ssh://`, `git://`). For the scp-like
   form, the first `:` separates host from path.
2. Remove a `user@` prefix from the host, and a `:<port>` suffix in the URL forms.
3. Remove a trailing `/`, then a trailing `.git`.
4. Lowercase the host, owner and repo.
5. Join host and path segments with `/`. A path with more than two segments (a GitLab
   subgroup, for example) keeps every segment in order.

A `file://` URL, a local path, or a URL with fewer than two path segments has no
normalised form and matches no entry.

### Reading the nexus

tsk keeps a bare clone of the nexus under the state root, at
`${XDG_STATE_HOME:-$HOME/.local/state}/tsk/nexus/`. Each resolution runs
`git fetch <url> +HEAD:refs/heads/nexus` there, then reads the file with
`git show refs/heads/nexus:nexus.json`. No working copy of the nexus exists. When the
fetch fails and an earlier fetch left `refs/heads/nexus`, tsk prints a note to stderr
and reads that copy. When the fetch fails and no copy exists, the command stops with an
error.

The clone is replaced by fetching from the attached URL each time, so attaching a
different URL needs no clean-up.

### Fetching and pushing a nexus ledger

The ledger worktree is a linked worktree of the managed repo's clone in both locations.
For a nexus ledger the managed repo's clone runs
`git fetch <nexus-url> refs/heads/ledgers/<repo-id>` and
`git push <nexus-url> HEAD:refs/heads/ledgers/<repo-id>`, with the nexus URL as the
remote argument. Fetching into the managed clone puts the ledger objects in its object
database, which `git worktree add` needs. The nexus repo is not an `origin` or any
named remote of the managed clone, and no `.git/config` entry is written.

### A ledger that does not exist yet

When the ledger ref is absent on its remote, `tsk ledger fetch` creates a new ledger. It
detects absence with `git ls-remote --exit-code`, which exits 2 for a missing ref,
after a failed `git fetch`. Any other failure is an error.

1. tsk writes an orphan commit with git plumbing (`hash-object`, `mktree`,
   `commit-tree`). Its tree holds `.tsk-ledger.toml` (`version = 1`, plus
   `repo_id = "<id>"` for a nexus ledger) and an `index.md` holding `# Ledger index`.
   `missions/`, `threads/` and `external-events/` are not in the tree: git does not hold
   an empty directory, and the commands create them on first write.
2. tsk creates the ledger worktree on that commit, detached.
3. tsk prints a note to stderr that the branch does not exist yet, and the path to
   stdout.

The branch is created by the first `tsk ledger push`. Its push lease is create-only
(`--force-with-lease=<ref>:` with an empty expected value), so the push is rejected if
another writer created the branch first. A later `tsk ledger fetch` while the branch is
still absent leaves the ledger worktree as it is and prints the note again.

This applies to both locations.

## Ledger worktree location

The ledger is materialised in the ledger worktree, a detached linked worktree of the
managed repo's clone, at a fixed path outside the repository:

```
${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger
```

- `XDG_STATE_HOME` set to an empty string counts as unset, the same as the shell's
  `${VAR:-default}`.
- `<clone-id>` is the content of `.git/tsk-clone-id` (see
  [Local-only files](#local-only-files)).
- The directory name is `ledger`. The `tsk/bootstrap` ledger worktree for the same clone
  is `.../repos/<clone-id>/bootstrap`, so both exist side by side until T-09 retires
  `tsk/bootstrap`.
- The ledger worktree is created with `git worktree add --detach`, so it holds no branch
  checkout and does not lock `tsk/ledger` against a checkout elsewhere. Its metadata is
  in the managed repo's `.git/worktrees/`, which only git edits.

The path is the same for both ledger locations. For a nexus ledger, the objects reach the
managed repo's clone by a fetch against the nexus URL (see
[Fetching and pushing a nexus ledger](#fetching-and-pushing-a-nexus-ledger)).

## Tree

```
.tsk-ledger.toml
index.md
future-missions-tbd.md
missions/
  administrative/
    M-<ID>-<slug>.md
    M-<ID>/
      ...
  operational/
    M-<ID>-<slug>.md
    M-<ID>/
      ...
threads/
  lookup-by-cloud-session.json
  <thread-id>/
    index.md
    continuation-state.jsonl
external-events/
  queue.ndjson
  watermark.json
```

The tree is the `tsk/bootstrap` tree unchanged, plus `.tsk-ledger.toml`. It is the same
in both ledger locations. The ADR 0007 event log is not part of it.

Every file is optional except `.tsk-ledger.toml`. A command that reads an absent file
treats it as empty, as described per file below.

## `.tsk-ledger.toml`

The ledger manifest, at the root of the tree. TOML:

```toml
version = 1
```

| Key | Type | Required | Meaning |
|---|---|---|---|
| `version` | integer | yes | Layout version. This document describes version `1`. |
| `repo_id` | string | no | The repo ID of a nexus ledger, the `id` of its nexus entry. In-repo ledgers may omit it. |

- A ledger with no `.tsk-ledger.toml` is not a tsk ledger. The binary stops and reports
  it.
- A `version` the binary does not support stops the binary with an error naming the
  version found and the versions it supports. The binary writes nothing to the ledger or
  the ledger worktree before this check.
- When the ledger is a nexus ledger and the manifest holds `repo_id`, the value must equal
  the entry's `id`. A different value stops the binary with an error before anything is
  written.
- Unknown keys are ignored.
- T-07 adds this file in the first commit on `tsk/ledger`, made on top of the
  `tsk/bootstrap` tip so history carries over.

## `index.md` and `future-missions-tbd.md`

Free-form Markdown, written by humans and agents. No command parses either file. Both are
file-era documents. Since mission records exist, `tsk mission list` is the index, and
no agent reads or writes `index.md`. A ledger keeps the two files until its missions are
migrated to records, and the migration retires them.

- `index.md` was the entry point: current state and next steps.
- `future-missions-tbd.md` held mission ideas not yet shaped into missions. Ideas now go
  in a document linked to an administrative mission, or in a mission of their own.

## `missions/`

The files missions leave behind, as Markdown: intelligence, research, reports and designs,
linked from the mission record by path and role. Briefings from the file era are also
here. A mission's purpose, tasks, decisions and questions are records and not documents,
so a new mission has no briefing. The binary moves these files between the ledger and the
ledger worktree without parsing them.

Layout in use:

| Path | Content |
|---|---|
| `missions/administrative/`, `missions/operational/` | One directory per mission category. |
| `missions/<category>/M-<ID>-<slug>.md` | A mission briefing from the file era. `<ID>` is the mission ID, for example `BOOT-04`. A done mission keeps its briefing here, linked from its record. |
| `missions/<category>/M-<ID>/` | A mission's supporting files: sub-briefings, intelligence indexes, reports. |
| `missions/<category>/M-<ID>-<slug>-report-<YYYY-MM-DD>.md` | A dated report beside its briefing. |

The one structural rule a command enforces: `thread start` and `thread set mission` take
a mission ID (`thread start` also runs with none, and makes a thread with no mission). They
accept the ID when the mission has a record, and, for a ledger not yet migrated, when a
briefing document at
`missions/<category>/<ID>-<slug>.md` or `missions/<category>/<ID>.md` exists (for
example `missions/operational/M-BOOT-04-official-ledger.md` for `M-BOOT-04`). The ID in
a file name is `M-` and the groups of capitals and digits that follow, up to the slug.
It refuses any other ID with `error: no such mission: <ID>`. Older skills pass a briefing
path as a second argument: it is accepted and ignored.

## `threads/`

The thread store from `docs/domain/session-continuation-design.md`.

### `threads/<thread-id>/`

One directory per thread. The directory name is the thread ID (see
[Common value formats](#common-value-formats)). Created by `thread start`, deleted with
its contents by `thread stop`. Its presence is the collision check when a new thread ID
is minted.

### `threads/<thread-id>/index.md`

Written at thread start, and rewritten by `thread set mission`. Exact content, with a
trailing newline. The last line is there only when the mission has no record:

```
# Thread <thread-id>

Mission: <mission-id>
Mission briefing: [<briefing-path>](<briefing-path>)
```

`<briefing-path>` is the path of the briefing document that `thread start` found, relative
to the ledger root. An older thread has no `Mission:` line. A thread with no mission also
has no `Mission:` line, and the file is only `# Thread <thread-id>`. A reader treats both
as valid. `thread set mission` rewrites the file for the new mission, and
`thread set mission --none` rewrites it without the `Mission:` and briefing lines.

`thread list` reads the mission ID from the `Mission:` line, and derives it from the
briefing path when the line is absent. It reads the mission link from this file: the text
between `Mission briefing: [` and the next `]`, first match. A missing file or no match
gives an empty string.

### `threads/<thread-id>/continuation-state.jsonl`

Append-only. Created empty at thread start. One JSON object per line, compact (no
whitespace between tokens), terminated by `\n`. Each line is one continuation state
entry, written by `thread pause` (today `thread-append-handover.sh`). Lines are never
rewritten or removed, except that `thread stop` deletes the whole file with the thread.

Fields, in the order written. `git` holds `ledger` then `code`; `ledger` holds `commit`;
`code` holds `ref` then `commit`:

| Field | Type | Source | Meaning |
|---|---|---|---|
| `mission_id` | string or null | command | The mission the thread works, from the thread. `null` when the thread has no mission. Entries written before version 5 do not have it. An entry keeps the mission it was written with: `thread set mission` does not rewrite earlier entries. |
| `mission_link` | string | command | The mission briefing the thread works on, as a path relative to the ledger root. Written only for a thread whose mission has no record. Earlier entries always have it. |
| `task_id` | string | caller | The task in progress at pause time, for example `T-03`. Can be empty. When the mission has a record, the command checks it against the mission's tasks and writes `T-5` as `T-05`. A mission with no record has no tasks to check against. A thread with no mission has no tasks, so `task_id` is empty: the command refuses another value with `error: thread <thread-id> has no mission, so it has no tasks. Attach a mission with tsk thread set mission <mission-id>, or pass an empty task ID`. |
| `whats_next` | string | caller | A short account of where things stand. |
| `git` | object | command | The commits the entry records, in two nested objects: `ledger` and `code`. |
| `git.ledger.commit` | string | command | Full 40-character SHA of the ledger worktree's `HEAD` after it is refreshed from the remote and before this entry is appended. |
| `git.code.ref` | string | command | Full ref of the branch the code worktree's `HEAD` is on at pause time, for example `refs/heads/feature/x`. |
| `git.code.commit` | string | command | Full SHA of the code worktree's `HEAD` at pause time. |
| `timestamp` | string | command | UTC time the entry is appended. |
| `written_by` | string | command | URN of the binding that wrote the entry. |

`git.ledger.commit` cannot be the commit the push creates: that commit holds this entry,
so its hash is unknown when the entry is written.

The flat fields `commit_on_bootstrap`, `commit_on_ledger` and `commit_on_main` of earlier
entries are not read or written by the binary, and there is no fallback for them. A
reader that passes an entry through unchanged (for example `thread resume`, which prints
the latest entry) prints it as stored. Entries written before the nested `git` object
exist on `tsk/ledger` only after a one-off migration commit, which sets
`git.code.ref` to `refs/heads/main` because the earlier rule required the commit to be on
origin's default branch.

#### Pause rule for the code worktree

`thread pause` runs in the directory it is given: the code worktree that contains the
current directory supplies `git.code.ref` and `git.code.commit`. The command checks, in
this order:

1. `git symbolic-ref --quiet HEAD` names a branch. A detached `HEAD` stops the pause with
   `error: HEAD is detached, so the branch the work is on cannot be recorded.` and exit
   status 1. A `HEAD` that points outside `refs/heads/` stops it with
   `error: HEAD points at '<ref>', which is not a branch.` and exit status 1.
2. `HEAD` is reachable from some branch on origin. The binary runs
   `git fetch --quiet --prune origin '+refs/heads/*:refs/remotes/origin/*'`, then
   `git for-each-ref --contains <commit> --format=%(refname) refs/remotes/origin/`. Any
   listed ref other than `refs/remotes/origin/HEAD` satisfies the rule. The explicit
   refspec makes the check independent of the clone's configured fetch refspec, and
   `--prune` drops remote-tracking refs of branches deleted on origin. If no ref is
   listed, the pause stops with
   `error: HEAD (<sha>) is not reachable from any branch on origin.` and exit status 1,
   and writes nothing: an actor resuming the thread from another clone cannot see an
   unpushed commit.

`git.code.ref` is the local branch name, not the origin branch that holds the commit. The
default branch of origin plays no part in the rule. Each entry is a snapshot at pause, so
a branch switch between pauses needs no tracking.

#### Reading the latest entry

The latest entry is the last non-empty line of the file. An empty file means the thread
has no entry yet; the scripts print `{}` for its latest entry. `thread list` sorts threads
by the latest entry's `timestamp`, newest first, with threads that have no entry last.
The `written_by` values across all entries are the set of actors that have touched the
thread.

### `threads/lookup-by-cloud-session.json`

The cloud session binding map. A single JSON object keyed by the
`CLAUDE_CODE_REMOTE_SESSION_ID` value (`cse_...`). Each value:

| Field | Type | Meaning |
|---|---|---|
| `thread_id` | string | The thread the session is bound to. |
| `registered_at` | string | UTC time the binding was written. |

```json
{
  "cse_015h8qmbPEuyKyaoY7xuur11": {
    "thread_id": "9qo5iin0",
    "registered_at": "2026-09-17T18:26:21Z"
  }
}
```

- Absent file: no cloud bindings. The first bind creates it as `{}` before adding the
  entry.
- Written whole, pretty-printed with two-space indentation (the default `jq` output),
  through a temporary file renamed over the original, so a reader never sees a partial
  file. The binary creates the temporary file in the same directory, so the rename is
  atomic.
- Bind sets or replaces the entry for the current session ID. Detach deletes that one
  key. Stop deletes every entry whose `thread_id` is the stopped thread, for any
  session.

### `threads/.worktree-bindings/<actor>.json`

The bindings of local code worktrees on the file ledger, one file for each worktree. The
directory starts with a dot, so it is not a thread. The file name is the worktree actor key,
`<clone-id>/<worktree-name>`, with every byte outside `A-Za-z0-9._-` written as `%XX` (so
`/` is `%2F`), and `.json`. The content is a JSON object:

```json
{
  "actor": "tsk-1a2b3c4d/main",
  "thread_id": "9qo5iin0",
  "registered_at": "2026-10-17T10:00:00Z"
}
```

One file for each actor keeps two clones that bind at the same time from changing the same
file, so their pushes do not conflict in a rebase. A bind writes the file, replacing the
actor's earlier one. Detach deletes it. Stop deletes every file whose `thread_id` is the
stopped thread. `thread list` shows these files and the entries of
[`lookup-by-cloud-session.json`](#threadslookup-by-cloud-sessionjson) as actors.
`tsk ledger migrate-to-dolt` carries both into `bindings`, with kind `cloud` or `worktree`.
The manifest stays at version 1.
- Only used when `CLAUDE_CODE_REMOTE_SESSION_ID` is set. A code worktree binds through
  `tsk-thread-id` (see [Local-only files](#local-only-files)) and never writes this file.

## `external-events/`

The external event queue (M-BOOT-02, T-19).

### `external-events/queue.ndjson`

Append-only. One JSON object per line, compact, terminated by `\n`. Each line is one
event envelope:

| Field | Type | Meaning |
|---|---|---|
| `source` | string | The event source, for example `github`. |
| `event_type` | string | The kind of event, for example `dependabot_alert`, `code_scanning_alert`, `secret_scanning_alert`. |
| `action` | string | What happened: the webhook action (`created`, `fixed`, ...), or `polled` for an event found by polling. |
| `repo` | string | The repository the event concerns, `owner/repo`. |
| `received_at` | string | UTC time the envelope was appended. |
| `payload` | any JSON value | The source's own payload, unchanged. |

- Absent file: an empty queue.
- An event's position is its 1-based line number. The queue length is the number of
  `\n`-terminated lines (`wc -l`).
- Producers append and push in one fetch, append, push cycle. A poll that finds several
  alerts appends all of them, with one shared `received_at`, and pushes once. A poll
  that finds none pushes nothing.

### `external-events/watermark.json`

How far a consumer has processed the queue. Pretty-printed JSON object:

| Field | Type | Meaning |
|---|---|---|
| `processed_through` | non-negative integer | The number of queue lines processed. Lines `1` to `processed_through` are done. |
| `updated_at` | string | UTC time the watermark was last advanced. |

```json
{
  "processed_through": 17,
  "updated_at": "2026-09-20T09:24:54Z"
}
```

- Absent file: `processed_through` is `0`.
- A missing `processed_through` field reads as `0`.
- Reading new events returns lines `processed_through + 1` to the end, oldest first, and
  does not write the watermark.
- Advancing takes a count. A count below the current value is refused. A count equal to
  it is a no-op and pushes nothing. A higher count rewrites the file and pushes. A
  consumer advances only after it has processed every event up to that count, so an
  interrupted run leaves the watermark behind rather than skipping events.

## Local-only files

Three files belong to one clone and are never committed to the ledger, in either ledger
location.

### `.git/tsk-clone-id`

Path: `$(git rev-parse --path-format=absolute --git-common-dir)/tsk-clone-id`. One
per clone, shared by all its code worktrees.

Content: `<name>-<suffix>` and a trailing newline. Readers strip all whitespace.

- `<name>` is the basename of the directory that holds the common git directory (the
  clone directory for a normal clone), with every character outside `A-Za-z0-9._-`
  removed. An empty result becomes `repo`.
- `<suffix>` is 8 lowercase hexadecimal characters, random per clone.

Example: `tsk-180ae86e`. Minted once, by the first command that needs it and finds the
file absent or empty, and read unchanged forever after. It survives a rename or move of
the clone directory, which a hash of the path would not. A failure to mint writes
nothing. The `tsk/bootstrap` scripts and the binary share this file, so the ledger
worktrees for `tsk/bootstrap` and `tsk/ledger` sit under the same `<clone-id>` directory.

### `tsk-repo-id`

Path: `$(git rev-parse --path-format=absolute --git-common-dir)/tsk-repo-id`. One per
clone.

Content: the repo ID of the clone's nexus entry and a trailing newline. Readers strip all
whitespace. An absent or empty file means no cached ID.

Written by the location resolution when it finds an entry, and by
`tsk nexus register-repo --local` (or by hand) for a repo with no `url` (an entry that
carries `local`), whose entry is found only through this file. It
is read first on every later run, so a change of the repo's remote URL does not lose the
entry. Never pushed.

### `tsk-thread-id`

No longer written. Before version 8 of the Dolt layout, and in the file ledger before this
change, this file was the binding of a code worktree:
`$(git rev-parse --path-format=absolute --git-dir)/tsk-thread-id`, that is `.git/tsk-thread-id` in
the main code worktree and `.git/worktrees/<name>/tsk-thread-id` in a linked one. Content: a
thread ID and a trailing newline.

A code worktree's binding is now a `bindings` row of kind `worktree`, keyed on
`<clone-id>/<worktree-name>`. The first command that resolves a worktree binding, finds
this file and finds no row for the worktree's actor key writes the row from the file and
removes the file. The row is a local ledger change, pushed with the next push. Rules:

- A command that reads the ledger store as it is and never fetches (`thread guard`,
  `thread binding --no-fetch`) reads a marker file but does not migrate it.
- A worktree that has a row and a marker keeps the row and loses the file.
- When no ledger store exists yet, the file is read as it is and nothing fails.
- When the ledger refuses the write, the file stays and still binds.
- After the migration nothing writes the file again.

### `tsk-thread-skip`

Path: `$(git rev-parse --path-format=absolute --git-dir)/tsk-thread-skip`, in
the git directory's other files. In a cloud session (`CLAUDE_CODE_REMOTE_SESSION_ID` set) the file is
`tsk-thread-skip-<session-id>`, with every character outside `A-Za-z0-9_-` replaced by `_`,
so each cloud session skips for itself.

Content: the word `skipped` and a newline. Only the file's presence counts.

Written by `thread skip`. Deleted by `thread start` and `thread resume`. While it exists,
`thread guard` passes in silence, and the session start context says the session works
without a thread. Never in the ledger and never pushed.

## Common value formats

| Value | Format |
|---|---|
| Timestamp | UTC, second precision, `YYYY-MM-DDTHH:MM:SSZ` (`date -u +%Y-%m-%dT%H:%M:%SZ`). |
| Commit | Full 40-character lowercase hexadecimal SHA-1. |
| Thread ID | 8 characters, `[0-9a-z]`. Minted as 8 lowercase hexadecimal characters; IDs minted before hex minting are base36 (for example `4onylfsg`) and stay valid. Unique within `threads/`: a new ID that matches an existing directory is discarded and minted again, up to 100 attempts. |
| Actor URN | `urn:tsk:cloudsession:<CLAUDE_CODE_REMOTE_SESSION_ID>` in a cloud session, otherwise `urn:tsk:worktree:<name>`, where `<name>` is the basename of `git rev-parse --git-dir` (`.git` in a main code worktree). |
| Text encoding | UTF-8. JSON strings use standard JSON escaping. |

## Fetching and printing the path

`tsk ledger fetch` replaces `fetch-bootstrap-ref.sh`. `tsk ledger path` replaces
`bootstrap-wt-path.sh`. Both run in the `tsk` client alone, with no `tskd`, from inside
any working tree of the managed repo.

### Outside a git repository

Every command that needs the clone's git directory (`ledger`, `mission`, `task`,
`decision`, `question`, `events` and the `thread` commands) stops outside a git repository
with exit 1 and one line on stderr:
`error: not inside a git repository. Run tsk from inside the repository whose ledger you want.`
The check is made once, where the repository is discovered. `tsk --version`,
`tsk self-update`, `tsk guide`, `tsk install-plugin`, `tsk config` and `tsk nexus list`
need no repository and run anywhere. The hooks `tsk thread session-start` and
`tsk thread guard` do not exit 1: a hook must produce its JSON, so `session-start` reports
the problem in its context message.

### `tsk ledger path`

Prints the ledger worktree path and a newline, and exits 0. It runs no fetch, writes no
file, and does not check whether the ledger worktree exists. If `.git/tsk-clone-id` is
absent or empty it does not mint one: it exits non-zero and names `tsk ledger fetch`,
which mints it.

### `tsk ledger fetch`

1. Resolve the ledger location (see [Choosing the location](#choosing-the-location)).
2. Read `.git/tsk-clone-id`, minting it if absent or empty, and compute the ledger
   worktree path.
3. `git fetch <remote> <ref>` with the full ref name, then read the fetched commit from
   `FETCH_HEAD`. If the ref is absent on the remote, create a new ledger instead (see
   [A ledger that does not exist yet](#a-ledger-that-does-not-exist-yet)).
4. Read `.tsk-ledger.toml` from the fetched commit (`git cat-file blob <sha>:.tsk-ledger.toml`)
   and check its version and, for a nexus ledger, its `repo_id`. A missing manifest, an
   unsupported version or a mismatched `repo_id` stops here, before the ledger worktree
   changes.
5. If the ledger worktree directory does not exist: remove a stale registration of that path
   (`git worktree prune`, only when `git worktree list --porcelain -z` lists the path),
   create the parent directories, and run `git worktree add --detach <path> <sha>`.
6. If the ledger worktree directory exists:
   - It must be a linked worktree of this clone (its `--git-common-dir` equals the clone's).
     Otherwise stop.
   - If `git status --porcelain=v2 -z` in the ledger worktree prints anything (staged,
     unstaged or untracked changes), stop with exit 1 and leave the ledger worktree as
     it is.
   - If the ledger worktree's `HEAD` holds commits the fetched commit does not
     (`git log -z --format='%h %s' <sha>..HEAD` is non-empty), print a note naming them to stderr,
     leave the ledger worktree as it is, and continue to step 7 with exit 0. A commit
     made without a push leaves a clean ledger worktree, and a reset would orphan it.
   - Otherwise `git reset --hard <sha>`.
7. Print the ledger worktree path and a newline to stdout.

Diagnostics go to stderr. Stdout carries the path only, so `WT="$(tsk ledger fetch)"`
works.

## Writing to the ledger

Every change to the ledger is an edit to files in the ledger worktree followed by a push.
No command writes a ledger ref any other way. `tsk ledger push "<message>"` replaces
`push-bootstrap-ref.sh`. It runs in the `tsk` client alone, with no `tskd`, from inside
any working tree of the managed repo.

### `tsk ledger push`

1. Resolve the ledger location and the ledger worktree path, as for `tsk ledger fetch`.
   If `.git/tsk-clone-id` is absent or empty, or the ledger worktree directory does not
   exist, stop with exit 1 and name `tsk ledger fetch`. The push does not mint a clone
   id. The ledger worktree must be a linked worktree of this clone.
2. Fetch the ledger ref by its full name in the ledger worktree and check the fetched
   commit's `.tsk-ledger.toml`, as `tsk ledger fetch` does. A missing manifest or an
   unsupported version stops here, before anything is staged or committed. If the ref is
   absent on the remote, the manifest of the ledger worktree's `HEAD` is checked instead.
3. `git add -A` in the ledger worktree.
4. If `git diff --cached --quiet` exits 1, commit the staged changes with the caller's
   message. If it exits 0, nothing is staged: print a note to stderr and continue, since
   an earlier run can have committed without pushing.
5. Up to 5 attempts. Attempt 1 uses the commit fetched in step 2. Each later attempt
   fetches the ledger ref again and checks the manifest again.
   1. If `HEAD` is an ancestor of the fetched commit
      (`git merge-base --is-ancestor HEAD <sha>`), the remote already holds this state:
      print a note to stderr and exit 0.
   2. If the fetched commit is not an ancestor of `HEAD`, `git rebase <sha>`. On a
      conflict, `git rebase --abort` and stop with exit 1. The ledger worktree keeps its
      local commit, and the remote is not written.
   3. `git push --porcelain --force-with-lease=<ref>:<sha> <remote> HEAD:<ref>`, with
      `<ref>` the full ref name and `<sha>` the fetched commit. When the ref is absent on
      the remote, `<sha>` is empty and the lease requires the ref to stay absent. The lease is a compare and
      swap: the remote updates the ref only if it still holds `<sha>`, so a commit a
      concurrent writer pushed after the fetch is never overwritten.
   4. Read the status line for `<ref>` from the porcelain output. Flag `!` is a rejected
      update: print a note to stderr and go to the next attempt. Any other flag is
      success: exit 0. No status line for `<ref>` (a network or authentication failure,
      for example) stops with exit 1 and git's own error, without a retry.
6. After 5 rejected attempts, stop with exit 1. The local commit stays in the ledger
   worktree, and a later `tsk ledger push` with nothing to commit sends it.

A rejected update is detected from the porcelain status flag only, never from error
text. Flag `!` covers a stale lease checked by the client (`[rejected] (stale info)`)
and a ref update the remote rejects because the ref moved during the push
(`[remote rejected] (failed to update ref)`). Both lead to a retry.

On success, stdout carries the full SHA the ledger ref holds after the command and a
newline: the pushed commit, or the fetched commit when the remote already holds the
ledger worktree's state. Diagnostics go to stderr.

Each command that writes does one push after all its file changes, not one per file.

## External event commands

The `tsk events` commands replace `append-external-event.sh`,
`read-new-external-events.sh` and `advance-external-events-watermark.sh`. They run in the
`tsk` client alone, with no `tskd`. Each one runs `tsk ledger fetch`'s steps first, as the
scripts run `fetch-bootstrap-ref.sh`. The three that write push once through
`tsk ledger push`'s steps.

### `tsk events append <source> <event-type> <action> <repo> <payload-file>`

1. If `<payload-file>` is not a file, stop with exit 1 and
   `error: no such payload file: <payload-file>`. If it holds anything that is not JSON,
   stop with exit 1. Both checks run before the fetch, so nothing is fetched or pushed.
2. Fetch the ledger.
3. Append one envelope line to `external-events/queue.ndjson`, creating the directory
   and the file when absent. `payload` is the first JSON value in the file, or `null`
   when the file holds none, with whitespace outside strings removed and key order kept.
   `received_at` is the current UTC time.
4. Push with the message `External event queue: <source> <event-type> (<action>) on <repo>`.
5. Print `queued`.

The script wrote the payload through `jq -c`, which also rewrites some literals: `1e2`
becomes `1E+2` and `"\u00e9"` becomes `"é"`. The binary keeps each literal as the payload
file holds it. The two forms hold the same JSON value.

### `tsk events append-batch [--source <source>] [--action <action>]`

Appends many events with one fetch and one push. An external event producer, such as
`poll-security-alerts.sh`, pipes its events to this command and does not write the queue
file or the envelope format itself.

1. Read stdin to its end. Each non-blank line is one event: a JSON object with the
   string fields `event_type` and `repo`, the field `payload` holding any JSON value,
   and optionally the string fields `source` and `action`. A line's `source` or `action`
   overrides `--source` or `--action`. A blank line is skipped.
2. Check every line before the fetch. A line that is not a JSON object, lacks a required
   field, holds a field that is not a non-empty string where a string is required, holds
   a field not in the list above (`received_at` included), or has no `source` or `action`
   with no matching flag, stops the command with exit 1 and
   `tsk events append-batch: stdin line <n>: <detail>`, where `<n>` counts every
   physical line from 1. Nothing is fetched, appended or pushed.
3. With no events, print `queued:0` and exit 0. Nothing is fetched or pushed.
4. Fetch the ledger.
5. Append one envelope line per event to `external-events/queue.ndjson`, in stdin order,
   with the field order and format of `tsk events append`. Every envelope in the batch
   has the same `received_at`, the current UTC time. `payload` is the line's value with
   whitespace outside strings removed and key order kept.
6. Push once, with the message `External event queue: append batch of <n> event(s)`
   (`event` for one, `events` otherwise).
7. Print `queued:<n>`.

### `tsk events read-new`

Fetches the ledger, then prints one compact JSON object and writes nothing:
`{"new_count":N,"total_count":M,"events":[...]}`. `total_count` is the number of
`\n`-terminated lines in the queue. `events` holds lines `processed_through + 1` to the
end, oldest first, each compacted with key order kept, and `new_count` is their number.
With no queue file, or with `processed_through` at or past `total_count`, `events` is
empty and `new_count` is `0`. A queue line that is not JSON, or a watermark file that is
not a JSON object with a non-negative integer `processed_through`, stops with exit 1.

### `tsk events advance-watermark <count>`

1. `<count>` must be decimal digits only. Otherwise stop with exit 1 and
   `tsk events advance-watermark: count must be a non-negative integer, got '<count>'`,
   before the fetch.
2. Fetch the ledger and read `processed_through`.
3. A lower count stops with exit 1 and
   `tsk events advance-watermark: refusing to move the watermark backwards (<current> -> <count>)`.
4. An equal count prints `note: watermark already at <current>; nothing to advance.` to
   stderr, prints nothing on stdout, pushes nothing, and exits 0.
5. A higher count rewrites `external-events/watermark.json`, pushes with the message
   `External event queue: advance watermark <current> -> <count>`, and prints
   `watermark:<count>`.

The command does not check `<count>` against the queue length, as the script did not.

### `poll-security-alerts.sh`

This script stays a script. It calls GitHub's REST API with `curl` and a personal access
token from `GH_PAT`, for Dependabot, code scanning and secret scanning alerts, then
appends every open alert to the queue in one fetch, append, push cycle. The binary holds
no HTTP client. From T-07 the script pipes its alerts to
`tsk events append-batch --source github --action polled`, one line per alert, in place
of writing envelopes to the queue file and calling `fetch-bootstrap-ref.sh` and
`push-bootstrap-ref.sh`. This keeps one push per poll. Calling `tsk events append` per
alert would push once per alert. The GitHub Actions workflow that runs it
(`.github/workflows/external-security-events.yml`) then needs `tsk` installed.

## Session start command

`tsk thread session-start` is the binary side of the Claude Code `SessionStart` hook. It
runs the same fetch as `tsk ledger fetch`, so it materialises or refreshes the ledger
worktree, and it holds the one copy of the unbound prompt text that `tsk thread guard`
also prints.

Steps, in order:

1. Read standard input to the end. The hook input can be empty or not JSON.
2. Claim the event. When the input names a `session_id`, and optionally a `source`,
   made only of ASCII letters, digits, `-` and `_`, create the file
   `<state-root>/session-start/<session_id>.<source>` with exclusive create. `source`
   defaults to `unknown`. When the file exists and is less than 300 seconds old, a run
   for the same event holds the claim: exit with status 0 and no output. A file 300
   seconds old or more belongs to an earlier event with the same source, such as a
   second compact, so replace it and continue. Remove any claim file older than one
   day. Without a usable `session_id`, or on any I/O error, continue.
3. Fetch the ledger. A fetch failure, or a directory outside a git repository, skips to
   the failure output below.
4. When the environment variable `CLAUDE_ENV_FILE` is set and not empty, append
   `export TSK_LEDGER_WT="<path>"` and a newline to that file. A write failure adds a
   warning to the context and does not stop the command.
5. Resolve the binding of the session, as `tsk thread binding` does, from the fetched
   ledger worktree.
6. Print one JSON object on one line and exit with status 0.

Standard output on success:

```json
{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"..."}}
```

`additionalContext` is these parts, joined by single spaces:

1. `The ledger was fetched and materialised at <path> (also exported as $TSK_LEDGER_WT).`
   and, when the ledger has mission records, `Run `tsk mission list` to see the missions.`
   When it has none, the part is empty. A bound thread whose mission has a record gets
   instead the mission and its open tasks:
   `This thread works mission <id> (<status>): <name>.` and
   `Its open tasks follow, one per line: id, status, blocked-by, description, tab-separated.`
   and the lines `tsk task list` prints for the tasks that are not done. With none, it
   ends `It has no open tasks.`
2. When the ledger worktree holds commits that are not on the remote ledger branch, a
   warning that starts `WARNING: the ledger worktree holds a commit that is not on
   <remote>'s <ref>, so it was left as it is rather than reset:`, lists the commits
   separated by `;`, and gives the `git log -p <remote-tip>..HEAD` command to read them and
   `tsk ledger push` to push them.
3. With a binding, `An existing thread binding was found: <binding>. Run /tsk:resume-thread
   <thread-id> next.` Without one, the unbound prompt text. When the ledger has mission
   records, the prompt lists the missions not done, in the format of `tsk mission list`,
   and offers them; it does not mention `index.md`. Both prompts end by naming
   `tsk thread skip` as the choice for no thread. When this session or code worktree
   skipped a thread, the context says so in place of the prompt.
   When the ledger store holds local changes that are not committed (records or documents
   written since the last push), the fetch leaves them in place and does not reset or
   merge. The context adds `The ledger store holds local changes that are not pushed
   (records or documents written since the last push). The refresh left them as they are.
   A thread command pushes them with its own change, or run `tsk ledger push`.` The
   `thread` commands refresh the same way. The merge with the remote happens at the push.
4. When the ledger has no mission records and no mission documents, the note that names
   `tsk guide import`.
5. When the command is given `--plugin-version <semver>` and that version is lower than
   `EXPECTED_PLUGIN_VERSION`, the plugin version this binary expects, the sentence
   `The tsk plugin loaded in this session is <given>; this tsk expects <expected>. Update
   the plugin (Claude Code: /plugin update tsk; or start a new session), because its
   skills describe an older command set.` An equal or higher version, an absent flag and a
   value that is not `major.minor.patch` add nothing. The plugin's `session-start.sh`
   reads its own version from `.claude-plugin/plugin.json` and passes it.

When the fetch finds a version 1 ledger, the context is `The ledger at <path> (also
exported as $TSK_LEDGER_WT) is at layout version 1: its documents are still in Dolt. Run
`tsk ledger migrate-docs` once to move them to the tsk/docs branch. Until then every tsk
command except `tsk ledger fetch` stops with an error that names this command.` Nothing
else is added.

On failure the output is the same object with the context
`Warning: the ledger could not be fetched automatically at session start (<error>). Run
`tsk ledger fetch` manually before reading the missions.` and the exit status is 0.

The command does not unshallow the clone, install plugins, or stash, check out or pull
the code worktree. Those steps stay in the repo's own hook script.

Two hooks call the command for one event in the tsk repo: the plugin's hook and
`ops/local/claude-session-start.sh`. Claude Code reads plugin hooks once, when its
process starts. The repo script installs the plugin during startup, so the plugin's
hook is absent from that process, and `/clear` starts a new session in the same process
without reading hooks again. The repo script runs the command on every event so that
`$TSK_LEDGER_WT` is set in every session. The claim in step 2 stops the second run from
fetching the ledger again and repeating the context.

`CLAUDE_ENV_FILE` is a file Claude Code creates for one session ID, under
`~/.claude/session-env/<session-id>/`. Claude Code runs it before each Bash command in
that session. A new session ID, such as the one `/clear` creates, gets a new, empty file.

## Script outputs

The skills and hooks parse these outputs today. The binary commands that replace the
thread scripts (T-05) and the external event scripts (T-06) print the same stdout and
exit with the same status, so T-07
switches the harness over by changing only the command it calls. This table records
what the harness expects until then.

| Script | Stdout on success | Other exits | Replaced by |
|---|---|---|---|
| `thread-start.sh` | `started:<thread-id>` | `resume-required:<thread-id>` and exit 2 when a binding exists | `tsk thread start [<mission-id>]` |
| `thread-append-handover.sh` | `paused:<thread-id>` | | `tsk thread pause <thread-id> <task-id> <whats-next>` |
| `thread-resume.sh` | `{"thread_id":"...","latest":{...},"warning":"..."}` (`latest` is `{}` with no entry; `warning` is empty unless another actor wrote to the thread) | | `tsk thread resume <thread-id>` |
| `thread-detach.sh` | `detached:<thread-id>` | exit 1, nothing on stdout, with no binding | `tsk thread detach` |
| `thread-stop.sh` | `stopped:<thread-id>` | | `tsk thread stop [<thread-id>]` |
| `thread-list.sh` | one compact JSON object per line: `id`, `mission_id` (`null` for a thread with no mission), `mission_link`, `latest_whats_next`, `latest_timestamp` (both `null` with no entry), `actors` (an array of `{"actor", "kind"}`, one for each session or worktree bound to the thread, oldest binding first; `kind` is `cloud` or `worktree`; empty when none is bound) | | `tsk thread list` |
| `thread-resolve-binding.sh` | `cloud:<thread-id>` or `worktree:<thread-id>` | exit 1, nothing on stdout, with no binding | `tsk thread binding`; `--no-fetch` in place of passing an already fetched ledger worktree |
| (none) | `mission-set:<thread-id>:<mission-id>`, or `mission-cleared:<thread-id>` with `--none` | exit 1 for an unknown mission or thread, or with no thread ID and no binding | `tsk thread set mission <mission-id> [<thread-id>]`, `tsk thread set mission --none [<thread-id>]` |
| `thread-binding-guard.sh` | nothing when bound, after `tsk thread skip`, or when the ledger holds no missions; `{"decision":"block","reason":"..."}` otherwise | | `tsk thread guard` |
| `claude-session-start.sh` (the tsk parts) | one JSON object, `{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"..."}}` | exit 0 in every case, including a failed fetch, which prints a warning context | `tsk thread session-start` (see [Session start command](#session-start-command)) |
| `thread-scaffold.sh` | nothing | exit 1 when `threads/<thread-id>` exists | internal to `tsk thread start` |
| `thread-mint-id.sh` | `<thread-id>` | | internal to `tsk thread start` |
| `append-external-event.sh` | `queued` | | `tsk events append <source> <event-type> <action> <repo> <payload-file>` |
| `poll-security-alerts.sh` | `queued:<count>`, or `no open alerts queued` | | not replaced: stays a script, calling `tsk events append-batch` from T-07, which prints `queued:0` for zero events (see [External event commands](#external-event-commands)) |
| `read-new-external-events.sh` | `{"new_count":N,"total_count":M,"events":[...]}` | | `tsk events read-new` |
| `advance-external-events-watermark.sh` | `watermark:<count>` | exit 0 and no stdout when already at the count; exit 1 when the count is lower | `tsk events advance-watermark <count>` |

## Differences from `tsk/bootstrap`

| | `tsk/bootstrap` | Ledger |
|---|---|---|
| Ref | `refs/heads/tsk/bootstrap` | `refs/heads/tsk/ledger`, or `refs/heads/ledgers/<repo-id>` in the nexus |
| Ledger worktree | `.../repos/<clone-id>/bootstrap` | `.../repos/<clone-id>/ledger` |
| Manifest | none | `.tsk-ledger.toml`, `version = 1` |
| Continuation commit fields | flat `commit_on_bootstrap`, `commit_on_main` | nested `git.ledger.commit`, `git.code.ref`, `git.code.commit`; the flat names are not read |
| Fetch and path | `fetch-bootstrap-ref.sh`, `bootstrap-wt-path.sh` | `tsk ledger fetch`, `tsk ledger path` |
| Thread commands | `thread-*.sh`, `mint-token-lib.sh` | `tsk thread` subcommands |
| Push | `push-bootstrap-ref.sh`, plain push, retried on any failure | `tsk ledger push`, explicit lease, retried on a rejected update only |
| Legacy ledger worktree migration | moves `.git/tsk/bootstrap-ref-wt` (ADR 0009) | none: the ledger worktree has no earlier location |
