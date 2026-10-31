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
- **Agents.** Three runs per arm that count, Sonnet 5.5, all started together, each in its own copy.
  They read their instructions from a file, which costs one tool call in each arm.
- **Scoring.** A checker clones each run's origin afresh and compares it with the start.
  It counts the four writes, any other change, and the two lists against the true sets.
- **Not tested.** The live origin (the experiment wrote to scratch copies only), a ledger
  of a larger size, and two actors writing at the same time.

## Results

| Run | Tokens | Tool calls | Time | Writes landed | Other changes | Open tasks | Blocked list |
|---|---|---|---|---|---|---|---|
| tsk-4 | 61,299 | 7 | 25.5 s | 4 of 4 | 0 | right | right |
| tsk-5 | 62,736 | 6 | 26.2 s | 4 of 4 | 0 | right | right |
| tsk-6 | 61,288 | 7 | 31.1 s | 4 of 4 | 0 | right | right |
| md-1 | 73,659 | 9 | 131.8 s | 4 of 4 | 0 | right | right |
| md-2 | 76,576 | 9 | 46.6 s | 4 of 4 | 0 | right | right |
| md-3 | 73,713 | 9 | 69.9 s | 0 of 4 published | 0 | right | right |

| | `tsk` (mean) | Markdown (mean) | Difference |
|---|---|---|---|
| Tokens | 61,800 | 74,600 | 17 percent fewer |
| Tool calls | 6.7 | 9 | 26 percent fewer |
| Time | 28 s | 83 s (median 70 s) | about a third of the time |

The token figures include what every agent loads before it starts (the system prompt and
the repo's `CLAUDE.md`). That part is the same in both arms, so the difference between the
arms is a larger share of the work itself than the totals show.

## Findings

1. **`tsk` is cheaper and faster.** About 17 percent fewer tokens, about a quarter fewer
   tool calls, and about a third of the time. The Markdown runs read whole mission files
   and then computed the answers with a script. The `tsk` runs asked a command, and read
   the derived `blocked` flag that `tsk mission list <id> --json` prints.
2. **Both arms were correct.** Every run that published landed all four changes, with no
   other change anywhere in the ledger, and both lists (open tasks and blocked items) were
   exactly right. The Markdown arm was right because its README states the rule for
   "blocked" and the agents checked the statuses. The `tsk` arm was right because the
   command derives it.
3. **A permission check blocked a git push.** In md-3 the permission layer refused the
   `git push` to the scratch origin, so the run ended with its edits unpublished. md-1 and
   md-2 were warned but went through. No `tsk ledger push` was refused. This depends on the
   sandbox settings and not on the approach, but it was a real cost of the Markdown route.
4. **The cost of the Markdown route grows with the ledger.** Each Markdown run read three
   whole mission files and searched all 24. Here that is small. The `tsk` queries read what
   they need. This was not measured.

## A mistake in the first batch

The first three `tsk` runs (tsk-1 to tsk-3) each listed 7 missions as blocked in error. I
first read this as a defect of `tsk`. It was an error in my own seed script, which created
the missions in the scratch ledgers but did not set their statuses. Every mission was then
"todo", so missions whose blockers were really done looked blocked. The Markdown seed set
the statuses correctly. My start-state check compared tasks, decisions and questions but
not mission statuses, so it did not catch the gap. I fixed the seed, made the checker
compare mission statuses and the blocked set, confirmed that the scratch ledger has the
same 12 blocked items as the live one, and ran three new `tsk` runs (tsk-4 to tsk-6). The
table above uses those. The first batch used 61,100 to 67,300 tokens, 7 tool calls and 24
to 32 s, which is close to the corrected runs. The list columns of `tsk` already show only
open blockers, so no change to `tsk` follows.

## Limits

- The experiment wrote to scratch copies only. It did not use the live origin.
- The ledger is small (24 missions, 118 tasks). A larger ledger would widen the gap.
- Three runs per arm is a small sample, and the Markdown times vary a lot (47 to 132 s).
- Two actors writing at the same time were not tested.
- The agents loaded the repo's `CLAUDE.md`, which says to use `tsk`. The Markdown agents
  were told to ignore it.
