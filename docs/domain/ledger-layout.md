# Ledger layout

Reference for the tsk ledger: the Dolt database that holds its records, the second branch
that holds its documents, where the ledger lives, the ref that holds it, and the
commands that read and write it. The files backend that kept the whole ledger as files
in a git branch was removed in 0.16.0: see [File ledger (removed)](#file-ledger-removed).

Depends on: `docs/domain/session-continuation-design.md` (threads, bindings and
continuation state), `docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md`,
`docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md`,
`docs/adr/0010-ledger-stays-a-branch-not-a-directory-on-main.md`,
`docs/adr/0016-a-custom-ref-is-the-default-for-a-new-ledger.md`.

## Contents

- [Dolt backend](#dolt-backend)
- [Mission commands](#mission-commands)
- [Task, decision and question commands](#task-decision-and-question-commands)
- [File ledger (removed)](#file-ledger-removed)
- [Location and ref](#location-and-ref)
- [`missions/`](#missions)
- [Threads](#threads)
- [`external-events/`](#external-events)
- [Local-only files](#local-only-files)
- [Common value formats](#common-value-formats)
- [Fetching and printing the path](#fetching-and-printing-the-path)
- [Writing to the ledger](#writing-to-the-ledger)
- [External event commands](#external-event-commands)
- [Session start command](#session-start-command)
- [Script outputs](#script-outputs)

## Dolt backend

The default ledger backend stores the ledger's data in a Dolt database and its documents
as Markdown files on a second branch, both synced over git. The decisions are in
[ADR 0014](../adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md) and
[ADR 0015](../adr/0015-the-files-backend-is-removed.md). The documents are files on the
branch `tsk/docs` (see [Documents](#documents)). The ledger refs, the nexus rules and the
local-only files are described from [Location and ref](#location-and-ref) on.

### Backend

A `LedgerStore` trait in the `tsk` binary names every read and write the ledger supports.
The Dolt store is its one implementation (ADR 0015 kept the trait and removed the second
one). There is no backend setting. A ledger is a Dolt database.

The files backend was selected by `[ledger] backend = "git"` in the user config or by
`TSK_LEDGER_BACKEND=git`. A `tsk` that finds either set to `git` prints one line and
exits with status 1:

```
error: the files ledger backend was removed in 0.16.0; use tsk 0.15.1 to read an old file ledger
```

The environment value wins over the config, and an empty value counts as unset. Any value
other than `git` is ignored.

### Location

The Dolt database is a `dolt clone` at:

```
${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger-db
```

- `tsk ledger path` prints this path. `$TSK_LEDGER_WT` names it.
- The clone has one remote, `origin`. On the `file-remote` transport (the default from
  0.21.0) its URL is `file://<state>/tsk/repos/<clone-id>/ledger-remote`, a directory that
  `tsk` keeps as a linked worktree of the code repository (see [Transport](#transport)).
  On the older `git-remote` transport it is added with `--ref refs/tsk/ledger` for an
  in-repo ledger, or `--ref refs/tsk/ledgers/<repo-id>` for a nexus ledger, or the
  `refs/heads/...` form of either on a ledger that uses branches (see
  [Location and ref](#location-and-ref)). Its URL is `git+<remote url>`, with the remote
  URL resolved as in [Location and ref](#location-and-ref).
- The database needs the external `dolt` binary, version 2.5.0 or later. The `SessionStart` hook
  installs it.
- No file in `ledger-db` is edited by hand. The `tsk ledger` commands are the interface.
- The documents have their own worktree, `docs`, beside `ledger-db` (see
  [Documents](#documents)).

### Transport

The Dolt data reaches origin in one of two ways. The ledger ref says which, and so does the
clone's own store. [ADR 0017](../adr/0017-the-ledger-is-a-dolt-file-remote-carried-by-plain-git.md)
gives the reasons.

| Transport | What the ledger ref holds | The store's `origin` | Who talks to origin |
|---|---|---|---|
| `file-remote` (default from 0.21.0) | A Dolt `file://` remote directory: `manifest`, table files, `.tsk-transport`, `.gitignore`. Zero-byte `LOCK` and `oldgen/LOCK` files are ignored by git | `file://<state>/tsk/repos/<clone-id>/ledger-remote` | `tsk`, with `git fetch` and `git push`. Dolt reads and writes a local directory |
| `git-remote` (before 0.21.0, a fallback) | Dolt's own git blobstore | `git+<remote url>` with `--ref` | Dolt, through git processes it starts |

- `.tsk-transport` holds the text `file-remote`. A ref without it holds the `git-remote`
  layout. The store's transport is the URL scheme of its `origin` remote. For a
  `file-remote` store the ledger ref is recorded in `ledger-ref` beside `ledger-db`, because
  Dolt records no ref for a file remote.
- A new ledger uses `file-remote`. `TSK_LEDGER_TRANSPORT=git-remote` makes a new ledger use
  `git-remote`; any other value but `file-remote` is an error. The variable has no effect
  on a clone of a ledger that exists: the ref decides.
- The directory `ledger-remote` is a detached linked worktree, as `docs` is. A fetch resets
  it to origin's commit, so anything in it that origin lacks is dropped. A new ledger
  starts it from an orphan commit that holds `.tsk-transport` and `.gitignore`.
- The manifest version stays at 8. No table changed. A `tsk` older than 0.21.0 cannot read
  a `file-remote` ref and is expected to stop with a Dolt error, not with `unsupported ledger version`.
- `tsk ledger migrate-transport` converts a `git-remote` ledger once (see below). A store
  on `git-remote` whose origin now holds a `file-remote` ref moves to it on its next fetch
  that finds the ref changed: it points `origin` at the directory and keeps its commits,
  including those not pushed.

### Tables

A `mission_id` column in any table below holds the internal ID of a mission, `missions.id`. It is not the mission ID a person types, which is `missions.code`.

| Table | Columns | Content |
|---|---|---|
| `manifest` | `k` varchar(64), primary key; `v` text | The manifest keys as rows: `version`, `repo_id` for a nexus ledger, `mission_prefix` when set, and `ledger_id` (version 12), a UUID that names the ledger in `urn:tsk:ledger:<ledger-id>`. The `version` row is `14`. |
| `threads` | `thread_id` varchar(64), primary key; `briefing_path` varchar(1024), null; `mission_id` varchar(64), null | One row per thread. `mission_id` is the internal ID of the mission the thread works (the code itself for a mission with no record), and is null for a thread with no mission. Every command reads it through `missions.code`, so a renamed mission shows its new ID. `briefing_path` is the mission's briefing document, stored only when the mission has no record. There is no foreign key from `mission_id` to `missions`, because a thread made before mission records names a mission with no record. A thread made before version 5 has a null `mission_id`: the reader derives it from the file name of `briefing_path` (`M-BOOT-04` from `M-BOOT-04-official-ledger.md`) and never writes it back. |
| `continuation_entries` | `thread_id` varchar(64); `seq` int; `entry` longtext. Primary key (`thread_id`, `seq`) | One row per continuation state entry, `seq` from 1. `entry` is the JSON line, in the format of [`continuation-state.jsonl`](#threadsthread-idcontinuation-statejsonl). |
| `thread_bindings` | `instance` varchar(255), primary key; `kind` enum(`cloud`, `worktree`); `thread_id` varchar(64); `registered_at` varchar(32) | One row per bound harness instance, of either kind (version 12, ADR 0030; `bindings`, with the column `actor`, from version 8 to 11, and `cloud_session_bindings` below 8). The key of a cloud session is the session ID (`cse_...`). The key of a local code worktree is `<clone-id>/<worktree-name>`: the ID in `.git/tsk-clone-id`, a slash, and `main` for the main worktree or the name of the linked worktree. A bind sets or replaces the instance's row. `thread stop` deletes every row that names the stopped thread. There is no foreign key to `threads`. |
| `actors` | `name` varchar(200), primary key; `kind` enum(`human`, `robot`); `registered_at` varchar(32); `uid` varchar(36), unique (version 12) | One row per actor: a human or a robot. The `uid` is the identity every link names (ADR 0030): minted at `tsk actor register` for a new name, and derived from the name at the upgrade to version 12, so two clones that upgrade on their own write the same value. The name can change; `actor_names` keeps the history. Nothing deletes a row. Version 9. |
| `instances` | `instance` varchar(255), primary key; `kind` enum(`cloud`, `worktree`); `actor_name` varchar(200); `registered_at` varchar(32) | One row per harness instance that registered as an actor before version 12. Since version 12 an actor belongs to a session (`session_actors`), and `tsk actor register` still writes this row so a reader of the older link finds it; nothing else reads it except the fallback of `tsk actor name` on an instance that has no recorded session. The key is the one `bindings.actor` holds: the session ID of a cloud session, or `<clone-id>/<worktree-name>` of a code worktree. `tsk actor register` sets or replaces the instance's row. There is no foreign key to `actors` or to `bindings`. A registration is separate from a binding: an instance may be registered and unbound, or bound and unregistered. Version 9. |
| `sessions` | `session` varchar(255), primary key; `instance` varchar(255); `kind` enum(`cloud`, `worktree`); `started_at` varchar(32) | One row per session that ran `tsk thread session-start`. The key is the session URN, `urn:tsk:session:<harness>:<id>` (ADR 0029): the harness that minted the ID, then the ID. A cloud session is named by the cloud session ID, which is also its instance key. Any other session is named by the ID the harness gives its hook. `instance` and `kind` are the key and kind of the harness instance the session ran in, as `instances` and `bindings` hold them. A code worktree runs many sessions, one for each start and each `/clear`, so this table maps a session back to its instance. A second start of the same session changes nothing. There is no foreign key. `tsk session find` and `tsk session list` read it. Version 11. |
| `actor_names` | `actor_uid` varchar(36); `name` varchar(200); `from_at` varchar(32); `until_at` varchar(32), null. Primary key (`actor_uid`, `from_at`) | One row per name an actor has held: from `from_at` until `until_at`, which is null for the name it holds now. `tsk actor register` adds the first row of a new actor; the upgrade to version 12 adds one open row per existing actor. Version 12. |
| `session_actors` | `session` varchar(255); `actor_uid` varchar(36); `role` enum(`primary`, `supporting`); `joined_at` varchar(32). Primary key (`session`, `actor_uid`) | The actors of a session (ADR 0030): one primary, any number supporting. `tsk actor register` makes the registering actor the primary of the current session; `tsk actor join <uid> [--primary]` adds one. The session start copies the rows of the session a new session continues from. The upgrade to version 12 gives every recorded session the actor its instance was registered as, as primary. Version 12. |
| `external_events` | `position` bigint, primary key; `envelope` longtext | The external event queue, `position` from 1. `envelope` is the JSON line, in the format of the [event envelope](#event-envelope). |
| `watermark` | `id` int, primary key; `processed_through` bigint; `updated_at` varchar(32) | One row, `id` 1, the processed position of the queue. No row reads as `0`. It replaces `watermark.json`. |
| `missions` | `id` varchar(64), primary key; `code` varchar(64), null, unique; `name` varchar(255); `category` enum(`operational`, `administrative`); `status` enum(`todo`, `in-progress`, `done`), default `todo`; `parent` varchar(64), null, foreign key to `missions(id)`; `purpose`, `decision_authority` longtext, null; `next_objective`, `next_task`, `next_question`, `next_constraint`, `next_out_of_scope` int, default 1 | One row per mission. `id` is the internal ID: every other table names the mission by it, and it never changes. `code` is the mission ID a person types and sees, such as `M-NAMES`; a rename changes `code` and nothing else, and a `code` that is null reads as the `id`. A mission made before layout 10 has its first ID as both. A mission made from layout 10 on has a random UUID as `id`. Empty text is stored as null. Each `next_` column is the number the next objective outcome, task, question, constraint or out-of-scope item gets. It only grows, so a number is never reused. A ledger below version 7 has two more columns, `constraints_text` and `out_of_scope` (longtext, null), in place of the last two counters. |
| `mission_constraints` | `mission_id` varchar(64), foreign key to `missions(id)`; `seq` int; `text` longtext. Primary key (`mission_id`, `seq`) | One row per constraint of a mission, one line each. |
| `mission_out_of_scope` | `mission_id` varchar(64), foreign key to `missions(id)`; `seq` int; `text` longtext. Primary key (`mission_id`, `seq`) | One row per out-of-scope item of a mission, one line each. |
| `objective_outcomes` | `mission_id` varchar(64), foreign key; `seq` int; `outcome` longtext; `met` boolean, default false. Primary key (`mission_id`, `seq`) | One row per objective outcome. |
| `mission_blocks` | `mission_id` varchar(64), foreign key; `blocked_by_id` varchar(64), foreign key to `missions(id)`. Primary key of both | One row per block. Whether a block still holds is derived, never stored. |
| `mission_documents` | `mission_id` varchar(64), foreign key; `path` varchar(1024); `doc_id` varchar(8), null (version 14); `role` enum(`intelligence`, `report`, `design`). Primary key (`mission_id`, `path`); unique key (`mission_id`, `doc_id`) | One row per document a mission links. `doc_id` is the id in the file name of the document, and it is the link. `path` is the path the file has now, kept so an older reader still finds it, and it is refreshed when the document is renamed. |
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
  The variable is not in Dolt's documentation. Measured with Dolt 1.88.2 and 2.5.0, with the same result: a name sets
  the branch's name, and an empty string writes no branch. The conformance suite checks
  that a push leaves no branch on the remote other than the ledger ref.

The binary also sets git protocol version 0 for every `git` process it starts, and for
the `git` processes that `dolt` starts for it. It adds the entry `protocol.version=0`
to `GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_n` and `GIT_CONFIG_VALUE_n` in the child's
environment, after any entries the user already set. The user's own git configuration
stays as it is. Version 2 sends two HTTP requests for a fetch with nothing new, and
version 0 sends one; a live fetch of this ledger took 2.65 s on version 2 and 1.9 s on
version 0 (median of 6 interleaved runs). Two things leave the setting out:

- `TSK_GIT_PROTOCOL=default`, for a server that rejects version 0.
- A `protocol.version` entry in the user's own `GIT_CONFIG_COUNT` list, which stands.

`git.ledger.commit` in a continuation entry holds the Dolt commit hash with the Dolt
backend.

### The local Dolt server

Everything above describes the path with the server off: every SQL batch is a `dolt sql`
process.
[ADR 0018](../adr/0018-a-local-dolt-server-per-ledger-store-behind-a-switch.md) adds a
second way to reach the same store, which `tsk` uses unless the switch says off
(`TSK_DOLT_SERVER=off`, or `[ledger] server = "off"` in the user config; the environment
wins; the default is on since 0.29.0, and off in 0.23.0 to 0.28.x). Where the server cannot
start, the command falls back to `dolt sql` and prints one `note:` line. `tsk config
ledger-server` prints the switch and writes it.

- **What it is.** One `dolt sql-server` for each ledger store whose `origin` is a file
  remote. The first command that needs it starts it, under a file lock, through a detached
  supervisor (`tsk ledger server run`). `tsk` talks to it over `<store>/.tsk-dolt.sock`
  (`/tmp/tsk-dolt-<uid>-<hash>.sock` when that path is over 100 bytes), with an own MySQL
  client that returns the rows in the JSON shape of `dolt sql -r json`. Dolt also binds a
  free port on `127.0.0.1`, because it does not start without one.
- **Files**, in the state directory of the clone, beside `ledger-db`: `dolt-server.pid`
  (`pid=`, `supervisor=`, `state=starting|ready`, `socket=`, `port=`, `store_id=`,
  `started=`, `idle=`), `dolt-server.lock`, `dolt-server.log`, `dolt-server.yaml`,
  `dolt-server.seen` (touched by each connection) and `dolt-server-cfg/` (Dolt's
  privileges). The store gets one file, the socket.
- **What it runs.** All SQL, and the procedures for `fetch`, `push`, `reset --hard`,
  `clean`, `add -A`, `commit -m`, `merge`, `merge --no-commit` and `merge --abort`. The
  commit identity of a connection comes from `DOLT_AUTHOR_NAME` and its siblings, then from
  the global Dolt configuration. `init`, `clone`, `remote` and `config` stay on the
  command line, and so does everything for a store on Dolt's git blobstore.
- **When it is replaced.** A command probes the socket with `select 1` (5 s). Refused or
  stale: the files are cleared and a server starts. Silent: the processes are killed with
  `SIGKILL` and a server starts. `tsk thread session-start` does this check first, in the
  background, so a cloud session that woke from hibernation finds a live server.
- **When it stops.** After `TSK_DOLT_SERVER_IDLE_SECS` (1800 since 0.25.0; 120 before) with no connection, or when
  the store directory goes, or on `tsk ledger server stop`.
- **The command line beside it.** While the server runs, `dolt` opens the store read only:
  reads work and print typed JSON, and a write fails with `database is read only`. A
  `tsk` with the switch off stops the server of a store when its first write is refused.
  A `tsk` older than 0.23.0 does not, so it needs `tsk ledger server stop` or
  `tsk self-update`. A person who writes with `dolt` runs `tsk ledger server stop` first.
- **Commands.** `tsk ledger server status|start|stop`.

### Documents

Documents are plain Markdown files on a second branch, `tsk/docs`. The rule: structured
data goes in Dolt as typed rows, and free text goes on `tsk/docs` as files. Dolt owns the
whole tree of the ledger branch, and each `dolt push` writes a git tree of Dolt's own
files. A Markdown file on that branch would be dropped at the next push, so the documents
need a branch of their own. Any tool can open them: GitHub, an editor, `marq`.

| Item | Value |
|---|---|
| Ref, in-repo ledger | `refs/tsk/docs` (`refs/heads/tsk/docs` in `branches` mode) on the same remote as the ledger ref |
| Ref, nexus ledger | `refs/tsk/ledgers/<repo-id>-docs` (`refs/heads/ledgers/<repo-id>-docs` in `branches` mode) on the nexus remote |
| Local checkout | `${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/docs`, a detached linked worktree of the managed clone |
| First commit | An orphan commit holding `README.md` ("Documents of the tsk ledger. Written by tsk ledger doc put.") and the seed `index.md`, made with git plumbing |
| Manifest | None. The documents branch has no manifest |

The nexus ref ends in `-docs` and not `/docs`. Git cannot hold both `ledgers/<repo-id>`
and `ledgers/<repo-id>/docs` as refs, because one would be a file and the other a
directory.

The worktree, the refresh and the compare-and-swap push (`cli/src/ledger/fetch.rs` and
`push.rs`) run with the documents ref. The path rules are those of a file in git: no
absolute path, no `..`, no `.git` component. No name is reserved.

#### The id of a document

From layout 14 a document carries its own identity in its file name: an id of 8 lowercase
hexadecimal characters, unique within the ledger, at the end of the name and before the last
extension. `missions/administrative/M-TSK-ADHOC/report-renaming-missions-2026-10-25.3f2a9c1e.md`
is the document with the id `3f2a9c1e`. A name with no extension ends in `.<id>`. The URN is
`urn:tsk:document:<ledger-id>:<id>` (`docs/domain/urn-scheme.md`). A multi-file document, when
one arrives, is a directory named in the same form.

The file name is the identity, so the index on the record side can be rebuilt from the branch:
`mission_documents.doc_id` names the id, and the path is whatever the tree holds. A rename of
the file changes the path and keeps the id, so `tsk ledger doc mv` rewrites the `path` of the
links and no link is lost. A document written without an id (`doc put missions/x/report.md`)
gets a minted one, drawn again when the ledger already uses it. Every command that takes a
document accepts the path with the id, the path without it, or the id alone. A path without
the id names the file in the same directory that has the name with an id, so a second `doc put
missions/x/report.md` rewrites `missions/x/report.<id>.md`. The two seed files of a new
branch, `README.md` and `index.md` with their seed text, have no id.

Six commands read and write the documents:

| Command | Effect |
|---|---|
| `tsk ledger doc get <ref>` | Prints the body of the document `<ref>` names. |
| `tsk ledger doc put <path> [<file>]` | Stores the content of `<file>`, or stdin when no file is given, as the document at `<path>`, and prints the path it has, with its id. Writes the documents worktree only. |
| `tsk ledger doc list [<prefix>]` | Prints each document path under `<prefix>` (all when omitted) and its id, aligned (`-` for a file with no id). `--porcelain` separates them with a tab. |
| `tsk ledger doc rm <ref>` | Removes the document. Writes the documents worktree only. |
| `tsk ledger doc mv <ref> <path>` | Renames the file, keeps the id, and updates `path` in `mission_documents` for every mission that links the id. |
| `tsk ledger doc path <ref>` | Prints the absolute path of the document's file, for opening it in another tool. Exits 1 with `error: no such document: <ref>` when it is absent. |

`tsk mission link <mission> <ref> --role <role>` links by the id the reference names.
`tsk mission list <id> --json` prints `path`, `id`, `urn` and `role` for each linked document.

`doc put` and `doc rm` do not reach the remote. `tsk ledger push "<message>"` does.

`tsk thread start` takes a mission ID, or none. It refuses an ID that has no mission record. With no ID it mints a thread with no mission. `tsk thread set mission` applies the same check. A ledger from before the mission records also accepts a briefing document at `missions/<category>/<ID>-<slug>.md`.

`tsk ledger fetch` refreshes both stores: the Dolt database, then the documents
worktree. It creates the worktree when it is absent. It refuses a worktree with
uncommitted changes, as it does for the database, and notes a local commit that is not on
the remote. It still prints the path of the Dolt database.

### The documents hook point

A repo may check its ledger documents with an executable script named `pre-docs-commit`.
`tsk` owns the hook point and knows nothing about what a script does: any tool can check
the documents through it. The script is looked up in four places, and the first place
that has the file wins:

| Order | Place | Path |
|---|---|---|
| 1 | Environment | The file `TSK_DOCS_HOOK` names. Set to a path that is not a file, it is an error that names the path. |
| 2 | Clone | `<state>/repos/<clone-id>/hooks/pre-docs-commit`, in the state directory of the clone, next to `ledger-db`. |
| 3 | User | `${XDG_CONFIG_HOME:-$HOME/.config}/tsk/hooks/pre-docs-commit`. |
| 4 | Repo | `.tsk/hooks/pre-docs-commit` at the root of the code worktree, next to `.tsk/config.toml`. Versioned and reviewed with the code. |

A place with no file is skipped. A file that exists and is not executable is an error that
names its path. A person who cannot commit a file to a repo (it is not theirs) uses the
environment, clone or user place. `tsk config docs-hook` prints the script that applies
and the place it came from, or `docs hook: none`, and exits 0:

```text
docs hook: /home/me/.config/tsk/hooks/pre-docs-commit
source: user (<config>/tsk/hooks/pre-docs-commit)
```

| Item | Value |
|---|---|
| Arguments | The changed document paths, relative to the documents worktree, one argument each. A deleted document is not passed. |
| Current directory | The documents worktree. |
| Environment | `TSK_DOCS_WORKTREE` is the absolute path of the documents worktree. |
| Output | Passed through unchanged. On exit 0 stdout and stderr go to the same streams of `tsk`. On a non-zero exit both go to stderr of `tsk`, followed by one `error:` line that names the hook and the exit status. |
| Skip | `TSK_SKIP_DOC_CHECK=1` in the environment. `tsk` then prints `note: TSK_SKIP_DOC_CHECK is set; skipped the documents hook <path>` on stderr. |

The hook runs at two points:

1. `tsk ledger doc put <path>`, after the document is written to the documents worktree,
   with that one path. A non-zero exit makes `doc put` exit 1. The document stays written
   in the local worktree, so the author corrects it and puts it again.
2. `tsk ledger push` and the background publish (`tsk ledger push --background`), before
   the documents commit, once, with every document changed since the last push: the
   documents `tsk ledger status` lists. With no changed document it does not run. A
   non-zero exit stops the push before anything is pushed and exits 1. The local state
   stays. The background publish stores the output in its result record, and the warning
   of the next command shows its first 20 lines.

A hook that cannot start (its interpreter line names a program that is not there, say)
is an error that names the path. When no place has a file, nothing runs. `tsk thread start`, `pause` and `stop` push through the
same code and run the hook too.

The repo place is the code worktree of the repo the command runs in, the way
`.tsk/config.toml` is. For a ledger kept in a nexus, the nexus holds the ledger and the
documents, and the repo whose ledger it is holds the hook: a command run in repo A uses
`.tsk/hooks/pre-docs-commit` of repo A, and the nexus repo needs none. A repo with several
code worktrees uses the file of the worktree the command runs in. The clone place is the
state directory of the clone, whatever holds the ledger.

### Git hooks in the ledger worktrees

The documents worktree and the ledger transport worktree are linked worktrees of the
managed clone of the code repo, so `git rev-parse --git-common-dir` in them is the code
repo's `.git`, and git would run the code repo's hooks (`pre-commit`, `commit-msg`,
`pre-push`, `post-checkout` and the rest) for the commits, merges, checkouts and pushes
`tsk` makes there. A hook written for the code (a lint of staged Markdown, say) stopped
`tsk ledger push` before it pushed anything. `tsk` therefore starts every git process of
the ledger with `core.hooksPath` set to `/dev/null`, so no git hook runs for them. The
setting goes in the environment (`GIT_CONFIG_COUNT` and its keys, the way `gitenv.rs` sets
`protocol.version`), so the arguments of the command stay the plain ones and the setting
reaches any git process that git starts. `--no-verify` would skip only `pre-commit` and
`commit-msg`. The setting is made in one place, the `Git` helper of
`cli/src/ledger/git.rs`. The one helper for the user's own code worktree,
`Git::in_code_worktree`, keeps the hooks. See ADR 0025.

### Push

`tsk ledger push "<message>"` with the Dolt backend:

1. Runs `dolt add -A`, then `dolt commit` with the caller's message when anything is
   staged. With nothing staged it prints a note and continues.
2. Pushes the documents. It commits the changes in the documents worktree, then pushes
   `HEAD` to the documents ref with a compare and swap, and rebases onto a newer remote
   tip, up to 10 attempts. A rebase conflict exits 1 and names the files both sides
   changed. A side that has no change and no unpushed commit sends nothing.
3. Makes up to 10 attempts at the Dolt data, with a random wait of 50 to 250 ms before each
   retry:
   1. Fetches. On `file-remote`: `git fetch` of the ledger ref (skipped when the up-to-date
      check finds origin unchanged and the directory at that commit), `git reset --hard` of
      the `ledger-remote` directory to it, then `dolt fetch origin`, which reads the
      directory. On `git-remote`: `dolt fetch origin`. If `HEAD` already equals
      `origin/main` or is its ancestor, the remote holds this state: exit 0.
   2. When both sides hold commits the other lacks, renumbers the local rows that took
      a number the remote also took (see [Numbers and collisions](#numbers-and-collisions)),
      in one local commit. Then `dolt merge origin/main`, then reads `dolt_conflicts` and
      `dolt_constraint_violations`. Any row aborts the merge with `dolt merge --abort`,
      drops the renumbering commit, and the command exits 1 and names the conflicting
      tables.
   3. `dolt push origin main`. On `file-remote` that writes into the directory; `tsk`
      then commits the directory and runs a plain `git push` of the commit to the ledger
      ref: no force and no lease. Git rejects it when origin moved. A rejected push prints
      a note and goes to the next attempt, from the fetch. Any other failure stops with
      exit 1. Before the `dolt push`, `tsk` clears the record of the commit it last fetched, so a process
      that dies before the `git push` fetches next time.

The documents go first, and the Dolt data only after them. A record must never name a
document the remote lacks. A document that no record names is harmless. So when the
documents push fails, because the remote rejects it after the retries or the rebase
conflicts, `tsk ledger push` exits 1 and does not push the Dolt data. Both sides keep
their local commits, and the next `tsk ledger push` resumes.

The local commit stays in the database after a conflict or a failure, and a later
`tsk ledger push` sends it. Dolt merges by row and cell, so two writers that change
different rows, or different cells of one row, do not conflict. Two writers that append
an external event at the same moment take the same `position` and conflict.

On success, stdout carries one line per renumbered row, then the Dolt commit hash, or the
documents commit when only the documents changed. Notes go to stderr.

### Numbers and collisions

Tasks, questions, objective outcomes, constraints, out-of-scope items and decisions are
rows keyed by the mission and a number. A clone mints the next number for its own rows.
Two clones that each add one to the same mission before they push mint the same number.
Dolt cannot merge two new rows with one key, so the second push would stop with a
conflict. A number is a short name that people say across sessions, so tsk keeps numbers
and renumbers the second clone's rows instead of using random ids.

At push, after the fetch and before the merge, tsk compares the local unpushed history
with the remote since their merge base. A local row that is new, with a key the remote
also added, takes the next free number for its mission and table. That number is higher
than every number either side holds, and higher than a number a removal freed. In one
transaction and one local commit, `Renumber after a collision`, tsk:

- gives the row its new number;
- updates the rows that name a renumbered task: the `task_blocks` rows and the `task_seq`
  of the decisions;
- leaves every other row alone. A row the clone did not add never changes its number, and
  neither does a new row whose number is free on the remote.

Then the merge runs with no key collision left, and the push completes. The command
prints one line per renumbered row on stdout, before the commit hash:

```text
T-03 is now T-05 in M-X
Q-02 is now Q-03 in M-X
objective 2 is now objective 3 in M-X
constraint 2 is now constraint 4 in M-X
out-of-scope 1 is now out-of-scope 2 in M-X
decision 2 is now decision 3 in M-X
```

A row that is renumbered again by a second attempt of the same push keeps one line, from
its first number to its last. Text that names an old number, such as a decision or a
document that says `T-03`, does not change. The printed lines are the record, so a caller
reads them and uses the new numbers.

The counters `next_task`, `next_question`, `next_objective`, `next_constraint` and
`next_out_of_scope` on the mission row are one cell each. When both clones moved the same
counter to different values, Dolt reports a conflict on `missions`. Before the merge, tsk
sets the local counter back to its value at the merge base, so the remote's value wins the
merge. After the merge and before its commit, tsk raises each such counter to the highest
of the two clones' values and one more than the highest number in the merged table. This
runs for a counter whose rows were renumbered, and for a counter that both clones moved
without a collision. A counter never goes down, so a number that a removal freed is not
reused.

A conflict that is not a counter cell, such as both clones editing one field, keeps the
old behaviour. The push exits 1 and names the tables. The renumbering commit is dropped
with the merge, so the local rows keep the numbers the clone gave them. A mission that
both clones created is a conflict of its own, and its rows are not renumbered. There is
no flag to turn renumbering off.

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
  with a zero count is left out, and one commit is singular.
- When the fetch fails, one line on stderr says so (`note: could not fetch ...; reporting
  from what is local`), and `Origin` says `unknown`.

`tsk ledger status --local` makes no network call. It prints the `Local` block and `Origin  not checked (--local)`, and with `--porcelain` the two remote lines read `not checked`. A refresh, `status`, `thread list` and `thread session-start` first run one `git ls-remote` of both ledger refs and skip the fetch when both tips match the tips last fetched, which are recorded in `<state>/repos/<clone-id>/remote-tips` after each successful fetch. When `ls-remote` fails, they fetch as before. A clone with no ledger store, or no record of a fetch, runs one `git fetch` of both refs instead (into `refs/tsk-fetch/ledger` and `refs/tsk-fetch/docs`), because it has to fetch both whatever origin holds. `thread pause`, `thread start`, `thread binding` and `ledger push` run neither: on a store with a file remote and a record of both refs, the record stands in for the answer, and a push that finds origin moved is rejected and retried after a fetch (0.22.0, `docs/arch/2026-10-20-session-start-and-pause.md`).

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

`tsk ledger discard` drops every local change that is not pushed, in the data and the
documents, so a trial mission can be undone without `dolt reset`. It is destructive.

- Without `--yes` it changes nothing. It prints the `Local` block of `status` (the headline
  and detail lines, no `Origin` line) and exits 1 with
  `error: this would discard the local changes above. Rerun with --yes to discard them.`
  With `--porcelain` it prints the `data` and `docs` lines of the porcelain form instead
  (no `remote` lines). When both sides are clean it prints `Local   nothing to push` (or the
  two `clean` lines with `--porcelain`) and exits 0.
- With `--yes` it fetches both remote states first. If a fetch fails it exits 1 with
  `Nothing was discarded` and changes nothing. Otherwise it runs
  `dolt reset --hard origin/main` and `dolt clean` (which drops a table that was never
  committed), and in the documents worktree `git reset --hard <fetched tip>` and
  `git clean -fd`. When origin
  holds no such branch, the reset target is the local `HEAD`. It prints `discarded`.
- It does not touch thread state that lives outside the ledger: `tsk-thread-skip` in the
  git directory. A binding is a ledger row, so `discard` drops a binding that is not
  pushed, like any other local change.

Both are methods on the storage seam (`LedgerStore::status` and `LedgerStore::discard`).

### Manifest version

The `version` row of the `manifest` table is `14` for a ledger whose documents carry an id in
the file name and whose `mission_documents` has `doc_id`, `13` for a ledger whose `tasks`,
`decisions` and `questions` have a `uid` column, `12` for a ledger with `thread_bindings`,
`actors.uid`, `actor_names`, `session_actors` and the manifest key `ledger_id` (ADR 0030),
`11` for a ledger with the `sessions` table and the bindings still in `bindings`, `10` for a
ledger with `missions.code` and no `sessions` table, `9` for a ledger with the `actors` and
`instances` tables, `8` for a ledger whose bindings, of cloud sessions and of code
worktrees, are rows of `bindings` and that has no actor tables, `7` for a ledger whose constraints
and out of scope are tables of their own with the bindings still in `cloud_session_bindings`, `6` for a ledger whose `decisions` table has
`adr_path`, `5` for a ledger whose `threads` table has
`mission_id`, `4` for a ledger with the mission, task, decision and question tables and the
older `threads` table, `3` for a ledger with the mission tables only, and `2` for a
ledger whose documents are files on `tsk/docs` and that has no mission tables. The Dolt
backend accepts versions 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13 and 14. Version 1 is a ledger that still keeps its
documents in a `documents` table.

- A version 1 ledger is refused by every command except `tsk ledger fetch`,
  `tsk ledger path` and `tsk ledger migrate-docs`. The refusal reads: `error: this ledger
  stores its documents in Dolt (version 1). Run tsk ledger migrate-docs to move them to the
  tsk/docs branch.` `tsk ledger fetch` refreshes the database only and prints a note.
- A version 2 ledger works for everything it did before. The mission read commands behave
  as an empty mission set and write nothing. The first mission write command creates the
  mission, task, decision and question tables, adds `mission_id` to `threads` and makes
  `briefing_path` nullable, and sets the `version` row to `10` in the same local Dolt
  change as the mission, so `tsk ledger push` publishes both. The tables are created in
  their newest shape.
- A version 3 ledger works for everything it did before. The task, decision and question
  read commands behave as if the mission had none, and write nothing. The first write
  command of any of the four kinds (`mission`, `task`, `decision` or `question`) adds the
  columns `next_task` and `next_question` to `missions`, creates the four new tables, alters
  `threads` as above and sets the `version` row to `10` in the same local Dolt change as the
  record, so `tsk ledger push` publishes both. A write that fails a check upgrades nothing.
- A version 4 ledger works for everything it did before. `thread list` derives each
  thread's mission ID from its briefing path. The first write that needs the new column
  alters `threads` and sets the `version` row to `10` in the same local Dolt change:
  `tsk thread start`, `tsk thread set mission`, and a mission, task, decision or question write.
  Reads, `thread skip` and a `thread pause` upgrade nothing, and neither does a write that is refused.
  A version 2, 3, 4, 5 or 6 ledger takes the whole chain of changes up to version 11 at its first
  such write.
- A version 5 ledger works for everything it did before. `decision list` prints `-` as the
  ADR path of every decision, and `mission list --json` prints `adr` as null. The first
  write of any of the four kinds adds `decisions.adr_path` with
  `alter table decisions add column` and the later steps, and sets the `version` row to `10`
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
  5. sets the `version` row to `10`, after the bindings step and the actor step below.
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
  4. sets the `version` row to `10`.
  The same step ends the chain for a ledger at version 2 to 6, and for the first write of
  any other kind that upgrades it. A write that is refused upgrades nothing. `tsk ledger
  status` labels the upgrade and does not show the copied rows as changes.
- A version 8 ledger works for everything it did before. It has no actor: `tsk actor name`
  prints `no actor registered`, `tsk actor list` prints nothing, `thread list` prints
  `null` as the `name` of every actor, and a read upgrades nothing. The first write that
  reaches a binding or a registration (`tsk actor register`, `thread start`, `thread
  resume`, `thread detach`, `thread stop`, or the marker migration in
  [`tsk-thread-id`](#tsk-thread-id)) creates `actors` and `instances`, both empty, and sets
  the `version` row to `10` in the same local Dolt change as the write. The same step ends
  the chain for a ledger at version 2 to 7, after the bindings step. `tsk ledger status`
  labels the upgrade.
- A version 9 ledger works for everything it did before. A mission has no `code`: every
  command reads the `id` as the mission ID, and `mission list --json` prints it as `uid`.
  A read upgrades nothing. The first write that reaches a mission or a thread adds the column
  `missions.code` after `id`, sets it to the `id` of every mission, adds the unique index
  `missions_code_unique`, and sets the `version` row to `10`, in the same local Dolt change
  as the write. The step adds a column and an index and rewrites no key, so two clones that
  each upgrade write the same change and Dolt merges them. The same step ends the chain for
  a ledger at version 3 to 8. `tsk ledger status` labels the upgrade.
- A `tsk` binary from before version 11 does not know the version. It stops at its manifest
  check with `unsupported ledger version 11`, before it writes. The binary needs updating.
- **Automatic update (tsk 0.18.0 and later).** A command refused with `unsupported ledger
  version N`, where N is above the highest layout this tsk supports, prints `warning: this
  ledger is at layout N and this tsk (X) supports up to M. Updating tsk now.` to stderr,
  runs the update that `tsk self-update` runs, then replaces itself with the same command
  line run by the updated binary. The re-run keeps stdin and stdout, and its exit status
  is the command's. The update step finds the new binary by the path the command was
  invoked by, then by the canonical path of the running binary. The re-run sets
  `TSK_AUTO_UPDATED=1`. If the error still happens, the command stops with the original
  error and `tsk was updated but still does not support layout N; update it by hand: tsk
  self-update`. If the update fails, or the version does not rise, the command prints that
  error, then the original one, and exits 1.
  - `tsk thread session-start` and `tsk thread guard` never update. They have timeouts,
    and a Cargo update compiles for about a minute. They behave as before.
  - `TSK_AUTO_UPDATE=never` or `TSK_NO_UPDATE_CHECK=1` keeps today's behaviour for every
    command. `TSK_AUTO_UPDATE=always` is the default.
  - Only the unsupported-layout error starts an update. A network error or a merge
    conflict never does.
  - A tsk that is newer than the ledger reads the old layout and upgrades it on its first
    write. Nothing upgrades on a read, and nothing here runs for that direction.
  - A binary released before 0.18.0 does not update itself. It stops with the error, and
    the person runs `tsk self-update`.
  - Tests set `TSK_TEST_MODE=1` and `TSK_SELF_UPDATE_CMD=<script>`. The update step then
    runs the script, with the installed tsk path as `$1`, instead of the real update.

- A version 11 ledger (and every lower one, after the steps above) works for everything it
  did before: the bindings are read from `bindings`, an actor has no `uid`, and `tsk ledger
  id` exits 1. The first write of any kind that upgrades it (a record, a thread, a
  registration, a bind) runs the version 12 step in the same local Dolt change: `actors`
  gains `uid`, filled from the MD5 of each name; `thread_bindings` is created, every
  `bindings` row is copied into it under the column `instance`, and `bindings` is dropped;
  `actor_names` gets one open row per actor; `session_actors` gets a primary row for each
  recorded session whose instance had registered an actor; and the manifest gets
  `ledger_id`, a UUID from the MD5 of the root commit hash. Every value is a function of
  what the ledger holds, so two clones that upgrade on their own write the same change and
  Dolt merges them. `tsk ledger status` labels the upgrade and does not show the copied
  rows as changes. A session start never upgrades: on a ledger below 12 it records the
  session and no actors.

- A version 12 ledger works for everything it did before: a task, a decision and a question
  have no uid, the JSON output has no `urn` for them, and a read upgrades nothing. The first
  write of any kind that upgrades it adds the column `uid` varchar(36) to `tasks`,
  `decisions` and `questions`, and fills it from the MD5 of the kind, the mission ID and the
  number, in the same local Dolt change as the write. Every value is a function of what the
  ledger holds, so two clones that upgrade on their own write the same change and Dolt merges
  them. A row written later gets a uid from `uuid()` at its first insert, and an update of the
  row does not name the column, so a renumbering (ADR 0022) keeps it. The column has no unique
  key.

- A version 13 ledger works for everything it did before: its documents have no id in the file
  name, and `mission list --json` prints no `id` or `urn` for a linked document. A read
  upgrades nothing, and shows each link as the upgrade will leave it. The upgrade has two
  parts, which keep two clones consistent because each is a function of the path alone:
  1. **The Dolt change.** `mission_documents` gains `doc_id` and the unique key
     `mission_documents_doc_unique`. Each row gets the id of its path, the first 8 hexadecimal
     characters of the MD5 of `urn:tsk:document-path:<path>`, and its `path` gains the id
     before the last extension. A path that already carries an id keeps it. A thread whose
     `briefing_path` is a document moves the same way. The rows are written with the schema
     change, in the first Dolt write that upgrades the ledger, so two clones that upgrade on
     their own write the same change and Dolt merges them.
  2. **The files.** The renames are a change of the documents branch, which Dolt cannot hold.
     A read of a document (`doc get|list|path`, `mission link`) renames nothing, so it never
     leaves a dirty worktree that `tsk ledger fetch` would refuse: a file with no id keeps
     its name, a reference to it by path, by the path with the id it will get, or by that id
     still resolves, and `doc list` prints `-` for its id. A write of a document (`doc put`,
     `mv`, `rm`) first upgrades a ledger still below 14 with the Dolt change, then renames
     every file that has no id to the name with the id of its path. `tsk ledger push` on a
     ledger at 14 does the same renames before it commits the documents. A push of a ledger
     below 14 upgrades nothing. The step is idempotent: a file that has an id is left alone,
     so a clone whose branch is already renamed, or whose rename was interrupted, does
     nothing or finishes. The two seed files of a new branch (`README.md` and `index.md` with
     their seed text) are not renamed, and a write of one gives it the id of its path. Two
     clones that rename on their own make the same renames, so the second push rebases the
     same change onto the first. `tsk ledger push` sends the documents first, so a link never
     names a file the remote lacks.

### Migration from the file ledger

`tsk ledger migrate-to-dolt` was removed with the files backend. To move an old file
ledger to Dolt, use `tsk` 0.15.1, the last release that has the command, then update. The
command pushed the file ledger's commit to `refs/heads/archive/tsk-ledger-files` (or
`refs/heads/archive/ledgers/<repo-id>-files` for a nexus ledger) before Dolt's first push
replaced the branch. A `tsk` that finds a file ledger on the ledger ref stops with an
error that says so.

### Moving the transport

`tsk ledger migrate-transport` converts a ledger whose ref holds the `git-remote` layout.
It runs in one clone, once.

1. It refreshes the store. A store whose origin already holds a `file-remote` ref moves to
   it and the command prints `migrated-transport:already`.
2. It stops with exit 1 for a version 1 ledger (run `tsk ledger migrate-docs` first) and
   for a store that holds commits origin lacks (run `tsk ledger push` first).
3. It resets `ledger-remote` to the ref's tip, removes the old tree, and pushes the
   store's history into it through a temporary Dolt remote. It adds `.tsk-transport` and
   `.gitignore` and commits.
4. It pushes that commit to the ledger ref with a plain `git push`. The commit's parent is
   the old tip, so the old content is replaced by a normal commit. A rejected push means
   origin moved: nothing changed, run the command again.
5. It points the store's `origin` at the directory, fetches, and checks that `origin/main`
   equals the store's `HEAD`.

It prints `migrated-transport:file-remote` and the commit the ref now holds. Notes go to
stderr. Update every `tsk` first: a binary older than 0.21.0 cannot read the result.
`tsk ledger migrate-docs` works on a store that uses `git-remote` only.

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
tsk mission list [<id>] [--json] [--porcelain]
```

Rules:

- A mission ID matches `^M-[A-Z0-9]+(-[A-Z0-9]+)*$` and is at most 64 characters. A name is
  one line of at most 255 characters.
- A ledger can have a mission prefix (ADR 0027): 3 or 4 capital letters, the manifest key
  `mission_prefix`. `tsk mission prefix` prints it, or sets it with an argument, and exits
  1 with no argument when the ledger has none. With a prefix, `create` and `mv` add it after
  `M-` to an ID that lacks it, and print the resulting ID on stdout when they added it. An
  ID that starts with `M-<prefix>-`, or is `M-<prefix>`, is not changed. A trailing group of
  digits marks a part of a mission: `M-TSK-BOOT-01` has to be a part of the mission
  `M-TSK-BOOT`, which has to exist. `create` makes it the parent when none is given, and
  refuses a different parent. `mv` refuses a number whose mission is not the parent the
  mission has, or is the mission being renamed. With no prefix none of this applies, and
  `create` prints a note on stderr when the ledger already has missions. Setting the prefix
  renames no mission, and two clones that set the same value merge.
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

`list` with no ID prints one line per mission, ordered by ID: `id`, `status`,
`blocked-by` (the IDs of the blockers that are not done, comma-separated, or `-`), `parent`
(or `-`) and `name`. The columns are aligned: each is padded with spaces to the widest cell
in it, over all lines, two spaces separate columns, and the last column (the name) is not
padded. The output has no tabs and no trailing whitespace, like `tsk ledger status`.
`--porcelain` prints the same lines tab-separated, unpadded, for scripts. With an ID it
prints that mission's line. `list <id> --json` prints the
full record as one JSON object, and `list --json` prints an array of them:

```
{"id":"M-J","name":"Json","category":"operational","status":"todo","parent":"M-P","blocked":true,"purpose":"Why","decision_authority":"","constraints":[{"n":1,"text":"No new crates"}],"out_of_scope":[{"n":1,"text":"Projections"}],"objective":[{"n":1,"outcome":"One","met":true},{"n":2,"outcome":"Two","met":false}],"blocked_by":[{"id":"M-Q","blocking":true}],"documents":[{"path":"m/design.md","role":"design"}],"tasks":[],"decisions":[],"questions":[]}
```

`blocked_by` lists every blocker. `blocking` is true for each one that is not done, and
`blocked` is true when any is. `objective list <id>` prints `n`, `met` or `open`, and the
outcome, tab-separated. There is no `show` command and no Markdown output.

A mission is a row of `missions` and rows of the tables that hang from it, so Dolt merges two missions, or two fields of one mission, without a conflict.

### Rename and move

`tsk mission mv <old> <new> [--dry-run]` changes `missions.code` and nothing else, because every other row names the mission by `missions.id`. Concurrent writes to the mission on another clone merge with it, whatever they are. It also renames each linked document whose path names the mission, and updates its link. A path names the mission when a component is the old ID, or starts with it and a `-` or a `.`: `M-X.md`, `M-X-slug.md` and a directory `M-X`. A component that starts with the ID of a mission with a longer ID belongs to that one. The command refuses an unknown mission, an ID that is not a mission ID, an ID that a mission has (compared without regard to case), the ID the mission has, and a new document path that exists. It prints `mission <old> -> <new>`, then `doc <old> -> <new>` for each document, then `mentions <mission> <record>` for each record whose text names the old ID as a whole word. It does not change that text. `--dry-run` prints the same lines and a note on stderr, and changes nothing.
`tsk mission rewrite <old> <new> [--dry-run]` replaces the ID `<old>` with `<new>` where it stands as a whole word (not preceded or followed by a letter, digit or `-`) in the text of every mission: `name`, `purpose`, `decision_authority`, objective outcomes, constraints, out-of-scope items, task descriptions, decision texts, question texts and answers. It changes no key and no document. `<new>` has to be a mission. `<old>` can be the ID of a mission renamed earlier, which is the use: a rename leaves the old ID in free text, and `rewrite` brings it up to date. It prints `rewrote <mission> <record>` for each text it changed. `tsk mission mv --rewrite` runs the same rewrite after the rename and prints its lines in place of the `mentions` lines.

The new ID has to be a mission ID by the rule of `tsk mission create`.

`tsk task mv <mission> <task> <to>`, `tsk decision mv <mission> <n> <to> [--task <task>]` and `tsk question mv <mission> <question> <to>` move a record to another mission, which gives it the next number there. The number it had is not used again. The two missions are saved in one local change.

- A task keeps its description and status. The decisions that name it move with it and name it under its new number. A task with blocks, in either direction, is refused and the refusal names them, because a block is a row of one mission. `--force` drops them and prints one `dropped block` line for each.
- A decision is addressed by its place in `tsk decision list <mission>`, from 1, because the list prints no number. It keeps its text, author, date and ADR. Its task link is cleared and the output says so, unless `--task` names a task of the target.
- A question keeps its text, status, answer and dates.

Each command prints one line per change, such as `task M-A T-05 -> M-B T-12` and `decision M-A 1 -> M-B 4`. A mission save removes a task, decision or question that the mission no longer holds. It writes decisions before it removes a task, because a decision points at a task.

`tsk ledger doc mv <from> <to>` renames a document in the local store and updates every mission link to the old path, keeping its role. It prints `doc <from> -> <to>` and `link <mission> <role>` for each mission, and refuses a document that does not exist and a path that does.

`constraints` and `out_of_scope` are rows of `mission_constraints` and `mission_out_of_scope`, numbered from 1, with the counters `next_constraint` and `next_out_of_scope` on the mission row.

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
tsk task list <mission> [--porcelain]
tsk decision add <mission> <text> --by <who> [--task <T-nn>] [--on <YYYY-MM-DD>] [--adr <path>]
tsk decision list <mission> [--adr <path>] [--porcelain]
tsk decision list [--adr <path>] [--porcelain]   # no mission: every mission, led by the mission ID
tsk question add <mission> <text> [--on <YYYY-MM-DD>]
tsk question edit <mission> <question> <text>
tsk question answer <mission> <question> <text> --by <who> [--on <YYYY-MM-DD>]
tsk question list [<mission>] [--porcelain]
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
  with neither a mission nor `--adr` lists every decision of every mission, as `question
  list` does with no mission.
- `question edit` rewrites the text of an open question and nothing else: the ID and the
  date asked stay. An answered question cannot be edited:
  `error: Q-nn of M-X is answered; it cannot be edited`.
- `question add` records the UTC date of the command as `asked_on`, or the date given to
  `--on`. `question answer` records the answer, the name after `--by` and the UTC date, or
  the date given to `--on`, as `answered_on`, and the question is no longer open. `--on`
  is checked as it is for `decision add`: `YYYY-MM-DD`, a real calendar date, else an
  error and nothing is written. No check compares `answered_on` with `asked_on`. A question is answered once: a second answer is an error.
- An unknown mission, task or question is an error that names the ID.

Each `list` prints one record per line, with no header. The columns are aligned as in
`mission list`: padded with spaces to the widest cell of the column over all lines, two
spaces apart, no tabs, no trailing whitespace, and the last column unpadded. (In
`decision list` the text is padded, and the ADR path is the last column.) `--porcelain`
prints the same columns tab-separated and unpadded, the stable form for scripts. The JSON
form of `mission list` is not changed by either. The columns:

| Command | Columns |
|---|---|
| `task list <mission>` | `T-nn`, `status`, `blocked-by` (the IDs of the blockers that are not done, comma-separated, or `-`), `description`. Ordered by number. |
| `decision list <mission>` | `decided_on` (`YYYY-MM-DD`), `decided_by`, `T-nn` or `-`, `text`, `adr` (the ADR path, or `-`). Oldest first. With `--adr`, only the decisions that name that ADR, in the same five columns. |
| `decision list` with no mission | `mission`, then the five columns above. Every decision of every mission, ordered by mission and then oldest first. With `--adr <path>`, only the decisions that name that ADR. |
| `question list [<mission>]` | `mission`, `Q-nn`, `asked_on` (`YYYY-MM-DD`), `text`. Open questions only, ordered by mission and number. All missions when none is given. |

`tsk mission list --json` carries the records. Each mission object has three more keys after
`documents`: `tasks` (`id`, `description`, `status`, `blocked`, and `blocked_by` as a list
of `id` and `blocking`), `decisions` (`text`, `decided_by`, `decided_on`, `task` or null, and `adr` or null)
and `questions` (`id`, `text`, `status`, `asked_on`, and `answer`, `answered_by` and
`answered_on`, null while open). There is no `show` command and no Markdown output.

Two decisions added at the same time by two clones get the same `seq`, as do two tasks
added to one mission. The second push renumbers its rows and prints each change as, for
example, `T-03 is now T-05 in M-X` (see [Numbers and collisions](#numbers-and-collisions)).

## File ledger (removed)

Up to and including 0.15.1, `tsk` could keep the whole ledger as files on one git branch,
selected as the `git` backend. The branch held a `.tsk-ledger.toml` manifest (`version =
1`), `index.md`, `missions/` and `threads/` (a directory for each thread, with an
`index.md` and a `continuation-state.jsonl`, and the cloud and worktree bindings as JSON
files), `external-events/` and, from layout version 1 on, `records/missions/<ID>.json` for
the mission records. It was checked out as a detached linked worktree at
`.../repos/<clone-id>/ledger` and published with a git commit and a compare-and-swap push.

0.16.0 removed it (ADR 0015). Release 0.15.1 is the last that has it, and every release up
to it is tagged. The full history of this repo's file ledger is on the branch
`archive/tsk-ledger-files`. To read an old file ledger or to move it to Dolt, build `tsk`
at the tag `v0.15.1` and run `tsk ledger migrate-to-dolt`. This document no longer
describes the file layout. Read the 0.15.1 copy of this file for it.

## Location and ref

A ledger is a git ref. It is never checked out in the managed repo's main working copy.
Two modes decide which refs hold it (ADR 0016):

- `custom` is the default for a new ledger. The refs are outside `refs/heads/*`, so GitHub
  lists no branch and offers no "Compare & pull request" banner for them.
- `branches` keeps the refs under `refs/heads/*`. A repo used from Claude Code cloud
  sessions needs it, because the cloud proxy refuses to push or update a ref outside
  `refs/heads/*` (ADR 0008).

| Location | Remote | Ref in `custom` | Ref in `branches` |
|---|---|---|---|
| In-repo (default) | the managed repo's `origin` | `refs/tsk/ledger` | `refs/heads/tsk/ledger` |
| Nexus (option) | the nexus repo | `refs/tsk/ledgers/<repo-id>` | `refs/heads/ledgers/<repo-id>` |

The documents ref is `refs/tsk/docs` or `refs/heads/tsk/docs` in-repo, and
`refs/tsk/ledgers/<repo-id>-docs` or `refs/heads/ledgers/<repo-id>-docs` on a nexus.

The binary passes the full ref name to every git command, and `dolt remote add --ref` takes
the full name too. It never passes `tsk/ledger` or any other unqualified name. On the tsk
repo's `origin`, an orphaned custom ref `refs/tsk/bootstrap` shadows the unqualified name
`tsk/bootstrap`, and an unqualified fetch returns that ref's content with exit status 0
(ADR 0008, amended 2026-09-16). The full name makes the same trap impossible for
`tsk/ledger`. A conformance case runs a logging `git` and fails when a short name reaches
it. Git fetches a custom ref into `FETCH_HEAD`, as it does a branch, and creates no
remote-tracking ref for either mode. Dolt 2.5.0 accepts a ref outside `refs/heads/*` for
its remote: push, clone, fetch and merge work, and it keeps its copy of the remote under
`refs/dolt/remotes/origin/tsk/ledger/last`.

### Which mode a repo uses

The mode is read from these sources. The first that has an answer wins:

1. `TSK_LEDGER_REFS`, the environment of one process. It holds `branches` or `custom`.
2. The repo's entry in the nexus: the field `ledger_refs` (see
   [The nexus entry](#the-nexus-entry)).
3. The repo config, `.tsk/config.toml` at the top of the code repo, committed with the repo:

   ```toml
   [ledger]
   refs = "branches"
   ```

4. Discovery, for a ledger that no person or file configured:
   1. The ledger store on this machine, when it exists. The ref its `origin` remote
      records decides the mode. This is a file read.
   2. The answer an earlier discovery cached in `<state>/repos/<clone-id>/ledger-refs`.
   3. One `git ls-remote <remote> refs/heads/tsk/ledger refs/tsk/ledger` (the nexus ref
      names for a nexus ledger). The branch ledger wins when both exist. The answer is
      cached. When origin cannot be reached, nothing is cached and the next rule applies.
   4. A new ledger takes `custom`. In a Claude Code cloud session
      (`CLAUDE_CODE_REMOTE=true`) it takes `branches` instead, because the first push of a
      custom ref would fail. The command that creates the ledger, and the session start
      context, say so:

      > This is a Claude Code cloud session, which cannot push a custom ref, so the ledger
      > uses branches. To choose, run tsk config ledger-refs custom or branches before the
      > first push.

A nexus entry or a repo config that says nothing leaves the choice to the next source. The
nexus is part of the environment and holds the setting for a repo that keeps nothing of
tsk's. The repo config serves a standalone repo with no nexus entry, such as one used from
cloud sessions.

`tsk config ledger-refs` with no argument prints the mode and where it came from:

```
ledger refs: custom
source: default
ledger ref: refs/tsk/ledger
docs ref: refs/tsk/docs
```

`source` is `environment`, `nexus`, `repo config (<path>)`, `ledger store`,
`discovered on origin`, `default` or `default (Claude Code cloud session)`.
`tsk config ledger-refs branches|custom` writes `[ledger] refs` to `.tsk/config.toml` at
the top of the code worktree. It creates the directory, keeps every other key, and prints
the new mode, the two refs and `wrote <path>`. It forgets the cached discovery. It prints a
note to stderr when the environment or the nexus entry outranks the file. When a ledger
store exists that was never published, the command points its Dolt remote at the new refs,
so a person who chose the wrong mode before the first push can correct it. `tsk nexus
register-repo --ledger-refs branches|custom` writes `ledger_refs` on the new entry.

tsk does not move a ledger from one mode to the other. A mode that disagrees with a ledger
that exists stops the command with an error that names both refs and the place to change
the setting:

```
error: this repo is set to custom ledger refs (repo config), which use refs/tsk/ledger, but origin holds the ledger on refs/heads/tsk/ledger.
       tsk does not move a ledger between branches and a custom ref, and it does not start a second ledger.
       To follow origin, run: tsk config ledger-refs branches
```

The same error comes with "the ledger store on this machine" in place of `origin` when the
store is on the other refs. Where the setting comes from the nexus, the last line reads
`set "ledger_refs": "branches" on the repo's entry in nexus.json`, and from the
environment, `set TSK_LEDGER_REFS to branches, or unset it`. The check at fetch time asks
origin for the other refs only when the ledger ref is absent, so a ledger that exists
costs no extra call. `tsk config ledger-refs <mode>` refuses to switch a ledger that
origin holds.

A host that refuses a custom ref (the Claude Code cloud proxy, or a server hook) stops
`tsk ledger push` with:

```
error: origin refused the push to refs/tsk/docs. The host may accept only branches, as the Claude Code cloud proxy does.
       The local commits stay. To use branches for this ledger, run: tsk config ledger-refs branches
```

When git or Dolt reported the refusal, a line that starts `The server said:` and quotes
it follows the first line. The documents push comes first, so the documents ref is the one
named when both are refused. Nothing is lost: the records and documents stay in the
ledger store, and after the fix the next `tsk ledger push` sends them.

The nexus location is for a managed repo the operator cannot push a ledger branch to,
such as a work repo. The ledger objects live in the nexus repo. The managed repo's clone
fetches and pushes them against the nexus URL as the remote. The documents worktree stays
a linked worktree of the managed repo's clone.

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
   in-repo. `"ledger": "nexus"` means the ref `ledgers/<repo-id>` in the nexus (under
   `refs/heads/` or `refs/tsk/`, by the mode), where `<repo-id>` is the entry's `id`. Any
   other `ledger` value stops with an error.
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
| `ledger_refs` | no | `"branches"` or `"custom"`: which refs hold the ledger (see [Which mode a repo uses](#which-mode-a-repo-uses)). Absent leaves the choice to the repo config and discovery. Any other value stops with an error. |
| `prefix` | no | The mission prefix of the repo's ledger (3 to 4 capital letters), written at registration. Two entries in one nexus cannot have the same prefix. Absent for an entry registered before 0.36.0; registering again records it. |

`tsk nexus register-repo`, run in the managed repo, adds its entry. It fetches the
nexus's default branch, adds the entry to `nexus.json`, and commits and pushes it with the
machine's own git credentials. On a rejected push it fetches again and retries once.
The ledger of the repo needs a mission prefix first (`tsk mission prefix <PREFIX>`); without
one the command stops. It refuses a prefix that another entry has, and writes the prefix to
the new entry. When the repo's entry exists without a prefix, registering again records it.

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
- `ledger` is `--ledger`, `nexus` by default, and is always written. `ledger_refs` is
  `--ledger-refs`, written only when the option is given.
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

The Dolt remote of a nexus ledger is `git+<nexus-url>` with the ref
`refs/tsk/ledgers/<repo-id>` (`refs/heads/ledgers/<repo-id>` in `branches` mode). The
documents ref is `refs/tsk/ledgers/<repo-id>-docs` (`refs/heads/ledgers/<repo-id>-docs`) on
the same remote. The managed repo's clone fetches the documents ref into its own object
database, which `git worktree add` needs. The nexus repo is not an `origin` or any named
remote of the managed clone, and no `.git/config` entry is written. A manifest `repo_id`
that differs from the entry's `id` stops `tsk ledger fetch` and `tsk ledger push` before
anything is written.

### A ledger that does not exist yet

When the ledger ref is absent on its remote, `tsk ledger fetch` creates a new local ledger.
It detects absence with `git ls-remote --exit-code`, which exits 2 for a missing ref. Any
other failure is an error.

1. tsk runs `dolt init`, creates the schema, writes the manifest rows (`version`, and
   `repo_id` for a nexus ledger), commits, and adds the remote with the ledger ref.
2. If the documents ref is absent, it makes the orphan documents commit with git
   plumbing and creates the documents worktree on it, detached.
3. tsk prints a note to stderr that the ref does not exist yet, and the path of the
   Dolt database to stdout. A ledger that took `branches` because it was made in a Claude
   Code cloud session adds the sentence from
   [Which mode a repo uses](#which-mode-a-repo-uses).

Before step 1, tsk asks origin for the ledger ref of the other mode. When origin holds the
ledger there, the command stops with the mismatch error and makes nothing.

The refs are created by the first `tsk ledger push`. A later `tsk ledger fetch` while
the ref is still absent leaves the store as it is and prints the note again.

When another clone then publishes the ledger, its root is not the root of this clone's
store. A store that holds only its creation commits has no data to lose, so `tsk ledger
fetch` replaces it with origin's ledger, with no prompt, and prints one line on stderr. The
two sides are checked on their own:

- The Dolt database is replaced when every unpushed commit is `Initialize data repository`
  or `Create the ledger`, its working set is clean, and no table other than `manifest` has a
  row. The line is `note: replaced an empty local ledger (creation commits only, no rows)
  with origin's ledger`.
- The documents worktree is replaced when its only unpushed commit is `Create the documents
  branch` and its working tree is clean. The line is `note: replaced an empty local
  documents branch (creation commit only, no files) with origin's documents`. It is printed
  only when the database was not replaced.

Any other local commit, any row and any uncommitted change keeps the earlier behaviour: the
note that names the commits, and no reset.

## `missions/`

The files missions leave behind, as Markdown: intelligence, research, reports and designs,
linked from the mission record by path and role. Briefings from the file era are also
here. A mission's purpose, tasks, decisions and questions are records and not documents,
so a new mission has no briefing. The binary moves these files between the documents branch and the
documents worktree without parsing them.

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

## Threads

The thread store from `docs/domain/session-continuation-design.md`. A thread is a row of
`threads` (its ID, its mission and, for a thread made before layout version 5, a briefing
path). Its continuation entries are rows of `continuation_entries`, one compact JSON
object for each, numbered from 1 by `seq`. Its bindings are rows of `thread_bindings`. See
[Tables](#tables).

`thread list` takes the mission ID from the thread row, and derives it from the briefing
path when the row stores none. It takes the mission link from the briefing path.

### Continuation entries

Append-only. Each entry is one compact JSON object (no whitespace between tokens), written
by `thread pause`. Entries are never rewritten or removed, except that `thread stop`
deletes them with the thread.

Fields, in the order written. `git` holds `ledger` then `code`; `ledger` holds `commit`;
`code` holds `ref` then `commit`:

| Field | Type | Source | Meaning |
|---|---|---|---|
| `mission_id` | string or null | command | The mission the thread works, from the thread. `null` when the thread has no mission. Entries written before version 5 do not have it. An entry keeps the mission it was written with: `thread set mission` does not rewrite earlier entries. |
| `mission_link` | string | command | The mission briefing the thread works on, as a path relative to the ledger root. Written only for a thread whose mission has no record. Earlier entries always have it. |
| `task_id` | string | caller | The task in progress at pause time, for example `T-03`. Can be empty. When the mission has a record, the command checks it against the mission's tasks and writes `T-5` as `T-05`. A mission with no record has no tasks to check against. A thread with no mission has no tasks, so `task_id` is empty: the command refuses another value with `error: thread <thread-id> has no mission, so it has no tasks. Attach a mission with tsk thread set mission <mission-id>, or pass an empty task ID`. |
| `whats_next` | string | caller | A short account of where things stand. |
| `git` | object | command | The commits the entry records, in two nested objects: `ledger` and `code`. |
| `git.ledger.commit` | string | command | The Dolt commit hash of the ledger store's `HEAD`, after it is refreshed from the remote and before this entry is written. |
| `git.code.ref` | string | command | Full ref of the branch the code worktree's `HEAD` is on at pause time, for example `refs/heads/feature/x`. |
| `git.code.commit` | string | command | Full SHA of the code worktree's `HEAD` at pause time. |
| `timestamp` | string | command | UTC time the entry is appended. |
| `written_by` | string | command | Who wrote the entry (ADR 0030): the session URN, `urn:tsk:session:<harness>:<id>`, when the session is known, which is every session that ran the session start; else the instance key, the session ID in a cloud session or `<clone-id>/<worktree-name>` as `thread_bindings` holds it. Entries written before 0.39.0 hold the instance key; entries written by 0.16.0 or earlier hold `urn:tsk:worktree:.git` or `urn:tsk:cloudsession:<id>`, and 0.38.0 writes `urn:tsk:instance:<kind>:<key>` as a fallback. A reader maps a session URN to its instance and its primary actor through `sessions` and `session_actors`, an instance key to the actor of its latest session, and the three older URN forms to an unknown writer. |

`git.ledger.commit` cannot be the commit the push creates: that commit holds this entry,
so its hash is unknown when the entry is written.

The flat fields `commit_on_bootstrap`, `commit_on_ledger` and `commit_on_main` of earlier
entries are not read or written by the binary, and there is no fallback for them. A
reader that passes an entry through unchanged (for example `thread resume`, which prints
the latest entry) prints it as stored.

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

The latest entry is the one with the highest `seq`. A thread with no entries has no
latest entry yet; `thread resume` prints `{}` for it. `thread list` sorts threads by the
latest entry's `timestamp`, newest first, with threads that have no entry last. The
`written_by` values across all entries are the set of actors that have touched the thread.
`thread resume` and the session start warn when no writer of the thread is the current
actor: a writer is the current actor when both actors are known and equal, or when either
is unknown and both instances are known and equal. The warning names each other writer by
its actor's name, else by its instance key. A writer with no known instance (an old `urn:`
form) is left out, so a thread paused before the instance key was written gives no warning
until a pause writes one.

### Bindings

A binding is a row of `thread_bindings`: `instance`, `kind` (`cloud` or `worktree`),
`thread_id` and `registered_at` (ADR 0030; `bindings` with the column `actor` before
version 12). The key of a cloud session is the `CLAUDE_CODE_REMOTE_SESSION_ID` value
(`cse_...`). The key of a code worktree is `<clone-id>/<worktree-name>`. A bind sets or
replaces the instance's row. Detach deletes that one row. Stop deletes every row whose
`thread_id` is the stopped thread, of either kind. `thread list` shows the rows as
`actors`, each with the `name` of the primary actor of the instance's latest session, else
the actor the instance registered as, or `null`.

An actor is a human or a robot, with a `uid`, a name and a kind, and its identity persists
outside any harness instance or session: a row of `actors`, with its names in
`actor_names`. An actor belongs to a session, never to a harness instance (ADR 0030): a
session has one primary actor and any number of supporting actors, as rows of
`session_actors`. `tsk actor register <name> --kind human|robot` checks the name (1 to 200
characters, not starting with `/`), adds the `actors` row with a new uid when the name is
new, and makes the actor the primary of the current session. A name already registered must
have the same kind: the command then prints `<name> (<kind>) is now this session's actor`
or `... this worktree's actor`; with the other kind it exits 1 with `<name> is registered
as a <kind>` and changes nothing. A new name prints `registered <name> (<kind>)`. A second
line prints the actor's URN, `urn:tsk:actor:<uid>`. The command needs no thread binding and
does not fetch. The write is local; the next push publishes it. A rebind, a detach and a
stop leave the actors as they are. Nothing deletes an `actors` row. `tsk actor join <uid>
[--primary] [--session <urn>]` adds an actor to the current session, or to the named one,
as supporting, or as primary with the earlier primary demoted; it refuses an unknown uid.
`tsk session actor [--session <urn>]` prints the actors of a session, one per line: role,
name, kind, uid, the primary first, or `no actor registered`. `tsk actor name` prints the
name of the primary actor of this session, else of the latest session of this instance that
has one, else the name the instance registered as, or `no actor registered`, and exits 0
either way. `tsk actor list` prints one actor per line: name, kind, uid, and the keys of
the instances that ran as it, comma-separated, or `-`, in the order `registered_at` then
key; `--porcelain` prints the columns tab-separated and `--json` prints the records with
the `urn`, the `names` history and their `instances`. `tsk thread binding` prints
`actor: <name> (<kind>)` on a second line when the session has a primary actor.

A new session takes its actors from the session it continues (ADR 0030, decision 6). The
session start, after it records the session, copies the `session_actors` rows of the
session that wrote the latest pause on the bound thread from the same instance; else of the
latest recorded session of the instance that has actors; else it makes the actor the
instance registered as its primary. The context then prints `Bindings:` with the session
URN, the instance URN and one item per actor. The session URN of a terminal session is kept
in the local file `tsk-session` of the git directory, written by the session start, so the
later commands of the session know it; a cloud session reads it from the environment.

A cloud session rarely detaches, so its row stays after it ends. `tsk thread prune-bindings
[--older-than <days>] [--dry-run]` deletes the cloud rows whose `registered_at` is more
than `<days>` days (30 by default) before the command runs, except the row of the session
that runs it. It never deletes a `worktree` row, and it keeps a row whose `registered_at`
is not a timestamp. It prints `removed <actor> <thread-id>` for each row, in the order
`registered_at` then `actor`, then `<n> bindings removed`. With `--dry-run` it prints
`would remove <actor> <thread-id>` lines and `<n> bindings would be removed`, and changes
nothing. The delete is one local write, which `tsk ledger push` publishes. The command
does not fetch.

## `external-events/`

The external event queue (M-BOOT-02, T-19) is the `external_events` table: a 1-based
`position` and the `envelope` text of each event. The watermark is the one row of the
`watermark` table.

### Event envelope

One compact JSON object for each event:

| Field | Type | Meaning |
|---|---|---|
| `source` | string | The event source, for example `github`. |
| `event_type` | string | The kind of event, for example `dependabot_alert`, `code_scanning_alert`, `secret_scanning_alert`. |
| `action` | string | What happened: the webhook action (`created`, `fixed`, ...), or `polled` for an event found by polling. |
| `repo` | string | The repository the event concerns, `owner/repo`. |
| `received_at` | string | UTC time the envelope was appended. |
| `payload` | any JSON value | The source's own payload, unchanged. |

- No rows: an empty queue.
- Producers append and push in one fetch, append, push cycle. A poll that finds several
  alerts appends all of them, with one shared `received_at`, and pushes once. A poll
  that finds none pushes nothing.

### Watermark

How far a consumer has processed the queue: `processed_through`, a non-negative integer,
and `updated_at`, the UTC time the watermark was last advanced. Positions `1` to
`processed_through` are done.

- No row: `processed_through` is `0`.
- Reading new events returns positions `processed_through + 1` to the end, oldest first,
  and does not write the watermark.
- Advancing takes a count. A count below the current value is refused. A count equal to
  it is a no-op and pushes nothing. A higher count writes the row and pushes. A
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
nothing. The `tsk/bootstrap` scripts and the binary share this file, so the stores of
the clone sit under the same `<clone-id>` directory.

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

No longer written. Before version 8 of the Dolt layout, this file was the binding of a code worktree:
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
| Commit | A git commit is the full 40-character lowercase hexadecimal SHA-1. A Dolt commit is a 32-character hash. |
| Thread ID | 8 characters, `[0-9a-z]`. Minted as 8 lowercase hexadecimal characters; IDs minted before hex minting are base36 (for example `4onylfsg`) and stay valid. Unique among the threads of the ledger: a new ID that matches an existing thread is discarded and minted again, up to 100 attempts. |
| Actor URN | `urn:tsk:instance:<kind>:<key>`, the URN of a harness instance (ADR 0029): `<kind>` is `cloud` or `worktree`, and `<key>` is the instance key, the cloud session ID or `<clone-id>/<worktree-name>`. A pause entry's `written_by` holds the bare instance key. The URN is its fallback for a code worktree of a clone with no clone ID, where `<key>` is the basename of `git rev-parse --git-dir` (`.git` in a main code worktree). Entries and commit messages written before ADR 0029 hold `urn:tsk:cloudsession:<id>` and `urn:tsk:worktree:<name>`, and stay valid. |
| Text encoding | UTF-8. JSON strings use standard JSON escaping. |

## Fetching and printing the path

`tsk ledger fetch` replaces `fetch-bootstrap-ref.sh`. `tsk ledger path` replaces
`bootstrap-wt-path.sh`. Both run in the `tsk` client alone, from inside
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

Prints the path of the Dolt database and a newline, and exits 0. It runs no fetch, writes
no file, and does not check whether the database exists. If `.git/tsk-clone-id` is
absent or empty it does not mint one: it exits non-zero and names `tsk ledger fetch`,
which mints it.

### `tsk ledger fetch`

1. Resolve the ledger location (see [Choosing the location](#choosing-the-location)).
2. Read `.git/tsk-clone-id`, minting it if absent or empty, and compute the paths of the
   database and of the documents worktree.
3. Ask the remote whether the ledger ref exists, by its full name. If it is absent,
   create a new local ledger instead (see
   [A ledger that does not exist yet](#a-ledger-that-does-not-exist-yet)). If the ref
   holds a file ledger, stop with the error that says the files backend was removed.
4. Clone the database from the remote when it does not exist locally: from the
   `ledger-remote` directory, reset to the ref's commit, when the ref holds `file-remote`
   (see [Transport](#transport)). Otherwise fetch it
   and merge a newer remote tip, unless local changes are not committed, in which case
   stop with exit 1 and leave the store as it is. Check the manifest version and, for a
   nexus ledger, its `repo_id`. A version this `tsk` does not know or a mismatched
   `repo_id` stops here.
5. Refresh the documents worktree the same way (see [Documents](#documents)): create it
   when absent, refuse uncommitted changes, note a local commit the remote lacks and leave
   it, otherwise reset to the fetched tip.
6. Print the path of the Dolt database and a newline to stdout.

Diagnostics go to stderr. Stdout carries the path only, so `WT="$(tsk ledger fetch)"`
works.

## Writing to the ledger

Every change to the ledger is a write to the Dolt database or to the documents worktree,
followed by a push. `tsk ledger push "<message>"` commits both and publishes them (see
[Push](#push)). It runs in the `tsk` client alone, from inside any working tree of the
managed repo. If `.git/tsk-clone-id` is absent or empty, or the database does not exist,
it stops with exit 1 and names `tsk ledger fetch`. The push does not mint a clone ID.

Each command that writes does one push after all its changes, not one per change.

## External event commands

The `tsk events` commands replace `append-external-event.sh`,
`read-new-external-events.sh` and `advance-external-events-watermark.sh`. They run in the
`tsk` client alone. Each one runs `tsk ledger fetch`'s steps first, as the
scripts run `fetch-bootstrap-ref.sh`. The three that write push once through
`tsk ledger push`'s steps.

### `tsk events append <source> <event-type> <action> <repo> <payload-file>`

1. If `<payload-file>` is not a file, stop with exit 1 and
   `error: no such payload file: <payload-file>`. If it holds anything that is not JSON,
   stop with exit 1. Both checks run before the fetch, so nothing is fetched or pushed.
2. Fetch the ledger.
3. Append one envelope row to `external_events`. `payload` is the first JSON value in the file, or `null`
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
5. Append one envelope row per event, in stdin order,
   with the field order and format of `tsk events append`. Every envelope in the batch
   has the same `received_at`, the current UTC time. `payload` is the line's value with
   whitespace outside strings removed and key order kept.
6. Push once, with the message `External event queue: append batch of <n> event(s)`
   (`event` for one, `events` otherwise).
7. Print `queued:<n>`.

### `tsk events read-new`

Fetches the ledger, then prints one compact JSON object and writes nothing:
`{"new_count":N,"total_count":M,"events":[...]}`. `total_count` is the number of
rows in the queue. `events` holds positions `processed_through + 1` to the
end, oldest first, each compacted with key order kept, and `new_count` is their number.
With no rows, or with `processed_through` at or past `total_count`, `events` is
empty and `new_count` is `0`. A queue row that is not JSON stops with exit 1.

### `tsk events advance-watermark <count>`

1. `<count>` must be decimal digits only. Otherwise stop with exit 1 and
   `tsk events advance-watermark: count must be a non-negative integer, got '<count>'`,
   before the fetch.
2. Fetch the ledger and read `processed_through`.
3. A lower count stops with exit 1 and
   `tsk events advance-watermark: refusing to move the watermark backwards (<current> -> <count>)`.
4. An equal count prints `note: watermark already at <current>; nothing to advance.` to
   stderr, prints nothing on stdout, pushes nothing, and exits 0.
5. A higher count writes the watermark row, pushes with the message
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
   store.
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
   ends `It has no open tasks.` When the thread also has a continuation entry, one more
   sentence follows: `Its latest pause entry is on task <id> (<description>), written by
   <actor> at <timestamp>. What's next: <text>`, then `WARNING: thread <id> is already
   associated with other actor(s): <actor>.` when another actor wrote it, then `Write the
   resume summary from this and run no command until the person confirms the thread.`
   The mission name, the task description and the entry are read from the local ledger store,
   with no further network call. A thread with no mission, or with no entry, gets the
   shorter text it had before.
2. When the ledger store holds commits that are not on the remote ledger branch, a
   warning that starts `WARNING: the ledger worktree holds a commit that is not on
   <remote>'s <ref>, so it was left as it is rather than reset:`, lists the commits
   separated by `;`, and gives the `dolt log` command to read them and
   `tsk ledger push` to push them.
3. With a binding, `An existing thread binding was found: <binding>. Run /tsk:resume-thread
   <thread-id> next.` Without one, the unbound prompt text. When the ledger has mission
   records, the prompt lists the missions not done, in the format of `tsk mission list`,
   and offers them; it does not mention `index.md`. Both prompts end by naming
   `tsk thread skip` as the choice for no thread. When this session or code worktree
   skipped a thread, the context says so in place of the prompt. When this session or
   code worktree is registered as an actor (`tsk actor register`), `Actor: <name>
   (<kind>).` follows.
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
| `thread-append-handover.sh` | `paused:<thread-id>`, then `code: clean` or `code: dirty (N files)`, `code pushed: yes` or `no`, and `ledger: pushed` or `ledger: N changes not pushed` | | `tsk thread pause <thread-id> <task-id> <whats-next>` |
| `thread-resume.sh` | `{"thread_id":"...","latest":{...},"warning":"...","mission":{"id","name"}|null,"task":{"id","description"}|null}` (`latest` is `{}` with no entry; `warning` is empty unless another actor wrote to the thread; `mission` is null with no mission record; `task` is null when the latest entry names no task the mission has) | | `tsk thread resume <thread-id>` |
| `thread-detach.sh` | `detached:<thread-id>` | exit 1, nothing on stdout, with no binding | `tsk thread detach` |
| `thread-stop.sh` | `stopped:<thread-id>` | | `tsk thread stop [<thread-id>]` |
| `thread-list.sh` | one compact JSON object per line: `id`, `urn` (`urn:tsk:thread:<ledger-id>:<id>`, absent below layout 12), `mission_id` (`null` for a thread with no mission), `mission_link`, `latest_whats_next`, `latest_timestamp` (both `null` with no entry), `actors` (an array of `{"actor", "kind", "name"}`, one for each session or worktree bound to the thread, oldest binding first; `kind` is `cloud` or `worktree`; `name` is the actor the instance registered as, or `null`; empty when none is bound) | | `tsk thread list` |
| `thread-resolve-binding.sh` | `cloud:<thread-id>` or `worktree:<thread-id>`, then `actor: <name> (<kind>)` on a second line when the session has a primary actor | exit 1, nothing on stdout, with no binding | `tsk thread binding`; `--no-fetch` in place of passing an already fetched ledger store |
| (none) | `registered <name> (<kind>)` for a new name; `<name> (<kind>) is now this session's actor` or `... this worktree's actor` for a name already registered with that kind; then `urn:tsk:actor:<uid>` on a second line | exit 1 with `<name> is registered as a <kind>` for the other kind, or for a name that is empty, over 200 characters or starts with `/` | `tsk actor register <name> --kind human\|robot` |
| (none) | the name of the session's primary actor, or `no actor registered` | | `tsk actor name` |
| (none) | one line per actor of the session: role, name, kind, uid, tab-separated; or `no actor registered` | exit 1 with no session URN and no `--session` | `tsk session actor [--session <urn>]` |
| (none) | `<name> (<kind>) joined <session urn> as primary|supporting` | exit 1 for an unknown uid, a value that is not a session URN, or no session | `tsk actor join <uid> [--primary] [--session <urn>]` |
| (none) | `urn:tsk:ledger:<ledger-id>` | exit 1 below layout 12 | `tsk ledger id` |
| (none) | one actor per line: name, kind, uid, instances (comma-separated keys, or `-`); tab-separated with `--porcelain`; the records with `urn`, `names` and `instances` with `--json` | | `tsk actor list [--porcelain\|--json]` |
| (none) | `removed <actor> <thread-id>` per binding, then `<n> bindings removed`; `would remove ...` and `<n> bindings would be removed` with `--dry-run` | | `tsk thread prune-bindings [--older-than <days>] [--dry-run]` |
| (none) | `mission-set:<thread-id>:<mission-id>`, or `mission-cleared:<thread-id>` with `--none` | exit 1 for an unknown mission or thread, or with no thread ID and no binding | `tsk thread set mission <mission-id> [<thread-id>]`, `tsk thread set mission --none [<thread-id>]` |
| `thread-binding-guard.sh` | nothing when bound, after `tsk thread skip`, or when the ledger holds no missions; `{"decision":"block","reason":"..."}` otherwise | | `tsk thread guard` |
| `claude-session-start.sh` (the tsk parts) | one JSON object, `{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"..."}}` | exit 0 in every case, including a failed fetch, which prints a warning context | `tsk thread session-start` (see [Session start command](#session-start-command)) |
| `thread-scaffold.sh` | nothing | exit 1 when `threads/<thread-id>` exists | internal to `tsk thread start` |
| `thread-mint-id.sh` | `<thread-id>` | | internal to `tsk thread start` |
| `append-external-event.sh` | `queued` | | `tsk events append <source> <event-type> <action> <repo> <payload-file>` |
| `poll-security-alerts.sh` | `queued:<count>`, or `no open alerts queued` | | not replaced: stays a script, calling `tsk events append-batch` from T-07, which prints `queued:0` for zero events (see [External event commands](#external-event-commands)) |
| `read-new-external-events.sh` | `{"new_count":N,"total_count":M,"events":[...]}` | | `tsk events read-new` |
| `advance-external-events-watermark.sh` | `watermark:<count>` | exit 0 and no stdout when already at the count; exit 1 when the count is lower | `tsk events advance-watermark <count>` |
