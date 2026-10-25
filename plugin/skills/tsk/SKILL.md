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
local ledger store (`ledger-db`) outside the repository. The ref holds a Dolt file remote (a
directory of table files) that `tsk` moves with plain `git fetch` and `git push`; a ledger
made before 0.21.0 converts once with `tsk ledger migrate-transport`. A new ledger uses a custom ref,
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

A session or code worktree has one binding, so binding it again replaces the old one. More than one session
or code worktree can bind to the same thread, and `tsk thread list` shows them in `actors`,
an array of `{"actor", "kind", "name"}` objects (`cloud` or `worktree`; `name` is the actor
the instance registered as, or `null`), empty when none is bound.
Resuming a thread that another actor wrote to binds anyway, and prints a warning.

An **actor** is a human or a robot, with a name and a kind. Its identity persists outside
any harness instance. A **harness instance** is the running harness: a cloud session, or a
code worktree of a clone, the same key as `bindings.actor`. An instance registers as an
actor with `tsk actor register <name> --kind human|robot`, which is separate from binding
to a thread and needs none. A new name makes the actor; a name already registered must
have the same kind, and the instance then joins it. The registration stays through a
rebind, a detach and a stop. `tsk actor name` prints the name this instance is registered
as, or `no actor registered`. `tsk actor list` lists the actors with their instances.
`tsk thread binding` and the session start context name the actor when the instance is
registered. Registering is a local write; the next push publishes it.

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
tsk ledger migrate-transport             convert a ledger that holds Dolt's older git blobstore to a Dolt file remote carried by plain git; run once, other clones follow on their next fetch
tsk ledger server status|start|stop      inspect or control the local Dolt server of this clone's ledger store; it runs unless TSK_DOLT_SERVER=off or [ledger] server = "off" (on by default since 0.29.0; `tsk config ledger-server` prints and sets the switch); while it runs, `dolt` opens the store read only
tsk ledger doc get <path>                print a document's body
tsk ledger doc put <path> [<file>]       write a document from <file>, or from stdin; changes the local store only
tsk ledger doc list [<prefix>]           list document paths, optionally under a prefix
tsk ledger doc rm <path>                 remove a document; changes the local store only
tsk ledger status [--local] [--porcelain]  print Local (what is not pushed) and Origin (ahead or behind); --local makes no network call and prints Origin as not checked; --porcelain prints tab-separated facts for scripts; read-only
tsk ledger discard [--yes] [--porcelain] drop every local change that is not pushed; without --yes it only prints what it would drop and exits 1

tsk thread start [<mission-id>]                                        mint and bind a thread, for a mission or with no mission, push (the claim)
tsk thread pause <thread-id> <task-id> <whats-next>                   append a continuation state entry, push, print the verification lines
tsk thread pause --bound <task-id> <whats-next>                       the same for the thread this session is bound to
tsk thread set mission <mission-id> [<thread-id>]                     attach or change a thread's mission; local, tsk ledger push publishes
tsk thread set mission --none [<thread-id>]                           remove a thread's mission; local, tsk ledger push publishes
tsk thread resume <thread-id>                                         bind to a thread, print its latest entry; local, tsk ledger push publishes
tsk thread detach                                                     remove this session's or code worktree's binding; local, tsk ledger push publishes
tsk thread skip                                                       work without a thread; local to this clone, never pushed
tsk thread stop [<thread-id>]                                         delete a thread and every cloud binding to it, push
tsk thread list                                                       list threads, most recently paused first
tsk thread title                                                      print the session title: <mission-id>: <actor name>, the name alone, or the mission ID alone; exit 1 with neither; read-only, never fetches
tsk thread binding [--no-fetch]                                       print the current binding, and the actor when registered
tsk actor register <name> --kind human|robot                          register this session or code worktree as an actor; local, tsk ledger push publishes
tsk actor name                                                        print the actor this session or code worktree is registered as
tsk actor list [--porcelain|--json]                                   list the actors and the instances registered as each
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

After `tsk thread set mission`, set the session title to the output of `tsk thread title`:
with `set_session_title` from the `claude-code-remote` MCP server in a cloud session, or
by asking me to run `/rename <title>` in a terminal. The start, resume and
construct-robot skills set it the same way.

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

A repo may check its documents with a hook, `pre-docs-commit` (`tsk config docs-hook`
prints the script that applies). When `doc put`
exits 1 and prints findings, the document is written locally: fix it and put it again.
A `tsk ledger push` that exits 1 with the same findings names a document changed earlier:
fix that one, then push again. Do not set `TSK_SKIP_DOC_CHECK=1` to get past them.

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

**Rules for an agent.** These hold in every repo:

- Check whether the ledger exists and what it holds with `tsk ledger status`. Its `Ledger`
  block names the ledger store, the refs in use and the counts of missions, tasks, threads
  and actors. The `Origin` line says whether origin has the ledger yet.
- Do not run `ls`, `find`, `cat` or `git` against the ledger store. It is a Dolt database,
  not a directory of files, and a `tsk` command reads it.
- Do not pipe `tsk` output through `head`, `tail` or another command. The pipe hides the
  exit code. Run the command alone and read its exit code and its output. `--porcelain`
  and `--json` give a script the stable form.
- An empty `list` prints one line on stderr and exits 0. It is not a failure.

Outside a git repository the `ledger`, `mission`, `task`, `decision`, `question`,
`thread` (except the `session-start` and `guard` hooks) and `events` commands stop with
`error: not inside a git repository. Run tsk from inside the repository whose ledger you want.`

To bring an existing plan, in any format, into the ledger, run `tsk guide import` and
follow it. It explains what the ledger holds and how to map a plan onto missions. Do not
read the tsk source code to learn the format.

`tsk ledger fetch` and `path` print a bare path on stdout, so
`WT="$(tsk ledger fetch)"` works. `tsk ledger push` runs `dolt add -A` and `dolt commit`,
then up to 10 attempts of fetch, `dolt merge`, `dolt push` and (on a file remote) a plain `git push`. A merge conflict, such as
two clones editing one field, stops it with exit 1 and names the conflicting tables. The local
commit stays. `git.ledger.commit` in a
continuation state entry holds the Dolt commit hash. An older `tsk` finds no
`.tsk-ledger.toml` on the Dolt branch and stops before writing: update `tsk`.

**Read the `is now` lines after a push.** Two clones that each add a task, question,
objective outcome, constraint, out-of-scope item or decision to one mission take the same
number. The second push gives its rows the next free numbers and prints one line per change
on stdout, before the commit hash: `T-03 is now T-05 in M-X`, `Q-02 is now Q-03 in M-X`,
`objective 2 is now objective 3 in M-X`, and the same for `constraint`, `out-of-scope` and
`decision`. Use the new numbers from then on. Text that names the old number, in a document
or a thread entry, does not change, so the printed lines are the only record. A push that
renumbers nothing prints the commit hash alone. No flag turns renumbering off.

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
tsk mission prefix [<PREFIX>]                 # print or set the ledger's mission prefix, 3 or 4 capitals
tsk mission mv <old-id> <new-id> [--dry-run] [--rewrite]  # rename: only the ID changes; linked documents named for it are renamed; --rewrite also replaces the old ID in record text
tsk mission show <id>                       # one mission as plain text for a person: the data of list <id> --json, no new field
tsk doctor                                  # check the environment (tsk on PATH, dolt, jq, git, origin, ledger refs, plugin, Dolt server); prints the fix for each fault; exit 1 when any fails
tsk mission rewrite <old-id> <new-id> [--dry-run]  # replace an old ID, as a whole word, in the text of every mission (names, purposes, objectives, constraints, out-of-scope items, tasks, decisions, questions, answers); <new-id> must be a mission
tsk mission list [<id>] [--json] [--porcelain]
```

A ledger can have a mission prefix, such as `TSK` (ADR 0027). With one, `create` and `mv`
add it to an ID that lacks it (`M-BOOT` becomes `M-TSK-BOOT`) and print the ID that results.
A number at the end of an ID marks a part of a mission and nothing else: `M-TSK-BOOT-01` is
a part of `M-TSK-BOOT`, which has to exist, and it becomes the parent. A separate mission of
the same kind gets a name, not a number. A ledger with no prefix has none of these rules and
prints a note that says how to set one. If `mission prefix` says the ledger has none, choose 3
or 4 capital letters for the repo from its name, tell the person, and set it. A mission has an
internal ID, which a rename never changes, and the ID a person types. Every
command takes and prints the second. `list <id> --json` has the first as `uid`. A rename
changes one cell, so another clone can add records to the mission while you rename it, and
the two merge. `mv` prints one line per change (`mission M-OLD -> M-NEW`, `doc A -> B`),
then `mentions <mission> <record>` for each record whose text names the old ID. It does not
change that text: read the lines and decide. Use `--dry-run` to see the lines first. A new
ID has the form of any mission ID. After a rename, the thread of the mission names it under
the new ID, and continuation entries written before keep the ID they were written with.

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

### When records are published

Write freely, publish at four points. A record write (`tsk mission`, `tsk task`,
`tsk decision`, `tsk question`, `tsk ledger doc put`) changes the local ledger store only
and never pushes. Records go out at:

1. thread start: the claim and the binding,
2. thread pause: the hand-off,
3. the end of the session: the SessionEnd hook, and the Stop hook, which publishes in the
   background after a turn when the store holds unpushed changes. It does not ask you to
   push. A failed background publish shows as `warning: the last background publish
   failed ...` on the next `tsk` command: run `tsk ledger push`,
4. when asked: `tsk ledger push "<message>"`.

`tsk thread resume`, `detach`, `set mission`, `skip` and `prune-bindings` write locally and
do not push. `tsk thread stop` pushes. Progress can wait: marking a task done, adding a
task, recording a decision or answering a question goes out with the next of the four
points. A stale read is acceptable.

A claim must not wait, so that two actors do not take the same task. Two things are claims:
`tsk thread start`, which publishes by itself, and setting a task or a mission to
`in-progress` to take it. The second is a record write and does not push, so run
`tsk ledger push "Claim <mission> <task>"` straight after it. A container that is
reclaimed before the pause loses the records that were never published.

### Tasks, decisions and questions

A mission's tasks, decisions and open questions are data too. Record and change them only
with these commands, and never write them as Markdown in a briefing or any other document.
Every command changes the local store only and never pushes (see "When records are
published"). Setting a task to `in-progress` to take it is a claim: run `tsk ledger push
"Claim <mission> <task>"` straight after.

```
tsk task add <mission> <description>          # prints the new ID, T-01 upward
tsk task set <mission> <task> <field> <value> # status (todo|in-progress|done) or description
tsk task block <mission> <task> <by>          # and unblock; <by> is a task of the same mission
tsk task list <mission> [--porcelain]
tsk task mv <mission> <task> <to-mission> [--force]   # next number in the target; its decisions move with it
tsk decision add <mission> "<text>" --by <who> [--task T-nn] [--on YYYY-MM-DD] [--adr <path>]
tsk decision list <mission> [--adr <path>] [--porcelain]
tsk decision list [--adr <path>]                # no mission: every decision of every mission, or those that name the ADR
tsk decision mv <mission> <n> <to-mission> [--task T-nn]   # n is the place in decision list, from 1
tsk question add <mission> "<text>" [--on YYYY-MM-DD]   # prints the new ID, Q-01 upward
tsk question edit <mission> <q> "<text>"      # rewrites an open question; an answered one is refused
tsk question answer <mission> <q> "<text>" --by <who> [--on YYYY-MM-DD]
tsk question list [<mission>] [--porcelain]   # open questions, all missions when none is given
tsk question mv <mission> <q> <to-mission>
```

`mv` moves a record to another mission, which gives it the next number there. The number it
had is not used again, and each command prints one line, such as `task M-A T-05 -> M-B T-12`.
A task with blocks is refused, because a block belongs to one mission: unblock it, or add
`--force` to drop them. A decision loses its task link unless `--task` names a task of the
target. `tsk ledger doc mv <from> <to>` renames a document and updates the mission links.

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
--no-fetch` and `guard`. Only `thread start`, `thread pause` and `thread stop` push, once,
after all their changes, through the same code as `tsk ledger push`. `thread resume`,
`detach`, `set mission`, `skip` and `prune-bindings` write to the local ledger store and
print `note: saved in the local ledger store; it goes out with the next push ...` on
stderr (`resume`, `detach` and `set mission`). Their stdout is unchanged. Diagnostics go
to stderr.

| Command | Stdout on success | Other exits |
|---|---|---|
| `thread start` | `started:<thread-id>` | `resume-required:<thread-id>` and exit status 2 when a binding exists |
| `thread pause` | `paused:<thread-id>`, then `code: clean` or `code: dirty (N files)`, `code pushed: yes` or `no`, `ledger: pushed` or `ledger: N changes not pushed` | exit 1 when the managed repo's `HEAD` is not on any branch on origin, or when a thread with no mission gets a task ID |
| `thread set mission` | `mission-set:<thread-id>:<mission-id>`, or `mission-cleared:<thread-id>` with `--none` | exit 1 for an unknown mission or thread, or with no thread ID and no binding |
| `thread resume` | `{"thread_id":"...","latest":{...},"warning":"...","mission":{"id","name"}|null,"task":{"id","description"}|null}` | |
| `thread detach` | `detached:<thread-id>` | exit 1, nothing on stdout, with no binding |
| `thread stop` | `stopped:<thread-id>` | exit 1 with no binding and no thread ID given |
| `thread skip` | `skipped` | |
| `thread list` | one compact JSON object per line: `id`, `mission_id` (null for a thread with no mission), `mission_link` (empty for a mission with a record), `latest_whats_next`, `latest_timestamp`, `actors` (each `{"actor", "kind", "name"}`) | |
| `thread binding` | `cloud:<thread-id>` or `worktree:<thread-id>`, then `actor: <name> (<kind>)` on a second line when registered | exit 1, nothing on stdout, with no binding |
| `actor register` | `registered <name> (<kind>)` for a new name; `<name> (<kind>) is now this session's actor` or `... this worktree's actor` for a name already registered with that kind | exit 1 with `<name> is registered as a <kind>` for the other kind, or for a name that is empty, over 200 characters or starts with `/` |
| `actor name` | the actor name, or `no actor registered` | |
| `actor list` | one actor per line: name, kind, instances (comma-separated keys, or `-`); tab-separated with `--porcelain`; the records with their `instances` with `--json` | |
| `thread prune-bindings` | `removed <actor> <thread-id>` per binding, then `<n> bindings removed`; with `--dry-run`, `would remove <actor> <thread-id>` lines and `<n> bindings would be removed`, and nothing changes | |
| `thread guard` | nothing when bound, after `thread skip`, or when the ledger holds no missions; `{"decision":"block","reason":"..."}` otherwise, asking for a thread. With unpushed changes it also starts `tsk ledger push --background` and prints nothing | |

`thread prune-bindings` removes the bindings of cloud sessions that ended, which collect in
the ledger because a cloud session rarely detaches. It removes a cloud row registered more
than `--older-than` days ago (30 by default), except the row of the session that runs it.
It never removes a worktree row. It is a local write like the record commands, so run `tsk
ledger push` to publish it. Try `--dry-run` first.

In `thread resume`, `mission` and `task` contain the mission name and the task description,
so no lookup follows. `latest` is the thread's last continuation state entry as stored, or
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
