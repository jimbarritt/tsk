---
name: tsk
description: Reference for the tsk command line: the ledger, threads and bindings, continuation state, and the external event queue. Load when working with tsk missions, threads or the ledger, or when a tsk command is needed.
---

# tsk: agent context

tsk tracks missions and the threads that work them. The missions, their tasks, decisions
and questions, the threads and their continuation state live in a **ledger**. They are
records, typed rows in a Dolt database, written only through `tsk` commands. Free text
(intelligence, research, reports) is Markdown documents on a second ref. The ledger is a
Dolt database synced over a git ref on the managed repo's `origin` by default, cloned to a
local ledger store (`ledger-db`) outside the repository. A new ledger uses a custom ref,
`refs/tsk/ledger`. A repo used from Claude Code cloud sessions uses the branch
`refs/heads/tsk/ledger`, because the cloud proxy refuses to push a custom ref (see Ledger
refs below). For a repo that cannot hold a ledger ref, the ledger is
`refs/tsk/ledgers/<repo-id>` (or `refs/heads/ledgers/<repo-id>`) in the nexus repo (see
Nexus ledgers below). The `ledger`, `thread` and `events` commands run in the `tsk` binary alone, with no
daemon, from inside any working tree of the managed repo.

Design references in the tsk repository: every ledger file and format is in
`docs/domain/ledger-layout.md`. Threads, bindings and continuation state are in
`docs/domain/session-continuation-design.md`.

## Core concepts

A **thread** is one line of work on a mission. Its ID is 8 characters from `[0-9a-z]`.
A thread names a mission, or has none, and holds one continuation state entry per pause.
The ledger keeps a thread in tables: a `threads` row, its continuation entries and its
bindings.

To see the state of the work, run `tsk mission list` for the missions, `tsk mission list
<id> --json` for one mission in full, `tsk task list <id>`, `tsk decision list <id>` and
`tsk question list`. There is no index document and no briefing document to read.

A **binding** ties the current session or code worktree to a thread:

Every binding is one row of the ledger's `bindings` table, keyed on the actor:

- In a cloud session (`CLAUDE_CODE_REMOTE_SESSION_ID` set), the actor is the session ID.
- Otherwise, the actor is the code worktree: `<clone-id>/<worktree-name>`, where the clone
  ID is in `.git/tsk-clone-id` and the worktree name is `main` for the main worktree or the
  name of the linked worktree.

An actor has one binding, so binding it again replaces the old one. More than one session
or code worktree can bind to the same thread, and `tsk thread list` shows them in `actors`,
an array of `{"actor", "kind"}` objects (`cloud` or `worktree`), empty when none is bound.
Resuming a thread that another actor wrote to binds anyway, and prints a warning.

Older tsk versions kept the binding of a code worktree in the file `tsk-thread-id` in its
git directory. The first tsk command that resolves such a binding writes the row from the
file and removes the file. Do not write the file. `tsk thread skip` is still a local file
and is never in the ledger.

## Upgrading tsk

`tsk self-update` upgrades the binary to the latest release by the method it was installed
with (Homebrew, Cargo or a release binary). `tsk self-update --check` only reports. When
the session context says a newer tsk is available, run `tsk self-update`.

## Ledger refs

`tsk config ledger-refs` prints which refs hold the ledger and where the choice came from.
`tsk config ledger-refs branches` or `custom` writes `.tsk/config.toml` in the repo.
A custom ref is the default for a new ledger, and GitHub does not list it as a branch.
Branches are for a repo used from Claude Code cloud sessions, because the cloud proxy
refuses to push a custom ref. The order is: the nexus entry (`ledger_refs`), then
`.tsk/config.toml`, then discovery on `origin`. A new ledger made in a cloud session takes
branches and says so. Set the choice before the first push. tsk does not move an existing
ledger between the two. When a push stops with `origin refused the push to refs/tsk/...`,
run `tsk config ledger-refs branches` and push again. Run `git` against these refs only
with the full name, such as `refs/tsk/ledger`.

## Nexus ledgers

`tsk nexus add <url>`, or `tsk config attach-nexus <url>`, writes the nexus URL to
`${XDG_CONFIG_HOME:-~/.config}/tsk/config.toml`. A repo's entry in the nexus's `nexus.json`
with `"ledger": "nexus"` holds its ledger on `ledgers/<repo-id>` in the nexus (under
`refs/tsk/` or `refs/heads/`, see Ledger refs), where `<repo-id>` is the entry's `id`. With no attached nexus, no matching entry or no
`ledger` field, the ledger is in-repo. tsk finds the entry through the repo ID cached in
`.git/tsk-repo-id`, otherwise by matching the repo's raw `remote.origin.url` against each
entry's `url`. An entry with `"local": "<machine name>"` and no `url` is found only through
the cached ID, on that machine. `tsk nexus register-repo`, run in the repo, adds its entry
to `nexus.json` and pushes it (the `register-repo` skill runs it). Every ledger,
thread and events command works the same in both locations. The details are in
`docs/domain/ledger-layout.md`.

When the ledger branch does not exist yet, `tsk ledger fetch` creates a new ledger in the
local ledger store, and the first `tsk ledger push` creates the branch.

## Commands

```
tsk config attach-nexus <url>            record the nexus repo URL in the user config; idempotent, reports a replacement
tsk config show                          print the user config path and the attached nexus URL
tsk nexus add <url>                      the same as tsk config attach-nexus
tsk nexus list [--json]                  print the config path, the nexus URL, and the territories and repos in its nexus.json
tsk nexus register-repo [--id <id>] [--territory <id>] [--territory-name <name>] [--ledger nexus|repo] [--local]
                                         add this repo's entry to nexus.json, commit and push it; idempotent

tsk ledger fetch                         fetch the ledger branch (creating a new ledger when it does not exist), refresh the local ledger store, print the path
tsk ledger path                          print the ledger store path (the Dolt database); no fetch, no writes
tsk ledger push "<message>"              commit every local ledger change, pull and push; prints the ledger commit
tsk ledger doc get <path>                print a document's body
tsk ledger doc put <path> [<file>]       write a document from <file>, or from stdin; changes the local store only
tsk ledger doc list [<prefix>]           list document paths, optionally under a prefix
tsk ledger doc rm <path>                 remove a document; changes the local store only
tsk ledger status [--local] [--porcelain]  print Local (what is not pushed) and Origin (ahead or behind); --local makes no network call and prints Origin as not checked; --porcelain prints tab-separated facts for scripts; read-only
tsk ledger discard [--yes] [--porcelain] drop every local change that is not pushed; without --yes it only prints what it would drop and exits 1

tsk thread start [<mission-id>]                                        mint and bind a thread, for a mission or with no mission, push
tsk thread pause <thread-id> <task-id> <whats-next>                   append a continuation state entry, push
tsk thread set mission <mission-id> [<thread-id>]                     attach or change a thread's mission, push
tsk thread set mission --none [<thread-id>]                           remove a thread's mission, push
tsk thread resume <thread-id>                                         bind to a thread, print its latest entry
tsk thread detach                                                     remove this session's or code worktree's binding
tsk thread skip                                                       work without a thread; local to this clone, never pushed
tsk thread stop [<thread-id>]                                         delete a thread and every cloud binding to it, push
tsk thread list                                                       list threads, most recently paused first
tsk thread binding [--no-fetch]                                       print the current binding
tsk thread prune-bindings [--older-than <days>] [--dry-run]           remove old cloud bindings, local; tsk ledger push publishes
tsk thread guard                                                      Stop hook check for a binding; never fetches
tsk thread session-start [--format claude|copilot] [--plugin-version <semver>]
                                                                      SessionStart hook: fetch the ledger, export TSK_LEDGER_WT, print the hook output JSON; warns when the plugin is older than this tsk expects

tsk events append <source> <event-type> <action> <repo> <payload-file>   queue one event envelope, push
tsk events append-batch [--source <s>] [--action <a>]                    queue every NDJSON event line on stdin, push once
tsk events read-new                                                       print the events past the watermark; no writes
tsk events advance-watermark <count>                                      set the watermark to <count>, push
```

`<mission-id>` is a mission with a record, for example `M-TSK-SCHEMA`. `tsk thread start`
refuses an unknown ID.
`<task-id>` can be empty. When the thread's mission has a record, `tsk thread pause`
refuses a task that is not one of its tasks, and writes `T-5` as `T-01`. A thread started
with no mission ID has no mission and no tasks: its task ID must be empty, and
`tsk thread set mission <mission-id>` attaches a mission later. The old forms
`thread start <mission-id> <briefing-path>` and
`thread pause <thread-id> <mission-link> <task-id> <whats-next>` still work, with a note on
stderr. The path and the link are ignored. Do not use them.

### Ledger

The ledger store is a Dolt database at
`${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger-db`, synced over git
on `refs/tsk/ledger` or the branch `refs/heads/tsk/ledger` (see Ledger refs).
`$TSK_LEDGER_WT` names it. Do not open files in it and do not
run `dolt` against it by hand. The files backend was removed in 0.16.0: `tsk` stops with an
error when `[ledger] backend = "git"` is set in the user config or `TSK_LEDGER_BACKEND=git`
is set. Use `tsk` 0.15.1 to read an old file ledger.

Documents are plain Markdown files on a second ref, `refs/tsk/docs` or `refs/heads/tsk/docs`
(on a nexus ledger, `refs/tsk/ledgers/<repo-id>-docs` or `refs/heads/ledgers/<repo-id>-docs`). They are checked out as a worktree beside the
database, at `.../tsk/repos/<clone-id>/docs`. Read and change them with `tsk ledger doc`.
Documents hold intelligence, research and reports. They do not hold state: a mission's
purpose, tasks, decisions and questions are records, and a document must not repeat them.
Link a document to its mission with `tsk mission link`. To write one, run
`tsk ledger doc put <path>` (body on stdin, or a file argument), then
`tsk ledger push "<message>"`. `doc put` and `doc rm` change the local worktree only.
`tsk ledger doc path <path>` prints the absolute path of the document's file, to open it
in another tool. `tsk ledger push` pushes the documents first and the Dolt data second, so
a record never names a document the remote lacks. A rejected documents push stops it
with exit 1 and the Dolt data stays unpushed. Run it again.

The first `tsk` write on 0.14.0 upgrades the ledger to layout 8, and an older binary then
stops with `unsupported ledger version`. Run `tsk self-update` to bring it up to date.
From 0.18.0, a `tsk` that meets a newer layout prints a warning on stderr, updates itself
and runs your command again, so the command can take a minute. The hook commands never do
this. `TSK_AUTO_UPDATE=never` switches it off. A binary from before 0.18.0 cannot: run
`tsk self-update` yourself. A tsk newer than the ledger upgrades it on its first write,
never on a read.

A ledger at version 1 still keeps its documents in a Dolt table. Every command except
`tsk ledger fetch` refuses it with `this ledger stores its documents in Dolt (version 1)`.
Run `tsk ledger migrate-docs` once. It moves the documents to `tsk/docs` and prints
`migrated-docs:<count>`. `--dry-run` checks the move and pushes nothing.

To see what is not pushed, run `tsk ledger status`. It prints two answers for a person:
`Local   nothing to push` or `Local   N changes not pushed` with a detail line per changed
table or document, then `Origin` with `up to date`, `N data commits ... to fetch`, `... not
on origin`, `diverged: ...`, `not published yet` or `unknown`. It fetches the remote refs and
changes nothing else. **An agent that parses the output uses `tsk ledger status --porcelain`**,
which prints one tab-separated fact per line with no header: `data` and `docs` each `clean`
or `local changes`, then a line per changed table (with counts of rows added, modified and
deleted, or `schema changed`; a write that upgraded the schema prints one
`data<TAB>upgrade<TAB>schema upgraded to version N` line instead of the `manifest` and
schema-only table lines) or per changed file (`A`, `M` or `D`), then
`data<TAB>remote<TAB>...` and `docs<TAB>remote<TAB>...`, which say `up to date`, `ahead N`,
`behind N`, `diverged ahead N behind N`, `unknown` or `unpublished`. The default form may
change; the porcelain form is stable.

To undo work that is not pushed, such as a trial mission, run `tsk ledger discard`. It
prints what it would drop (the `Local` block, or the porcelain lines with `--porcelain`) and exits 1. Run `tsk ledger discard --yes` to drop every local
change in the data and the documents, including local commits that origin lacks, and
reset both to origin's state. It prints `discarded`. It leaves thread bindings alone.
**Use `tsk ledger discard`, never `dolt reset`, `dolt clean` or `git reset` against the
ledger.** Check `tsk ledger status` first: a discard cannot be undone.

Outside a git repository the `ledger`, `mission`, `task`, `decision`, `question`,
`thread` (except the `session-start` and `guard` hooks) and `events` commands stop with
`error: not inside a git repository. Run tsk from inside the repository whose ledger you want.`

To bring an existing plan, in any format, into the ledger, run `tsk guide import` and
follow it. It explains what the ledger holds and how to map a plan onto missions. Do not
read the tsk source code to learn the format.

`tsk ledger fetch` and `path` print a bare path on stdout, so
`WT="$(tsk ledger fetch)"` works. `tsk ledger push` runs `dolt add -A` and `dolt commit`,
then up to 5 attempts of `dolt fetch`, `dolt merge` and `dolt push`. A merge conflict stops it with exit
1 and names the conflicting tables. The local commit stays. `git.ledger.commit` in a
continuation state entry holds the Dolt commit hash. An older `tsk` finds no
`.tsk-ledger.toml` on the Dolt branch and stops before writing: update `tsk`.

### Missions

A mission is data. Create and change it only with `tsk mission` commands, and never edit a
mission row or file by any other route. A mission is done when its status is set to `done`.
tsk does not check its objectives or tasks. Every command changes the local store
only. `tsk ledger push` publishes.

```
tsk mission create <id> <name> [--category operational|administrative] [--parent <id>]
tsk mission set <id> <field> <value>          # name, status, purpose, decision-authority; `-` reads stdin
tsk mission constraint add <id> "<text>"      # prints n; also remove <id> <n> and list <id>
tsk mission out-of-scope add <id> "<text>"    # prints n; also remove <id> <n> and list <id>
tsk mission block <id> <by-id>                # and unblock
tsk mission objective add <id> <outcome>      # prints n; also done, undone, remove, list
tsk mission link <id> <path> --role <intelligence|report|design>   # and unlink
tsk mission list [<id>] [--json] [--porcelain]
```

Status is `todo`, `in-progress` or `done`. An administrative mission stays `in-progress`.
Blocked is derived: a mission is blocked while any mission it names in `block` is not
`done`. `list` prints `id`, `status`, `blocked-by`, `parent` and `name` in columns aligned
with spaces. Add `--porcelain` to print them tab-separated, the stable form for a script.
`list <id> --json` prints the full record. A linked path must be a document on `tsk/docs`,
written with `tsk ledger doc put`.

Constraints and out of scope are lists of one-line items, numbered per mission as objective
outcomes are, and a number is never reused. Add one item per command, and do not write a
paragraph: `tsk mission constraint add M-X "No new crates"`. `constraint list` and
`out-of-scope list` print the number and the text, tab-separated. `mission set constraints`
and `mission set out-of-scope` are refused. In `list <id> --json` both are arrays of
`{"n": 1, "text": "..."}`.

### Tasks, decisions and questions

A mission's tasks, decisions and open questions are data too. Record and change them only
with these commands, and never write them as Markdown in a briefing or any other document.
Every command changes the local store only. `tsk ledger push` publishes.

```
tsk task add <mission> <description>          # prints the new ID, T-01 upward
tsk task set <mission> <task> <field> <value> # status (todo|in-progress|done) or description
tsk task block <mission> <task> <by>          # and unblock; <by> is a task of the same mission
tsk task list <mission> [--porcelain]
tsk decision add <mission> "<text>" --by <who> [--task T-nn] [--on YYYY-MM-DD] [--adr <path>]
tsk decision list <mission> [--adr <path>] [--porcelain]
tsk decision list [--adr <path>]                # no mission: every decision of every mission, or those that name the ADR
tsk question add <mission> "<text>" [--on YYYY-MM-DD]   # prints the new ID, Q-01 upward
tsk question edit <mission> <q> "<text>"      # rewrites an open question; an answered one is refused
tsk question answer <mission> <q> "<text>" --by <who> [--on YYYY-MM-DD]
tsk question list [<mission>] [--porcelain]   # open questions, all missions when none is given
```

Text is one line. Every `list` here prints aligned columns: two spaces between columns, no
tabs, no trailing spaces. `--porcelain` prints the tab-separated lines instead, with no
header, and a script that parses the output must use it. `task list` prints `T-nn`,
`status`, `blocked-by` and `description`. A task is blocked while any task it names in `block` is not `done`.
`decision list` prints the date, who, the task or `-`, the text and the ADR path or `-`,
oldest first. With no mission it prints the mission ID first, for every decision of every
mission, or with `--adr` for every decision that names that ADR.
A decision links to an ADR when an ADR explains it: `--adr docs/adr/0014-x.md`, the path
from the root of the repository you are in. The file must exist there. A decision is a
one-line fact about the work. The reasoning belongs in the ADR, so link it and do not
repeat it.
`decision add` dates the decision today (UTC). When the decision was made earlier, give
its date with `--on`, a real calendar date such as `2026-03-14`, rather than writing the
date into the text.
`question add` and `question answer` date the question and the answer today (UTC) in the
same way. Give `--on` to keep the original date of a question you bring in from elsewhere.
`question list` prints the mission, `Q-nn`, the date asked and the text. Use `--by` with
the name of the person who made the decision or gave the answer. `tsk mission list <id>
--json` prints the mission with its tasks, decisions and questions.

### Threads

Every `thread` command that reads the ledger fetches it first, except `binding
--no-fetch` and `guard`. Every command that writes to the ledger pushes once,
after all its file changes, through the same code as `tsk ledger push`. Diagnostics go to
stderr.

| Command | Stdout on success | Other exits |
|---|---|---|
| `thread start` | `started:<thread-id>` | `resume-required:<thread-id>` and exit status 2 when a binding exists |
| `thread pause` | `paused:<thread-id>` | exit 1 when the managed repo's `HEAD` is not on any branch on origin, or when a thread with no mission gets a task ID |
| `thread set mission` | `mission-set:<thread-id>:<mission-id>`, or `mission-cleared:<thread-id>` with `--none` | exit 1 for an unknown mission or thread, or with no thread ID and no binding |
| `thread resume` | `{"thread_id":"...","latest":{...},"warning":"..."}` | |
| `thread detach` | `detached:<thread-id>` | exit 1, nothing on stdout, with no binding |
| `thread stop` | `stopped:<thread-id>` | exit 1 with no binding and no thread ID given |
| `thread skip` | `skipped` | |
| `thread list` | one compact JSON object per line: `id`, `mission_id` (null for a thread with no mission), `mission_link` (empty for a mission with a record), `latest_whats_next`, `latest_timestamp` | |
| `thread binding` | `cloud:<thread-id>` or `worktree:<thread-id>` | exit 1, nothing on stdout, with no binding |
| `thread prune-bindings` | `removed <actor> <thread-id>` per binding, then `<n> bindings removed`; with `--dry-run`, `would remove <actor> <thread-id>` lines and `<n> bindings would be removed`, and nothing changes | |
| `thread guard` | nothing when bound, after `thread skip`, or when the ledger holds no missions; `{"decision":"block","reason":"..."}` otherwise | |

`thread prune-bindings` removes the bindings of cloud sessions that ended, which collect in
the ledger because a cloud session rarely detaches. It removes a cloud row registered more
than `--older-than` days ago (30 by default), except the row of the session that runs it.
It never removes a worktree row. It is a local write like the record commands, so run `tsk
ledger push` to publish it. Try `--dry-run` first.

In `thread resume`, `latest` is the thread's last continuation state entry as stored, or
`{}` with no entry. `warning` is empty unless the thread's entries name a `written_by`
actor and none of them is the current actor.

A continuation state entry holds `mission_id`, `mission_link` (only for a thread whose
mission has no record), `task_id`, `whats_next`, a nested `git` object, `timestamp` and
`written_by`. The `git` object holds `ledger.commit`, `code.ref` and `code.commit`. An
older entry has `mission_link` and no `mission_id`.

`thread pause` records the commit the ledger store is at before the entry is appended,
and the managed repo's branch and `HEAD` commit. It refuses a `HEAD` that no branch on
origin holds, because an actor resuming from another clone cannot see it.

### External events

The external event queue is `external-events/queue.ndjson` on the ledger, one event
envelope per line: `source`, `event_type`, `action`, `repo`, `received_at` and `payload`.
`external-events/watermark.json` holds `processed_through`, the number of queue lines
processed. Each `events` command fetches the ledger first. `append`, `append-batch`
and `advance-watermark` push once, through the same code as `tsk ledger push`.

| Command | Stdout on success | Other exits |
|---|---|---|
| `events append` | `queued` | exit 1, nothing pushed, when the payload file is absent or not JSON |
| `events append-batch` | `queued:<count>`; `queued:0`, nothing fetched or pushed, for empty stdin | exit 1, nothing pushed, naming the stdin line, when any line is invalid |
| `events read-new` | `{"new_count":N,"total_count":M,"events":[...]}`, oldest first | |
| `events advance-watermark` | `watermark:<count>` | exit 0 and no stdout when already at the count; exit 1 when the count is lower |

`append` queues the first JSON value in the payload file, or `null` for an empty file.
`append-batch` reads one JSON object per stdin line, with `event_type`, `repo` and
`payload`, and `source` and `action` from the line or from `--source` and `--action`.
It checks every line before the fetch, then appends them all with one `received_at`
and one push.
`total_count` is the queue's line count. Advance the watermark to it only after every
event up to it is processed, so an interrupted run reads those events again rather than
skipping them.

### Thread skills

The plugin provides `/tsk:start-thread`, `/tsk:pause-thread`, `/tsk:resume-thread`,
`/tsk:detach-thread`, `/tsk:stop-thread` and `/tsk:switch-thread`. Each wraps the
`tsk thread` commands above.

### Switching threads

No single command switches threads. To switch: `tsk thread binding` to find the current
thread, `tsk thread detach`, optionally `tsk thread stop <old-thread-id>`, then
`tsk thread resume <new-thread-id>`. `tsk thread list` gives the candidates. To work with no
thread at all, run `tsk thread skip`.
