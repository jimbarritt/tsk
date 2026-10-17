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
| **The ledger** | Missions, briefings, threads, continuation state entries, mission reports | A Dolt database (the ledger store), synced over the `tsk/ledger` branch |

They are two branches of the same GitHub repository, `jimbarritt/tsk`. They are never
checked out together. Changes to the artefacts go to `main` in the normal way. The
ledger is written through the `tsk ledger` commands.

The reason they are split: the ledger is data, not a line of development. Keeping it on
its own branch means a change to what a mission says never appears in a diff of the
artefacts, and vice versa. The artefacts are what a mission builds; the ledger is the
account of the building.

## Where each one lives

**The artefacts**: wherever you cloned the repository. Nothing unusual.

**The ledger**: the ledger store, a Dolt database (a `dolt clone` of the `tsk/ledger`
branch) at

```
${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger-db
```

exported to every session as `$TSK_LEDGER_WT`. `<clone-id>` is minted once per
clone and stored in that clone's `.git/tsk-clone-id`, so two clones of tsk on one
machine each get their own store, and renaming a clone directory does not orphan
it. The ordinary checkout of the repository is the code worktree. The default backend
needs the `dolt` binary, version 1.88.2 or later. The `git` backend keeps the ledger as files in a
ledger worktree instead (see `docs/domain/ledger-layout.md`).

The store holds documents and tables. You read and write the documents with
`tsk ledger doc`, not as files:

```
index.md                        current state, the mission tree, next step
missions/                       one briefing per mission
missions/<id>/                  a mission's report, once it starts executing
future-missions-tbd.md          ideas not yet shaped into briefings
```

`tsk ledger doc get <path>` prints a document, `tsk ledger doc list [<prefix>]` lists
them, `tsk ledger doc put <path> [<file>]` writes one (stdin when no file is given), and
`tsk ledger doc rm <path>` removes one. The last two change the local store only. Run
`tsk ledger push "<message>"` to send the change. Threads, cloud session bindings,
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
| `tsk ledger push "<message>"` | Commits everything in the ledger store, pulls the latest ledger and pushes to `tsk/ledger` | Commits and pushes |
| `tsk ledger migrate-to-dolt` | One-off: moves a file ledger into a Dolt database | Pushes `archive/tsk-ledger-files` and the ledger branch |
| `tsk thread start [<mission-id>]` | Mints and binds a thread, for a mission or with none | Writes and pushes |
| `tsk thread set mission <mission-id> [<thread-id>]` | Attaches a mission to a thread, or changes it; `--none` removes it | Writes and pushes |
| `tsk thread pause` | Appends a continuation state entry | Writes and pushes |
| `tsk thread resume` | Loads a thread's latest continuation state entry | Binds, and pushes if the binding changed |
| `tsk thread binding` | Prints the current binding, if any | None beyond a fetch |
| `tsk thread guard` | Stop hook check for a binding | None. Never fetches |
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

**`/pause-thread`** writes one continuation state entry: the mission, the task in
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

then asks whether to continue. Take-over is additive: a different actor can bind to a
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
   `$TSK_LEDGER_WT`. The context it prints tells the agent to read the index with
   `tsk ledger doc get index.md`.
2. Resolves the current binding: the cloud session ID
   (`CLAUDE_CODE_REMOTE_SESSION_ID`, looked up in
   `threads/lookup-by-cloud-session.json`) if there is one, otherwise the
   `tsk-thread-id` marker file in the code worktree's own git metadata.
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
  directly, spell it out: `git fetch origin refs/heads/tsk/ledger`, then read
  `FETCH_HEAD`.
- **To undo local ledger changes that are not pushed, run `tsk ledger status` to see
  them and `tsk ledger discard`.** Never `dolt reset` or `git reset` by hand.
- **A script that needs to push to `tsk/ledger` calls `tsk ledger push`**
  rather than inlining the same git sequence. One copy of that logic, in one place.
- **Code work goes to `main` directly.** No development branch, no pull request unless
  asked.
- **`/pause-thread` is run manually, by a human, in this operating context.** An
  automated trigger for it is future work and deliberately not built.
