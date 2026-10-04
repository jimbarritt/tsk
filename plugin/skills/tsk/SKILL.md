---
name: tsk
description: Reference for the tsk command line: the ledger, threads and bindings, continuation state, and the external event queue. Load when working with tsk missions, threads or the ledger, or when a tsk command is needed.
---

# tsk: agent context

tsk tracks missions and the threads that work them. The missions, threads and their
continuation state live in a **ledger**: a git branch, `refs/heads/tsk/ledger` on the
managed repo's `origin`, checked out in a detached ledger worktree outside the
repository. The `ledger`, `thread` and `events` commands run in the `tsk` binary alone, with no
daemon, from inside any working tree of the managed repo.

Design references in the tsk repository: every ledger file and format is in
`docs/domain/ledger-layout.md`. Threads, bindings and continuation state are in
`docs/domain/session-continuation-design.md`.

## Core concepts

A **thread** is one line of work on a mission. Its ID is 8 characters from `[0-9a-z]`.
Each thread has a directory on the ledger, `threads/<thread-id>/`, that holds `index.md`
(a link to the mission briefing) and `continuation-state.jsonl` (one continuation state
entry per pause).

A **binding** ties the current session or code worktree to a thread:

- In a cloud session (`CLAUDE_CODE_REMOTE_SESSION_ID` set), the binding is an entry in
  `threads/lookup-by-cloud-session.json` on the ledger.
- Otherwise, the binding is the file `tsk-thread-id` in the code worktree's own git
  directory (`git rev-parse --git-dir`). It is never pushed.

More than one session or code worktree can bind to the same thread. Resuming a thread that
another actor wrote to binds anyway, and prints a warning.

## Commands

```
tsk ledger fetch                         fetch refs/heads/tsk/ledger from origin, refresh the ledger worktree, print the path
tsk ledger path                          print the ledger worktree path; no fetch, no writes
tsk ledger push "<message>"              commit every ledger worktree change, rebase onto the latest ledger, push; prints the ledger commit

tsk thread start <mission-id> <briefing-path>                         mint, scaffold and bind a thread, push
tsk thread pause <thread-id> <mission-link> <task-id> <whats-next>    append a continuation state entry, push
tsk thread resume <thread-id>                                         bind to a thread, print its latest entry
tsk thread detach                                                     remove this session's or code worktree's binding
tsk thread stop [<thread-id>]                                         delete a thread and every cloud binding to it, push
tsk thread list                                                       list threads, most recently paused first
tsk thread binding [--no-fetch]                                       print the current binding
tsk thread guard                                                      Stop hook check for a binding; never fetches
tsk thread session-start                                              SessionStart hook: fetch the ledger, export TSK_LEDGER_WT, print the hook output JSON

tsk events append <source> <event-type> <action> <repo> <payload-file>   queue one event envelope, push
tsk events append-batch [--source <s>] [--action <a>]                    queue every NDJSON event line on stdin, push once
tsk events read-new                                                       print the events past the watermark; no writes
tsk events advance-watermark <count>                                      set the watermark to <count>, push
```

`<briefing-path>` and `<mission-link>` are paths relative to the ledger root, for example
`missions/operational/M-BOOT-04-official-ledger.md`. `tsk thread start` refuses a path
with no file at it in the ledger worktree.

### Ledger

`tsk ledger fetch` checks the ledger's `.tsk-ledger.toml` version before it changes the
ledger worktree, and refuses to reset over uncommitted changes in it. Both `fetch` and
`path` print a bare path on stdout, so `WT="$(tsk ledger fetch)"` works. To change the
ledger, edit files in the ledger worktree, then run `tsk ledger push "<message>"`: it
commits everything, rebases onto the latest `refs/heads/tsk/ledger`, and pushes with a
compare and swap, retrying up to 5 times when another writer pushes first. With nothing to
commit it still pushes a commit an earlier run left unpushed. A rebase conflict stops it
with the rebase aborted.

### Threads

Every `thread` command that reads the ledger fetches it first, except `binding
--no-fetch` and `guard`. Every command that writes to the ledger pushes once,
after all its file changes, through the same code as `tsk ledger push`. Diagnostics go to
stderr.

| Command | Stdout on success | Other exits |
|---|---|---|
| `thread start` | `started:<thread-id>` | `resume-required:<thread-id>` and exit status 2 when a binding exists |
| `thread pause` | `paused:<thread-id>` | exit 1 when the managed repo's `HEAD` is not on any branch on origin |
| `thread resume` | `{"thread_id":"...","latest":{...},"warning":"..."}` | |
| `thread detach` | `detached:<thread-id>` | exit 1, nothing on stdout, with no binding |
| `thread stop` | `stopped:<thread-id>` | exit 1 with no binding and no thread ID given |
| `thread list` | one compact JSON object per line: `id`, `mission_link`, `latest_whats_next`, `latest_timestamp` | |
| `thread binding` | `cloud:<thread-id>` or `worktree:<thread-id>` | exit 1, nothing on stdout, with no binding |
| `thread guard` | nothing when bound; `{"decision":"block","reason":"..."}` when not | |

In `thread resume`, `latest` is the thread's last continuation state entry as stored, or
`{}` with no entry. `warning` is empty unless the thread's entries name a `written_by`
actor and none of them is the current actor.

A continuation state entry holds `mission_link`, `task_id`, `whats_next`, a nested `git`
object, `timestamp` and `written_by`. The `git` object holds `ledger.commit`, `code.ref`
and `code.commit`.

`thread pause` records the commit the ledger worktree is at before the entry is appended,
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
`tsk thread resume <new-thread-id>`. `tsk thread list` gives the candidates.
