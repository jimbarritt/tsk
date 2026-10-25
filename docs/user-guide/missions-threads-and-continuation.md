# Missions, threads and continuation

How tsk's own development is tracked today, and how a session picks up work across a
session boundary. Written for someone using this, not designing it. For the design
behind it, see `docs/domain/session-continuation-design.md`; for the terms, see
`docs/domain/ubiquitous-language.md`.

## Contents

- [Two stores, not one](#two-stores-not-one)
- [Where each one lives](#where-each-one-lives)
- [The commands](#the-commands)
- [The skills](#the-skills)
- [What happens at session start](#what-happens-at-session-start)
- [A worked session](#a-worked-session)
- [Rules that matter](#rules-that-matter)

## Two stores, not one

Work on tsk involves two separate stores, in two different places. They have names:
see the Artefact and Ledger entries in `docs/domain/ubiquitous-language.md`.

| | What it holds | Where |
|---|---|---|
| **The artefacts** | What a mission builds: source, `docs/`, `ops/`, `.claude/` | `main`, in the ordinary checkout |
| **The ledger** | Mission records (tasks, decisions, questions), documents, threads, continuation state entries | A Dolt database (the ledger store), synced over the ledger ref (`refs/tsk/ledger`, or the branch `tsk/ledger`) |

They live in the same GitHub repository, `jimbarritt/tsk`, as `main` and as the ledger
ref. This repo keeps the ledger on branches, because it is used from Claude Code cloud
sessions (`.tsk/config.toml` says so). A new ledger elsewhere uses a custom ref, which
GitHub does not list as a branch. They are never checked out together. Changes to the artefacts go to `main` in the normal way. The
ledger is written through the `tsk ledger` commands.

The reason they are split: the ledger is data, not a line of development. Keeping it on
its own branch means a change to what a mission says never appears in a diff of the
artefacts, and vice versa. The artefacts are what a mission builds; the ledger is the
account of the building.

## Where each one lives

**The artefacts**: wherever you cloned the repository. Nothing unusual.

**The ledger**: the ledger store, a Dolt database (a `dolt clone` of the ledger ref) at

```
${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger-db
```

exported to every session as `$TSK_LEDGER_WT`. `<clone-id>` is minted once per
clone and stored in that clone's `.git/tsk-clone-id`, so two clones of tsk on one
machine each get their own store, and renaming a clone directory does not orphan
it. The ordinary checkout of the repository is the code worktree. The default backend
needs the `dolt` binary, version 2.5.0 or later. The `git` backend keeps the ledger as files in a
ledger worktree instead (see `docs/domain/ledger-layout.md`).

The store holds records, as tables. You read and write them with the `tsk mission`,
`tsk task`, `tsk decision` and `tsk question` commands. `tsk mission list` lists the
missions, and `tsk mission list <id> --json` prints one in full. The documents are
Markdown files on the branch `tsk/docs`. They hold free text only, such as intelligence,
research and reports, and a mission links to them with `tsk mission link`. You read and
write them with `tsk ledger doc`, not as files:

```
missions/<category>/<id>/       a mission's research, intelligence and reports
```

`tsk ledger doc get <path>` prints a document, `tsk ledger doc list [<prefix>]` lists
them, `tsk ledger doc put <path> [<file>]` writes one (stdin when no file is given), and
`tsk ledger doc mv <from> <to>` renames one and updates the mission links to it, and
`tsk ledger doc rm <path>` removes one. The last three change the local store only. Run
`tsk ledger push "<message>"` to send the change. Threads, bindings,
continuation state entries and the external event queue are tables, which the
`tsk thread` and `tsk events` commands read and write.

An earlier design kept the ledger on a `tsk/bootstrap` branch and checked it out inside
the repository's `.git/` directory; see
`docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md`. The ledger branch
replaces it.

## The commands

The `tsk` binary provides these. Use them rather than running git against
`tsk/ledger` yourself. Run from inside any working tree of the managed repo.

| Command | Does | Side effects |
|---|---|---|
| `tsk ledger path` | Prints the ledger store path | None. Safe to call any time |
| `tsk ledger fetch` | Refreshes the ledger store to origin's latest, prints the path | Updates the ledger store. Refuses if it holds uncommitted work |
| `tsk ledger doc get <path>` | Prints a document | None |
| `tsk ledger doc list [<prefix>]` | Lists document paths | None |
| `tsk ledger doc put <path> [<file>]` | Writes a document from a file or stdin | Local store only |
| `tsk ledger doc rm <path>` | Removes a document | Local store only |
| `tsk ledger doc mv <from> <to>` | Renames a document and the mission links to it; one line per change | Local store only (0.30.0) |
| `tsk mission prefix [<PREFIX>]` | Prints the mission prefix of the ledger, or sets it: 3 or 4 capital letters that `create` and `mv` add to an ID; a number at the end of an ID marks a part of a mission | Setting is local only (0.31.0) |
| `tsk mission mv <old> <new> [--dry-run] [--rewrite]` | Renames a mission: its ID changes, and every record, thread, block and parent follows; linked documents named for it are renamed; one line per change. `--rewrite` also replaces the old ID in record text | Local store only (0.30.0) |
| `tsk mission rewrite <old> <new> [--dry-run]` | Replaces the ID `<old>` with `<new>`, as a whole word, in the text of every mission: names, purposes, objectives, constraints, out-of-scope items, tasks, decisions, questions and answers. Prints one `rewrote <mission> <record>` line for each change. `<old>` can be the ID of a mission renamed earlier. Documents are not changed | Local store only (0.32.0) |
| `tsk task mv <mission> <task> <to>` | Moves a task and the decisions that name it to another mission; `--force` drops its blocks | Local store only (0.30.0) |
| `tsk decision mv <mission> <n> <to> [--task <task>]` | Moves a decision, by its place in `tsk decision list`, to another mission | Local store only (0.30.0) |
| `tsk question mv <mission> <question> <to>` | Moves a question to another mission | Local store only (0.30.0) |
| `tsk task rm <mission> <task> [--force]` | Removes a task. Refused while a block or a decision refers to it; `--force` drops the blocks and clears the task of the decisions | Local store only (0.35.0) |
| `tsk decision rm <mission> <n>` | Removes a decision, by its place in `tsk decision list <mission>`, from 1 | Local store only (0.35.0) |
| `tsk question rm <mission> <question>` | Removes a question, open or answered | Local store only (0.35.0) |
| `tsk mission rm <id> [--force]` | Removes a mission. Always refused while it has child missions. Refused while it holds tasks, decisions or questions, has threads, blocks or blockers, or links documents, unless `--force` removes the records with it, drops the blocks, unlinks the documents and detaches the threads. Documents and threads are kept | Local store only (0.35.0) |
| `tsk mission show <id>` | Prints one mission as plain text | Read only (0.34.0) |
| `tsk ledger push "<message>"` | Commits everything in the ledger store, pulls the latest ledger and pushes to `tsk/ledger` | Commits and pushes |
| `tsk thread start [<mission-id>]` | Mints and binds a thread, for a mission or with none | Writes and pushes |
| `tsk thread set mission <mission-id> [<thread-id>]` | Attaches a mission to a thread, or changes it; `--none` removes it | Local store only (0.24.0) |
| `tsk thread pause` | Appends a continuation state entry | Writes and pushes |
| `tsk thread resume` | Loads a thread's latest continuation state entry | Binds in the local store only (0.24.0) |
| `tsk thread detach` | Removes this session's binding | Local store only (0.24.0) |
| `tsk thread binding` | Prints the current binding, if any, and the actor this session or worktree is registered as | None beyond a fetch |
| `tsk thread title` | Prints the session title: `<mission ID>: <actor name>`, the name alone, or the mission ID alone. Exit 1 with neither | None. It reads the ledger store as it is and never fetches |
| `tsk actor register <name> --kind human\|robot` | Registers this session or worktree as an actor; needs no thread | Local store only (0.26.0) |
| `tsk actor name` | Prints the actor this session or worktree is registered as | None |
| `tsk actor list` | Lists the actors and the sessions and worktrees registered as each | None |
| `tsk thread guard` | Stop hook check for a binding; starts a background publish when the store holds unpushed changes | None. Never fetches |
| `tsk thread session-start` | SessionStart hook: fetches the ledger, exports `$TSK_LEDGER_WT` | Updates the ledger store |

To read the path, use `tsk ledger path`. `tsk ledger fetch` also refreshes, so calling
it merely to find out where the ledger store is counts as a write.

## The skills

A thread is an execution sequence that survives a session ending. It has its own
identity, minted once, and a session or a code worktree binds to it. See the Thread and
Actor entries in `docs/domain/ubiquitous-language.md`.

**`/start-thread <mission>`** mints a thread for a mission and binds this session or
code worktree to it. If a binding already exists, it says so and points you at
`/resume-thread` instead, because that case is a resume, not a start.

**`/pause-thread`** runs one command, `tsk thread pause --bound`, and reads three
verification lines it prints (`code: clean`, `code pushed: yes`, `ledger: pushed`). It
says the session is safe to clear only when all three pass. It writes one continuation state entry: the mission, the task in
progress, a short account of where things stand, plus the commit on `tsk/ledger`,
the commit on `main`, a timestamp, and which actor wrote it. Run it before `/clear`.
The mission, the commit hashes and the timestamp come from `tsk thread pause`; the two
judgement fields, the task and the account, come from the agent.

**`/resume-thread <thread-id>`** loads the latest continuation state entry, binds the current
session or code worktree to that thread, and reports:

```
thread id: <id>
we are working on mission <mission title>
this is where we are at: <summary of the handover note>
```

then asks whether to continue. When the session start context already shows the thread and
its latest entry, the agent writes that summary at once and runs no command until you
confirm. Take-over is additive: a different actor can bind to a
thread someone else has worked. That prints a warning naming the other actors and
proceeds, rather than refusing.

## What happens at session start

The `SessionStart` hook is `plugin/hooks/session-start.sh`, provided by the tsk plugin.
It ensures the `dolt` and `tsk` binaries are installed, then runs `tsk thread session-start`. In the
tsk repo, `ops/local/claude-session-start.sh` also runs `tsk thread session-start`,
because Claude Code reads plugin hooks only when its process starts. The first run for
a session ID and source does the work, and a second run exits with no output.
`tsk thread session-start`:

1. Fetches `tsk/ledger` and refreshes the ledger store, exporting
   `$TSK_LEDGER_WT`. The context it prints tells the agent to run `tsk mission list`, or
   shows the bound thread's mission and its open tasks. It warns when the loaded plugin is
   older than the plugin version this tsk expects.
2. Resolves the current binding, a row of the ledger's `bindings` table: the row of the
   cloud session ID (`CLAUDE_CODE_REMOTE_SESSION_ID`) if there is one, otherwise the row
   of the code worktree (`<clone-id>/<worktree-name>`). A `tsk-thread-id` marker file
   from an older tsk becomes a row the first time a command resolves it.
3. Tells the agent what to do next: run `/resume-thread <id>` when a binding is
   found, or ask which mission to work and then run `/start-thread` when none is.

The hook never creates a thread. Both branches name a command and leave the agent to
invoke it.

## A worked session

```bash
# Starting fresh. The hook has already reported no binding.
/start-thread M-BOOT-02-01
# -> started:74etfo1p

# ... work happens, code commits land on main as usual ...

# Before ending the session:
/pause-thread
# -> paused:74etfo1p
#    code: clean
#    code pushed: yes
#    ledger: pushed

/clear

# The hook on the next session finds the binding and says:
#   "An existing thread binding was found: cloud:74etfo1p. Run /resume-thread 74etfo1p next."
/resume-thread 74etfo1p
# -> thread id: 74etfo1p
#    we are working on mission Continuation harness
#    this is where we are at: ...
```

To pick up someone else's thread, name it explicitly: `/resume-thread <their-id>`. The
warning tells you who else has worked it.

## Rules that matter

- **Edit mission files inside `$TSK_LEDGER_WT`, then run
  `tsk ledger push "<message>"`.** That is the only sanctioned way to change what a mission
  says.
- **Do not run `git fetch`, `git push`, `git reset` or `git rebase` against
  `tsk/ledger` by hand**, including for a read-only check. To check the branch
  directly, spell it out: `git fetch origin refs/heads/tsk/ledger` (or
  `refs/tsk/ledger` for a custom ref), then read `FETCH_HEAD`.
- **To undo local ledger changes that are not pushed, run `tsk ledger status` to see
  them and `tsk ledger discard`.** Never `dolt reset` or `git reset` by hand.
- **A script that needs to push to `tsk/ledger` calls `tsk ledger push`**
  rather than inlining the same git sequence. One copy of that logic, in one place.
- **Code work goes to `main` directly.** No development branch, no pull request unless
  asked.
- **`/pause-thread` is run manually, by a human, in this operating context.** An
  automated trigger for it is future work and deliberately not built.
