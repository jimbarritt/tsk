# Ledger layout

Reference for every file in a tsk ledger: where the ledger lives, the ref that holds it,
where it is checked out, its tree, and the format and fields of each file. The scripts
in `ops/local/` defined these formats before this document existed. This document is
taken from them, and `tsk ledger fetch`, `tsk ledger push`, the `tsk thread` commands
and the external event commands (M-BOOT-04, T-03 to T-07) build against it.

Depends on: `docs/domain/session-continuation-design.md` (threads, bindings and
continuation state), `docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md`,
`docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md`,
`docs/adr/0010-ledger-stays-a-branch-not-a-directory-on-main.md`.

## Contents

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

`tsk config attach-nexus <url>` records the nexus repo URL in the user config:

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
- `tsk config show` prints the config path and the attached URL.

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

Entries are added to `nexus.json` by hand. No command registers a repo. Automatic
transition of an entry from `local` to `url` on the first push is not implemented.

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

Free-form Markdown, written by humans and agents. No command parses either file.

- `index.md` is the entry point: current state and next steps. An agent reads it first.
- `future-missions-tbd.md` holds mission ideas not yet shaped into briefings.

## `missions/`

Mission briefings and the files missions leave behind, as Markdown. The binary moves
these files between the ledger and the ledger worktree without parsing them. Encoding the
briefing format is a later mission.

Layout in use:

| Path | Content |
|---|---|
| `missions/administrative/`, `missions/operational/` | One directory per mission category. |
| `missions/<category>/M-<ID>-<slug>.md` | A mission briefing. `<ID>` is the mission ID, for example `BOOT-04`. |
| `missions/<category>/M-<ID>/` | A mission's supporting files: sub-briefings, intelligence indexes, reports. |
| `missions/<category>/M-<ID>-<slug>-report-<YYYY-MM-DD>.md` | A dated report beside its briefing. |

The one structural rule a command enforces: `thread start` takes a briefing path
relative to the ledger root, for example
`missions/operational/M-BOOT-04-official-ledger.md`, and refuses it unless a file exists
at that path in the ledger worktree.

## `threads/`

The thread store from `docs/domain/session-continuation-design.md`.

### `threads/<thread-id>/`

One directory per thread. The directory name is the thread ID (see
[Common value formats](#common-value-formats)). Created by `thread start`, deleted with
its contents by `thread stop`. Its presence is the collision check when a new thread ID
is minted.

### `threads/<thread-id>/index.md`

Written once, at thread start. Exact content, with a trailing newline:

```
# Thread <thread-id>

Mission briefing: [<briefing-path>](<briefing-path>)
```

`<briefing-path>` is the path relative to the ledger root that `thread start` was given.

`thread list` reads the mission link from this file: the text between
`Mission briefing: [` and the next `]`, first match. A missing file or no match gives an
empty string.

### `threads/<thread-id>/continuation-state.jsonl`

Append-only. Created empty at thread start. One JSON object per line, compact (no
whitespace between tokens), terminated by `\n`. Each line is one continuation state
entry, written by `thread pause` (today `thread-append-handover.sh`). Lines are never
rewritten or removed, except that `thread stop` deletes the whole file with the thread.

Fields, in the order written. `git` holds `ledger` then `code`; `ledger` holds `commit`;
`code` holds `ref` then `commit`:

| Field | Type | Source | Meaning |
|---|---|---|---|
| `mission_link` | string | caller | The mission briefing the thread works on, as a path relative to the ledger root. |
| `task_id` | string | caller | The task in progress at pause time, for example `T-03`. Can be empty. |
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

Written by the location resolution when it finds an entry, and by hand for a repo with
no `url` (an entry that carries `local`), whose entry is found only through this file. It
is read first on every later run, so a change of the repo's remote URL does not lose the
entry. Never pushed.

### `tsk-thread-id`

Path: `$(git rev-parse --path-format=absolute --git-dir)/tsk-thread-id`. The git dir,
not the common dir: `.git/tsk-thread-id` in the main code worktree,
`.git/worktrees/<name>/tsk-thread-id` in a linked one. One per code worktree.

Content: a thread ID and a trailing newline. Readers strip all whitespace. An absent or
empty file means no code worktree binding.

Written by `thread start` and `thread resume` when `CLAUDE_CODE_REMOTE_SESSION_ID` is not
set. Deleted by `thread detach`, and by `thread stop` when it names the stopped thread.
Never pushed: a marker in another clone or code worktree that names a stopped thread stays
there, stale.

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

1. Read standard input to the end and discard it. The hook input can be empty.
2. Fetch the ledger. A fetch failure, or a directory outside a git repository, skips to
   the failure output below.
3. When the environment variable `CLAUDE_ENV_FILE` is set and not empty, append
   `export TSK_LEDGER_WT="<path>"` and a newline to that file. A write failure adds a
   warning to the context and does not stop the command.
4. Resolve the binding of the session, as `tsk thread binding` does, from the fetched
   ledger worktree.
5. Print one JSON object on one line and exit with status 0.

Standard output on success:

```json
{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"..."}}
```

`additionalContext` is these parts, joined by single spaces:

1. `The ledger was fetched and materialised at <path> (also exported as $TSK_LEDGER_WT). Read <path>/index.md next.`
2. When the ledger worktree holds commits that are not on the remote ledger branch, a
   warning that starts `WARNING: the ledger worktree holds a commit that is not on
   <remote>'s <ref>, so it was left as it is rather than reset:`, lists the commits
   separated by `;`, and gives the `git log -p <remote-tip>..HEAD` command to read them and
   `tsk ledger push` to push them.
3. With a binding, `An existing thread binding was found: <binding>. Run /tsk:resume-thread
   <thread-id> next.` Without one, the unbound prompt text.

On failure the output is the same object with the context
`Warning: the ledger could not be fetched automatically at session start (<error>). Run
`tsk ledger fetch` manually before reading index.md.` and the exit status is 0.

The command does not unshallow the clone, install plugins, or stash, check out or pull
the code worktree. Those steps stay in the repo's own hook script.

## Script outputs

The skills and hooks parse these outputs today. The binary commands that replace the
thread scripts (T-05) and the external event scripts (T-06) print the same stdout and
exit with the same status, so T-07
switches the harness over by changing only the command it calls. This table records
what the harness expects until then.

| Script | Stdout on success | Other exits | Replaced by |
|---|---|---|---|
| `thread-start.sh` | `started:<thread-id>` | `resume-required:<thread-id>` and exit 2 when a binding exists | `tsk thread start <mission-id> <briefing-path>` |
| `thread-append-handover.sh` | `paused:<thread-id>` | | `tsk thread pause <thread-id> <mission-link> <task-id> <whats-next>` |
| `thread-resume.sh` | `{"thread_id":"...","latest":{...},"warning":"..."}` (`latest` is `{}` with no entry; `warning` is empty unless another actor wrote to the thread) | | `tsk thread resume <thread-id>` |
| `thread-detach.sh` | `detached:<thread-id>` | exit 1, nothing on stdout, with no binding | `tsk thread detach` |
| `thread-stop.sh` | `stopped:<thread-id>` | | `tsk thread stop [<thread-id>]` |
| `thread-list.sh` | one compact JSON object per line: `id`, `mission_link`, `latest_whats_next`, `latest_timestamp` (both `null` with no entry) | | `tsk thread list` |
| `thread-resolve-binding.sh` | `cloud:<thread-id>` or `worktree:<thread-id>` | exit 1, nothing on stdout, with no binding | `tsk thread binding`; `--no-fetch` in place of passing an already fetched ledger worktree |
| `thread-binding-guard.sh` | nothing when bound; `{"decision":"block","reason":"..."}` when not | | `tsk thread guard` |
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
