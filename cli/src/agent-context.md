# tsk: agent context

tsk tracks missions and the threads that work them. The missions, threads and their
continuation state live in a **ledger**: a git branch, `refs/heads/tsk/ledger` on the
managed repo's `origin`, checked out in a detached ledger worktree outside the
repository. The `ledger` and `thread` commands run in the `tsk` client alone, with no
daemon, from inside any working tree of the managed repo.

Every ledger file and format is in `docs/domain/ledger-layout.md`. Threads, bindings and
continuation state are in `docs/domain/session-continuation-design.md`.

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

tsk events append <source> <event-type> <action> <repo> <payload-file>   queue one event envelope, push
tsk events append-batch [--source <s>] [--action <a>]                    queue every NDJSON event line on stdin, push once
tsk events read-new                                                       print the events past the watermark; no writes
tsk events advance-watermark <count>                                      set the watermark to <count>, push
tsk                                                                   launch the TUI (reads threads from tskd)
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
| `thread pause` | `paused:<thread-id>` | exit 1 when the managed repo's `HEAD` is not on origin's default branch |
| `thread resume` | `{"thread_id":"...","latest":{...},"warning":"..."}` | |
| `thread detach` | `detached:<thread-id>` | exit 1, nothing on stdout, with no binding |
| `thread stop` | `stopped:<thread-id>` | exit 1 with no binding and no thread ID given |
| `thread list` | one compact JSON object per line: `id`, `mission_link`, `latest_whats_next`, `latest_timestamp` | |
| `thread binding` | `cloud:<thread-id>` or `worktree:<thread-id>` | exit 1, nothing on stdout, with no binding |
| `thread guard` | nothing when bound; `{"decision":"block","reason":"..."}` when not | |

In `thread resume`, `latest` is the thread's last continuation state entry as stored, or
`{}` with no entry. `warning` is empty unless the thread's entries name a `written_by`
actor and none of them is the current actor.

A continuation state entry holds `mission_link`, `task_id`, `whats_next`,
`commit_on_ledger`, `commit_on_main`, `timestamp` and `written_by`. Entries written by the
`tsk/bootstrap` scripts hold `commit_on_bootstrap` in place of `commit_on_ledger`. tsk
reads both and writes `commit_on_ledger` only.

`thread pause` records the commit the ledger worktree is at before the entry is appended,
and the managed repo's `HEAD`. It refuses a `HEAD` that origin's default branch does not hold,
because an actor resuming from another clone cannot see it. `commit_on_main` holds
that commit whatever the default branch is called.

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

### Switching threads

No single command switches threads. To switch: `tsk thread binding` to find the current
thread, `tsk thread detach`, optionally `tsk thread stop <old-thread-id>`, then
`tsk thread resume <new-thread-id>`. `tsk thread list` gives the candidates.

## TUI

`tsk` with no arguments launches the TUI. It reads threads and tasks from the `tskd`
daemon over its Unix socket, `~/.tsk/tskd.sock`, so `tskd` must be running. The daemon
and its thread model are separate from the ledger threads above.

The TUI has two panes:

**Threads pane** (default): shows all threads grouped by state and priority. Navigate
with `j`/`k` to move a selection cursor. Press `Enter` to view tasks for the selected
thread. Type `gt` (two-key sequence) to view tasks for the active thread.

**Tasks pane**: shows tasks for a specific thread. At the top is a thread summary box
(id, slug, priority). Below is the task list sorted by state priority:
1. `▶` in-progress
2. `⏳` blocked
3. `○` not-started
4. `✓` done (greyed out)
5. `✗` cancelled (greyed out)

Press `Esc` or `Ctrl-O` to return to the threads pane. Press `q` to quit.
