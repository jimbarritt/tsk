# Action budget

The time each action may take. The optimisation tasks of M-PERF are measured against this table, and the experiment (M-PERF T-07) runs last, on the optimised tsk.

Rule, decided by Jim on 2026-10-18: a write never waits for origin. Acceptable time depends on the action. Marking a task as done at 3 s is unacceptable. Push less often.

Current times: 0.15.1, wall time. "Local" means a scratch copy of the real ledger with a local origin. "Live" means this repo's ledger, with origin on GitHub through the cloud proxy. A blank means not yet measured. Budgets are proposed, for Jim to change.

| Action | Command | Needs origin | Now | Budget | Where the time goes |
|---|---|---|---|---|---|
| Read what is open | `tsk mission list`, `tsk task list <id>` | no | 0.08 s live | 0.2 s | One dolt process. Met. |
| Add a task | `tsk task add` | no | 0.2 s local | 0.3 s | Two dolt processes. Near. |
| Mark a task done | `tsk task set <mission> <task> status done` | no | 0.2 s local | 0.3 s | Two dolt processes. Met for the write. |
| Record a decision or answer a question | `tsk decision add`, `tsk question answer` | no | 0.2 s local | 0.3 s | Two dolt processes. Near. |
| Mark a task done and publish it | the write, then `tsk ledger push` | yes | about 2.1 s local, live not measured | 0.3 s for the write; the publish off the critical path | Two fetches, a commit, a push, a documents fetch. |
| Publish changes | `tsk ledger push` | yes | 1.9 s with a change, 0.9 s with none, local | 1 s, or in the background | Fetch and push of both branches. |
| See what is unpublished | `tsk ledger status` | yes, it fetches | 1.75 s live | 0.3 s with a local-only mode | One fetch of each branch. |
| Start a thread | `tsk thread start` | yes | 3.2 s local | 1 s | Fetches, 13 dolt processes, a push. |
| Pause a thread | `tsk thread pause` | yes | 3.2 s local | 1 s, or in the background | The same. |
| Resume a thread | `tsk thread resume` | yes | 0.85 s local | 1 s | A fetch and a push. |
| Start a session | `tsk thread session-start` | yes | 3.5 s live | 1.5 s | A fetch of each branch and the mission list. |
| Stop hook, every turn | `tsk thread guard` | no | 0.09 s | 0.15 s | One dolt process. Met. |
| List threads | `tsk thread list` | yes, it refreshes | 3.4 s local, 4.1 s live | 0.3 s as a local read | A fetch. |

## Reading the table

- The local write is already about 0.2 s. A task marked done costs 2 s or more only when the agent publishes straight after, because a push fetches twice and pushes twice.
- Every command in the "needs origin" column pays the network. Two of them, `status` and `thread list`, need it only to compare with origin, and can read local state.
- The write-then-push rows are where the budget fails today. The task M-PERF T-08 (push less often, off the critical path) and T-01 to T-03 (fewer fetches) are what close it.

## Priority: the two latency-critical actions

Decided by Jim on 2026-10-20. Starting a new session and pausing a thread are the two most latency-sensitive actions. Both run at the edge of the agent's work, where the delay is felt. Jim's existing plan skills are slow at session start and he wants it faster.

| Action | Budget | Why it comes first |
|---|---|---|
| Start a session | 1.5 s, hook to first prompt, with the skills in place | Paid every session, before any work starts. |
| Pause a thread | 1 s | Paid at every stop. Only the first publish of work in progress pays the network. |

Every design choice in M-PERF is judged first by what it does to these two rows. M-PERF T-15 measures them on the live origin and cuts what is slow.
