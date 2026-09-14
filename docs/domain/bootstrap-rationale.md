# Bootstrap rationale

Status: settled. Source: 2026-09-14 bootstrap design session. Covers why the
bootstrap is shaped the way it is; the domain model itself is in
[domain-model-overview.md](domain-model-overview.md) and
[mission-model.md](mission-model.md).

## Self-hosting: the stage-zero problem

A self-hosting system needs a first version built by other means. Until tsk can
host its own development, the missions that build it run by hand, from a plain
directory, using tools other than tsk itself. That first stage is stage zero: it
exists only to build the stage that replaces it. The bootstrap's objective is
reached when tsk can host its own development, not when tsk is feature complete.
Everything after that point is tracked by tsk itself, not by the bootstrap
scaffolding.

## Gaps in the existing plan format

Source: `jimbarritt/dotfiles`,
`home/claude/skills/plan-format/PLAN-FORMAT.md`. Recorded because these gaps are
the reason tsk's task and mission fields are shaped the way they are, not because
the plan format itself is tsk's concern.

1. No acceptance criteria field: a task is a TODO marker plus a description, so
   done is a judgement, not a check. Resolved in tsk's model by the objective
   field itself, which is a checkable state on every task, not a new field.
2. No permitted-files constraint. Expected: that is ksobr domain, not the plan
   format's.
3. Blockers are a plan-level field, not a per-task one. A queue needs per-task
   dependencies to select an unblocked item.
4. Status values are `TODO`, `IN PROGRESS`, `DONE`. There is no `BLOCKED`, and
   nothing for claimed-by-an-agent.
5. Tasks have no stable identifier; numbering restarts per Delta. Concurrent
   agents need stable addressing.
6. `What's Next` holds a single pointer: a single-worker design. A parallel queue
   needs a set of claimable items.
