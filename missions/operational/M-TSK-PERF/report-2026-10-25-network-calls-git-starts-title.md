# M-TSK-PERF report, 2026-10-25: network calls, git starts and the session title

Written by the robot Still Here After The Compaction. Release 0.33.0 (plugin 0.3.29) carries the changes.

## T-18: an unpublished ledger

Measured in the cloud container, with a bare local origin that holds no ledger. `tsk ledger status` started these network operations:

1. `git fetch` of the ledger ref and the documents ref together. It fails because the refs are absent.
2. `git ls-remote` of both refs. It says which are absent.
3. `git fetch` of the ledger ref again, from the data sync.
4. `git ls-remote --exit-code` of the ledger ref, to tell an absent ref from a failed fetch.

Calls 3 and 4 repeat the answer of call 2. Against GitHub each call is a round trip. That fits the 4 to 6 s that Jim measured on a laptop.

Change: `DoltStore::ledger_tip_fetched` and `docs_tip_fetched` return "absent" when origin itself named no commit for the ref in this process, and no push has been rejected since. The status now makes two network calls. A conformance test counts them.

Not done: one call. A single `ls-remote` first would make a clone that has no record of a fetch pay two calls (the question, then the fetch), and every new cloud session is such a clone. The two calls stay. A cache of "origin has no ledger" would go stale when another clone publishes, so it was not built.

## T-20: git starts per command

Counted with `strace -f -e trace=execve` (git starts per command, before and after):

| Command | Before | After |
| --- | --- | --- |
| `tsk thread title` | 3 | 1 |
| `tsk thread list` | 9 | 7 |
| `tsk ledger status` | 7 | 6 |

Changes:

- `Repo::discover` starts one `git rev-parse` for the common directory and the git directory, and a process asks once for each directory. The session code asked for the git directory in a second start.
- `ensure_worktree_of_repo` checks a worktree once per process.
- `worktree_side` runs `git rev-list --left-right --count` first, and `git merge-base` only when the documents worktree and origin diverge. When one holds all of the other, the base is `HEAD` or the tip.

The six starts of `ledger status --local` in this repo are one `rev-parse`, a `cat-file` check of the recorded tip, and then `rev-list`, `diff`, `ls-files` and `merge-base`. The four after the `rev-parse` compare the documents worktree with the tip, and each reads a different fact. Fewer starts would need a library in place of the `git` binary or a single `git status` that compares against the tip. Neither was built.

## T-21: the 24 s thread pause

`tsk thread pause` timed in a scratch repo with a local origin (release build, cloud container):

| Dolt server | Wall time (3 runs) |
| --- | --- |
| on | 0.238 to 0.245 s |
| off | 0.600 to 0.628 s |

A pause makes one network operation, the push, and no fetch or `ls-remote` (the record of the last fetch stands in for the question). It starts git 24 times. At 40 to 60 ms a start on the laptop, the starts cost 1 to 1.5 s there, and the push adds one round trip to GitHub. The budget is 2 s. The pause skill runs one command (`tsk thread pause --bound`), so the 24 s that Jim saw is mostly the agent's own turns, which agrees with the hypothesis in the task and with T-22.

Not measured: the pause against GitHub from the laptop. Jim sends those timings after `brew upgrade` to 0.33.0.

## T-23: the session title in one call

The resume skill took three calls: `tsk thread title`, `get_session`, `set_session_title`. The cloud session ID in `CLAUDE_CODE_REMOTE_SESSION_ID` is `cse_<id>`, and the tool takes `session_<id>`. The suffix is the same (checked against `get_session` for this session and against the parent session ID in an earlier session).

Change: when a cloud session has a binding and a title can be named, the session start context ends with `Session title: call set_session_title once, with session_id session_<id> and title "<title>".` The resume skill calls the tool once with those values, and falls back to the three calls when the line is absent (an actor registered after the session start, or an older `tsk`).

Limit: the title needs the actor to exist at the session start. A robot that registers later still takes the fallback.

## Observations outside the tasks

- A background `strace -f` of a `tsk` command that starts the Dolt server does not return, because the server child outlives the command. Use a logging `git` shim on `PATH` to count git starts instead.
- `tsk thread session-start` reads the hook input on stdin. A run from a terminal without `</dev/null` waits.
