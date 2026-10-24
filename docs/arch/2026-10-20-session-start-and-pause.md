# Session start and thread pause: profile and cuts

Date: 2026-10-20. Mission M-PERF, task T-15. Release 0.22.0.

Jim named two actions as the most latency-sensitive: starting a session and pausing a
thread. The budgets are 1.5 s from the hook to the first prompt for a session start, and 1 s
for a pause. On the live origin with 0.21.0 a session start took 0.33 to 0.58 s, a resume
0.39 s, a pause 2.01 s and a push with a change 1.86 s. This document profiles where the
time went, says what changed, and reports the effect through a 330 ms network shim.

## Result

Medians of 7 runs through the shim (330 ms before every `git fetch`, `git push`,
`git ls-remote` and `git clone`), on a scratch origin seeded from the live refs. The
machine ran at a load average of 9 to 10 from other work, so every figure is higher than
on a quiet machine. The pause took 0.75 s in a run at a lighter load. Counts are exact.

| Action | Before | After | `dolt` runs | Network operations |
|---|---|---|---|---|
| `thread pause` | 1.42 s | 0.88 s | 5 to 3 | 2 to 1 |
| `ledger push`, with a change | 1.24 s | 0.82 s | 4 to 3 | 2 to 1 |
| `ledger push`, nothing to send | 0.70 s | 0.27 s | 3 to 2 | 1 to 0 |
| `thread resume` | 0.61 s | 0.46 s | 2 to 2 | 1 to 1 |
| `thread session-start`, nothing new | 0.52 s | 0.39 s | 1 to 1 | 1 to 1 |
| `thread session-start`, origin moved | 1.39 s | 1.16 s | 5 to 4 | 2 to 2 |
| `thread session-start`, new clone (a cloud session) | 1.55 s | 0.88 s | 2 to 2 | 3 to 1 |
| `thread pause`, origin moved | 2.33 s | 2.41 s | 9 to 9 | 3 to 3 |

The pause is under its budget of 1 s, a push with a change is under 1 s, and a session
start is under 1.5 s on every path. The one row that did not improve is a pause when
another clone pushed since this clone last fetched. It stays at 2.3 to 2.4 s (see
"Trade-offs").

The `SessionStart` hooks, with the `claude` CLI costs replaced by stand-ins that wait as
long as the real commands did (see "The hooks"): the repo script took 9.0 s on every
start, and takes 7.0 s on the first start and 0.58 s on later starts within an hour.

## Method

- A scratch bare origin seeded with `git fetch` of `main`, `tsk/ledger` and `tsk/docs` from
  the live origin. Nothing was written to the live origin. The live ledger is already a
  Dolt file remote (11 files).
- Two clones of the scratch origin, a state directory, a config directory and a home
  directory inside the scratch root. `TSK_AUTO_UPDATE=never`. Dolt 2.5.0.
- Two wrapper programs first on `PATH`, named `git` and `dolt`. Each runs the real program,
  logs the start, the duration and the arguments, and counts the `git` runs whose verb is
  `fetch`, `push`, `ls-remote`, `clone` or `pull` as network operations. For those the
  `git` wrapper waits 330 ms first, as in `2026-10-18-remote-growth-design.md`. This
  replaces the delay of the proxy.
- Each action ran 7 times in one scratch world, in a fixed order: pause, change and push,
  push with nothing, resume, session start, session start after another clone pushed, and
  pause after another clone pushed. The new-clone start ran 5 times, each in a new clone.
- The old binary is the 0.21.0 build of the base commit. The new binary is this
  commit's build. The before and after runs were back to back.
- Output was compared between the two binaries on a script of 35 commands (fetch, list,
  start, pause with and without a task that exists, unknown thread and mission, status,
  push, resume, session start in both formats, guard, a second clone moving origin, detach).
  After masking paths, hashes, thread IDs and times, standard output, standard error and
  the exit status are the same, apart from the order of two threads with no pause that
  tie on their sort key.

## Where a pause spent its time

One run of the 0.21.0 binary, in order. Offsets and durations are in milliseconds.

| Step | Process | Time |
|---|---|---|
| Ask origin for both refs | `git ls-remote` (network) | 335 |
| Refresh: the remote check, the dirty check, the manifest, the bindings, the threads, the missions, `HEAD` | `dolt sql` | 125 |
| Refresh the documents worktree, check the code repository | 11 `git` runs | 30 |
| Write the continuation entry | `dolt sql` | 116 |
| Stage, count, list the document links, commit | `dolt sql` | 109 |
| Check the documents worktree has nothing to push | 4 `git` runs | 10 |
| The remote branch, the merge base, the remote tip, `HEAD` | `dolt sql` | 80 |
| Push to the directory of the file remote | `dolt push` | 108 |
| Commit the directory | 4 `git` runs | 12 |
| Push the directory to origin | `git push` (network) | 364 |

Three findings.

1. The network took 700 ms of 1340 ms. The shim has two operations, the question and
   the push, and a pause needs only the push. The question was there to learn whether
   origin had moved, which the push finds out for itself: a plain `git push` of a
   directory that is not a descendant of origin's commit is rejected.
2. Five `dolt` processes took 538 ms. Each costs 50 to 80 ms to start before it reads a
   table. The write, the commit and the reads of the push each started a process of
   their own, though nothing between them needed an answer from the first.
3. Everything else, 20 `git` runs, took about 60 ms. The commit of the directory, the
   documents worktree and the code repository checks do not matter.

### The candidates, checked

| Candidate | Finding |
|---|---|
| A pause pushes documents even when none changed | No. The documents branch has no change, `merge-base --is-ancestor` finds `HEAD` on the recorded tip, and nothing is sent. It costs 2 ms of `git` and no network. |
| Two pushes where one would do | No. A pause makes one network push. The data push is the only one. |
| A second fetch of tips already known | Not in a pause. In a new clone, yes: the question, a fetch of the ledger ref and a fetch of the documents ref (3 operations). Fixed. |
| The code-repository fetch that `thread` commands run | Not on this path. Since M-PERF T-03 the pause reads the remote-tracking branches first. The run above started no fetch of the code repository. |
| The number of `dolt sql` processes | Yes. 5 became 3. |
| The `git commit` and `git add` of the transport directory | 12 ms. Not a cause. |
| The retry-loop wait | Only on a retry. 50 to 250 ms. The first retry after a rejection that only means "the record was old" no longer waits. |
| Hook scripts that run several `tsk` commands | Each hook runs `tsk --version` (5 ms) and one `tsk thread session-start`. The costs are elsewhere (see "The hooks"). |

## What changed

### 1. A pause, a start and a push do not ask origin first

`LedgerStore::trust_last_fetch()` makes the store answer its question to origin from the
record of the last fetch (`remote-tips`, beside the ledger store). `thread pause`,
`thread start` and `ledger push` call it. The record needs both refs, a file remote
store, and a checkout of the remote directory; otherwise origin is asked. `thread binding`
does not call it: a binding is read from origin's state as of the command, and a test holds
that after another actor stops the thread. The pause skill reads the binding with
`--no-fetch` instead.

The push is still correct when the record is old. The data goes by a plain `git push`,
which origin rejects when it is not a descendant of origin's tip. The documents go by a
push with the recorded tip as its lease. Either rejection ends the first attempt, and the
retry fetches, merges and pushes as it did before. A rejection of the first attempt on the
record is not said aloud, because the old flow found the same fact by asking and printed
nothing. The random wait before the retry is skipped for the same reason: the wait
exists to keep two writers that collided from colliding again, and a record that was old
is not a collision.

A pause and a start check the thread, the task and the mission against what they read. If
a check fails on the record, the command refreshes with a question to origin and checks
again. A pause of a thread another clone made a minute ago, or of a task another clone
added, therefore works, and a thread or task that does not exist is reported from
origin's state. Tests cover each.

A store on the older `git-remote` layout ignores the record. Another clone may have
migrated the ref, and only a question to origin shows that (the existing migration test
covers this).

`thread resume`, `thread list`, `thread session-start`, `ledger fetch` and `ledger status`
ask origin as before.

### 2. Fewer `dolt` processes in a push

- A pause appends its continuation entry through
  `LedgerStore::continuation_append_deferred`. The store holds the statement and runs it
  first in the process that stages and commits. The write is not visible to a read before
  `commit_and_push`; a pause reads nothing after it. Ledgers below the current layout write
  at once.
- The reads that open the first attempt of the push (the remote branch, the merge base and
  `HEAD` against it, the remote tip, `HEAD`) ride in that process, when no fetch is due.
  They name the remote branch in a `where` clause on `dolt_remote_branches`, so a database
  with no remote branch answers with no rows rather than failing after the commit.
- If the batch fails, the held statement is made if the database does not hold it
  (`applied_query`), and the old one-by-one steps follow. `TSK_NO_BATCH=1` takes this path,
  and the existing "reads the same without batching" tests cover it.

### 3. A refresh reads its database while it asks origin

When the clone has a record of the last fetch, `refresh_data` asks origin on a second
thread and reads the database in the main thread at the same time. The read is the one that
the up-to-date path makes. If origin has nothing new, the read is the answer, and 100 ms of
`dolt` ran inside the 335 ms of waiting. If origin has something new, the read still gives
the dirty check, which the old flow made in a process of its own before the fetch.

After a fetch that left the database behind, the reset to the fetched state and the reads
that follow it are one `dolt sql` process (`call dolt_reset('--hard', ...)` and the
queries), not two. A failed batch resets by `dolt reset` and reads one by one.

The two queries for conflicts after a merge, and `HEAD`, are also one process.

### 4. A new clone fetches both refs in one `git fetch`

A clone with no ledger store, or no record of a fetch, has to fetch both refs whatever
origin holds. It did a question (335 ms), a fetch of the ledger ref and a fetch of the
documents ref. It now runs `git fetch --quiet --no-tags <remote> +<ledger-ref>:refs/tsk-fetch/ledger
+<docs-ref>:refs/tsk-fetch/docs`, reads the two commits with one `git rev-parse`, and
hands each to the sync that would have fetched it (once; a retry after a rejected push
fetches again). A cloud session start drops from three network operations to one.

If origin lacks either ref, the fetch fails with `couldn't find remote ref`, and the
question to origin follows, which gives the "does not exist yet" path as before. If the
fetch fails for another reason, origin is out of reach, and the old flow's failure
message follows without a second try.

The two refs under `refs/tsk-fetch/` are in the code repository's object store. They hold
the fetched commits, and the next combined fetch overwrites them.

### 5. The hooks and skills

See the next section.

## The hooks

The repo script `ops/local/claude-session-start.sh` and the plugin script
`plugin/hooks/session-start.sh` run side by side at a session start. Each starts `dolt`
three times (the version, the user name, the user email) and `tsk` for its version before the
session start proper. The repo script also runs the `claude` CLI 8 times, which
dominates.

Measured with a scratch `HOME` against the real CLI (2.1.304, through this sandbox's
proxy), warm:

| Step | Time |
|---|---|
| `claude plugin marketplace add` | 0.6 s |
| `claude plugin install` | 0.6 s |
| `claude plugin marketplace update` | 1.0 s |
| `claude plugin update` | 2.0 s |

The repo script ran all four for `swe` and again for `tsk`: 8 runs, about 8.4 s, before it
started `dolt` or `tsk`. A cold run took longer (the first add 1.5 s, the first install
2.4 s). The cost of `tsk thread session-start` itself, 0.4 to 1.4 s, was a smaller part.

| Change | Effect |
|---|---|
| One marketplace add and one refresh for the two plugins, not two of each | 8 runs become 6, 1.6 s less |
| The plugin steps run in the background while the dolt check and `tsk thread session-start` run. The script waits for them before it prints, so warnings reach the context as before. | The tsk start of 0.5 s is hidden |
| After a run with no warning, the steps are skipped for `TSK_PLUGIN_ENSURE_MINUTES` (60 unset; 0 never skips) | 8.4 s becomes 0 from the second start onward |
| `install-dolt.sh` records a passed version check in `$XDG_STATE_HOME/tsk/dolt-version-ok` and trusts it while the `dolt` binary is no newer than the file | `dolt version` (60 to 300 ms) once, not at every start in each of two hooks |
| `install-dolt.sh` reads `user.name` and `user.email` from `~/.dolt/config_global.json` as `tsk` does, and asks `dolt config` only when the file lacks them | two more `dolt` runs saved in each hook |

With stand-ins that wait as long as the real steps (and 0.45 s for `tsk`): before, 9.0 s
on every start; after, 7.0 s on the first and 0.58 s afterwards. The output is the same.

Warnings keep their wording. A failed marketplace add warns once for each plugin, as
before. A failed install skips that plugin's update, as before.

### Update, 2026-10-24: installing tsk from the release

Both hooks used to build `tsk` with `cargo install`: the repo script from the checkout
whenever the installed version differed from the workspace version, and the plugin script
from `TSK_SOURCE` or the crate. A resumed session does not pull `main`, so the repo script
built an old checkout, and a build takes 40 to 60 s. Both hooks now call
`plugin/hooks/install-tsk.sh`, which does nothing when the `tsk` first on the path is at or
above `plugin/tsk-version`, and otherwise installs the latest release binary (see
`CLAUDE.md` for the rules). `TSK_SOURCE` and the cargo branches are gone. The check costs
one `tsk --version`, 5 ms, as before.

The pause-thread skill runs `tsk thread binding --no-fetch` in place of `tsk thread
binding`, and `tsk ledger status --local` in place of `tsk ledger status`. The binding is
this session's own row, and the pause that follows checks the thread. The status asked
origin, which the push no longer does, and the skill only wanted to show what is unpushed.
The whole skill flow (`binding`, `pause`, `status`, `push`) made five network operations
and now makes one. The plugin is 0.3.13 and requires tsk 0.18.0, which introduced
`--local`.

The steps of the plugin install do not affect the current session: Claude Code reads
plugins once, at process start. A further saving would run them detached after the script
exits. That would also drop the warning from this session's context, so it was not done.

## Trade-offs and risks

- **A pause or a start can use an old view.** If another clone changed the thread, the
  mission or the tasks since this clone last fetched, the first checks run on the old view.
  A failure is rechecked on a fresh fetch, so an error never rests on the old view. A check
  that passes on the old view and would fail on a new one is not caught before the write:
  a pause of a thread another clone has stopped. The write merges at the push. It is the
  same race that existed between the question and the push, now a little wider.
- **The entry's ledger commit can be older.** A pause records `git.ledger.commit`, the
  ledger commit it was made on. On an old view that is the last commit this clone
  fetched. The entry's own commit is a descendant of it.
- **A pause after another clone pushed costs as before, not less.** The push is rejected
  (330 ms and one commit of the directory), then the retry fetches and merges. It was a
  question, a fetch and a push. The count of network operations is the same and the
  number of `dolt` runs is the same (9). The skipped wait and the single process for the
  conflict queries and `HEAD` made up for the rejected attempt; the result is within the
  noise of the old figure.
- **A push with nothing to send no longer asks origin.** "Already holds this state"
  rests on the last fetch. If origin was rewound, a push says so only after a push is
  attempted. Origin is only ever advanced by tsk.
- **A new clone leaves two refs under `refs/tsk-fetch/` in the code repository.**
- **Threads.** The refresh asks origin on a second thread while the main thread reads. Both
  only read; the thread joins before anything else runs. A panic in the asking thread is
  an unanswered question, and the old flow's failure path follows.

## What this does not cover

- The live origin. Nothing here ran against it with a write. Its latency per operation is
  higher than 330 ms for a push (GitHub's receive work), so the live pause will take longer
  than the scratch figure by about the difference. Fewer network operations help it more
  than they help the scratch.
- A pause after another clone pushed (2.4 s) and a push that meets a merge are still the
  slowest actions. The retry keeps 6 `dolt` runs (`dolt fetch`, the reads, the
  renumbering check, the merge, the conflict check with `HEAD`, `dolt push`) and 2 network
  operations after the rejected one.
- `thread start` was not timed separately. It uses the same trust and the same
  batches as a pause.
- The plugin hook script `plugin/hooks/session-start.sh` was timed with stand-ins only.
  The real `claude plugin` timings came from this sandbox's proxy and may differ from the
  cloud sessions.
- The 7-run medians carry the noise of a loaded machine: the spread of the pause was 803
  to 986 ms.

## Reproduce

The wrappers and scripts are in the session's scratch directory only. To repeat:
build a bare origin from the live refs with `git fetch` into a bare repository, clone it
twice, put wrappers named `git` and `dolt` first on `PATH` (each sleeps 330 ms before a
network verb and logs verb, arguments and duration), set `XDG_STATE_HOME`,
`XDG_CONFIG_HOME` and `HOME` inside the scratch root, run `tsk ledger fetch`,
`tsk thread start M-PERF`, then the actions in the order given in "Method".
