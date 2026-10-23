# Experiment: hand-edited Markdown against `tsk` on Dolt

Date: 2026-10-23. M-PERF T-07. The question: is an agent faster, cheaper and as correct
with the `tsk` commands on Dolt as with Markdown files that it edits by hand?

## Method

- **Data.** Both arms start from the same data, taken from this repo's ledger on
  2026-10-23: 24 missions, 118 tasks, 110 decisions and 19 questions. The `tsk` arm is a
  scratch ledger replayed through the commands, with its own local origin. The Markdown arm
  is one file per mission plus `index.md` in a scratch git repo with its own origin. A
  checker confirmed that both starts equal the live data, with no difference.
- **Jobs.** Each run did the same seven jobs, in the same words: list the open tasks of
  M-PERF and M-ENV; list what is blocked; set a task to done; add a task; record a
  decision; answer a question; publish.
- **Guidance.** The `tsk` arm got a ten-line cheat sheet. The Markdown arm got a README of
  the same size that describes the format and the rule for "blocked".
- **Agents.** Three runs per arm, Sonnet 5.5, all started together, each in its own copy.
  They read their instructions from a file, which costs one tool call in each arm.
- **Scoring.** A checker clones each run's origin afresh and compares it with the start.
  It counts the four writes, any other change, and the two lists against the true sets.
- **Not tested.** The live origin (the experiment wrote to scratch copies only), a ledger
  of a larger size, and two actors writing at the same time.

## Results

| Run | Tokens | Tool calls | Time | Writes landed | Other changes | Open tasks | Blocked list |
|---|---|---|---|---|---|---|---|
| tsk-1 | 67,263 | 7 | 31.7 s | 4 of 4 | 0 | right | 7 extra |
| tsk-2 | 61,138 | 7 | 25.1 s | 4 of 4 | 0 | right | 7 extra |
| tsk-3 | 61,845 | 7 | 24.4 s | 4 of 4 | 0 | right | 7 extra |
| md-1 | 73,659 | 9 | 131.8 s | 4 of 4 | 0 | right | right |
| md-2 | 76,576 | 9 | 46.6 s | 4 of 4 | 0 | right | right |
| md-3 | 73,713 | 9 | 69.9 s | 0 of 4 published | 0 | right | right |

| | `tsk` (mean) | Markdown (mean) |
|---|---|---|
| Tokens | 63,400 | 74,600 |
| Tool calls | 7 | 9 |
| Time | 27 s | 83 s (median 70 s) |

The token figures include what every agent loads before it starts (the system prompt and
the repo's `CLAUDE.md`). That part is the same in both arms, so the difference between the
arms is a larger share of the work itself than the totals show.

## Findings

1. **`tsk` is cheaper and faster.** About 15 percent fewer tokens, 2 fewer tool calls out
   of 9, and about a third of the time. The Markdown runs read whole mission files and then
   computed the answers with a script. The `tsk` runs asked a command.
2. **The writes were correct in both arms.** Every run that published landed all four
   changes, with no other change anywhere in the ledger.
3. **The `tsk` runs got "blocked" wrong, all three the same way.** Each listed 19 items
   where 12 are blocked. The 7 extras are missions that have a blocker which is already
   done: M-BOOT-02, M-BOOT-02-01, M-BOOT-02-02, M-BOOT-03, M-BOOT-04, M-BOOT-07 and
   M-NAMES. The `blocked-by` column of `tsk mission list` and `tsk task list` shows every
   blocker, done or not. An agent that reads the column takes any entry as "blocked". The
   `--json` output has the derived `blocked` flag and a `blocking` flag on each blocker, and
   gives the right answer. The Markdown arm was right because its README states the rule
   and the agents checked the statuses.
4. **A permission check blocked a git push.** In md-3 the permission layer refused the
   `git push` to the scratch origin, so the run ended with its edits unpublished. md-1 and
   md-2 were warned but went through. No `tsk ledger push` was refused. This depends on the
   sandbox settings and not on the approach, but it was a real cost of the Markdown route.
5. **The cost of the Markdown route grows with the ledger.** Each Markdown run read three
   whole mission files and searched all 24. Here that is small. The `tsk` queries read what
   they need and do not grow in the same way. This was not measured.

## What follows

- A task to make the `blocked-by` display show only blockers that are not done (or add a
  `blocked` column), since agents read the column as "blocked".
- The result supports the case for the model: for these operations the commands are
  cheaper, faster and as safe. The case would be stronger with a larger ledger and with
  two actors writing at once.
