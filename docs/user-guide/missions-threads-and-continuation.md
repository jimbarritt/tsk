# Missions, threads and continuation

How tsk's own development is tracked today, and how a session picks up work across a
session boundary. Written for someone using this, not designing it. For the design
behind it, see `docs/domain/session-continuation-design.md`; for the terms, see
`docs/domain/ubiquitous-language.md`.

## Contents

- [Two stores, not one](#two-stores-not-one)
- [Where each one lives](#where-each-one-lives)
- [The scripts](#the-scripts)
- [The three commands](#the-three-commands)
- [What happens at session start](#what-happens-at-session-start)
- [A worked session](#a-worked-session)
- [Rules that matter](#rules-that-matter)

## Two stores, not one

Work on tsk involves two separate stores, in two different places.

| | What it holds | Where |
|---|---|---|
| The repository's own code and docs | Source, `docs/`, `ops/`, `.claude/` | `main`, in the ordinary checkout |
| tsk's mission and task data | Missions, briefings, threads, continuation events | The `tsk/bootstrap` branch, checked out elsewhere |

They are two branches of the same GitHub repository, `jimbarritt/tsk`. They are never
checked out together. Code changes go to `main` in the normal way. Mission and task
state goes to `tsk/bootstrap` through its own scripts.

The reason they are split: mission data is data, not a line of development. Keeping it
on its own branch means a change to what a mission says never appears in a code diff,
and vice versa.

## Where each one lives

**The code**: wherever you cloned the repository. Nothing unusual.

**The mission data**: a linked git worktree at

```
${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/bootstrap
```

exported to every session as `$TSK_BOOTSTRAP_WT`. `<clone-id>` is minted once per
clone and stored in that clone's `.git/tsk-clone-id`, so two clones of tsk on one
machine each get their own checkout, and renaming a clone directory does not orphan
it.

Inside that worktree:

```
index.md                        current state, the mission tree, next step
missions/                       one briefing per mission
missions/<id>/                  a mission's report, once it starts executing
future-missions-tbd.md          ideas not yet shaped into briefings
threads/lookup-by-cloud-session.json    cloud session ID -> thread ID
threads/<thread-id>/index.md            a pointer to the mission being worked
threads/<thread-id>/continuation-state.jsonl   the thread's continuation events
```

This checkout used to sit inside the repository's `.git/` directory. It moved out on
2026-09-17; see `docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md`. An
existing clone migrates itself on the next fetch.

## The scripts

All in `ops/local/`. Use these rather than running git against `tsk/bootstrap`
yourself. There is a name collision on the remote that makes an unqualified
`git fetch origin tsk/bootstrap` return stale content with no error, and these
scripts spell the ref out in full.

| Script | Does | Side effects |
|---|---|---|
| `bootstrap-wt-path.sh` | Prints the worktree path | None. Safe to call any time |
| `fetch-bootstrap-ref.sh` | Refreshes the worktree to origin's latest, prints the path | Resets the worktree. Refuses if it holds uncommitted work |
| `push-bootstrap-ref.sh "<message>"` | Commits everything in the worktree and pushes to `tsk/bootstrap` | Commits and pushes |
| `thread-start.sh` | Mints and binds a thread | Writes and pushes |
| `thread-append-handover.sh` | Appends a continuation event | Writes and pushes |
| `thread-resume.sh` | Loads a thread's latest continuation event | Binds, and pushes if the binding changed |
| `thread-resolve-binding.sh` | Prints the current binding, if any | None beyond a fetch |

To read the path, use `bootstrap-wt-path.sh`. `fetch-bootstrap-ref.sh` also refreshes,
so calling it merely to find out where the worktree is counts as a write.

## The three commands

A thread is an execution sequence that survives a session ending. It has its own
identity, minted once, and a session or a worktree binds to it. See the Thread and
Actor entries in `docs/domain/ubiquitous-language.md`.

**`/start-thread <mission>`** mints a thread for a mission and binds this session or
worktree to it. If a binding already exists, it says so and points you at
`/resume-thread` instead, because that case is a resume, not a start.

**`/pause-thread`** writes one continuation event: the mission briefing, the task in
progress, a short account of where things stand, plus the commit on `tsk/bootstrap`,
the commit on `main`, a timestamp, and which actor wrote it. Run it before `/clear`.
The commit hashes and timestamp come from the script; the three judgement fields come
from the agent.

**`/resume-thread <thread-id>`** loads the latest continuation event, binds the current
session or worktree to that thread, and reports:

```
thread id: <id>
we are working on mission <mission title>
this is where we are at: <summary of the handover note>
```

then asks whether to continue. Take-over is additive: a different actor can bind to a
thread someone else has worked. That prints a warning naming the other actors and
proceeds, rather than refusing.

## What happens at session start

The `SessionStart` hook runs `.claude/hooks/session-start.sh` and:

1. Fetches `tsk/bootstrap` and materialises the worktree, exporting
   `$TSK_BOOTSTRAP_WT`.
2. Resolves the current binding: the cloud session ID
   (`CLAUDE_CODE_REMOTE_SESSION_ID`, looked up in
   `threads/lookup-by-cloud-session.json`) if there is one, otherwise the
   `tsk-thread-id` marker file in the worktree's own git metadata.
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

- **Edit mission files inside `$TSK_BOOTSTRAP_WT`, then run
  `push-bootstrap-ref.sh`.** That is the only sanctioned way to change what a mission
  says.
- **Do not run `git fetch`, `git push`, `git reset` or `git rebase` against
  `tsk/bootstrap` by hand**, including for a read-only check. An unqualified
  `tsk/bootstrap` resolves to an orphaned custom ref left over from an earlier design
  and returns content frozen at 2026-09-14, exiting zero with no error. To check the
  branch directly, spell it out: `git fetch origin refs/heads/tsk/bootstrap`, then read
  `FETCH_HEAD`.
- **A script that needs to push to `tsk/bootstrap` calls `push-bootstrap-ref.sh`**
  rather than inlining the same git sequence. One copy of that logic, in one place.
- **Code work goes to `main` directly.** No development branch, no pull request unless
  asked.
- **`/pause-thread` is run manually, by a human, in this operating context.** An
  automated trigger for it is future work and deliberately not built.
