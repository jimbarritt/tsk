# Mission: Ad hoc tasks

| Field | Value |
|---|---|
| ID | M-ADMIN-02 |
| Territory | agentic research |
| Assignee | session agent, whichever session Jim raises the task in |
| Blocked by | none |

## Objective

- A task Jim raises mid-session, unrelated to the mission that session is working, is
  recorded on this mission's plan and executed there, rather than being folded into the
  current mission's thread or lost when the session ends.
- Each ad hoc task's status (not-started, in-progress, done) is visible in this
  briefing's plan table.

## Intelligence

- `docs/domain/mission-model.md` in the tsk repo, under mission categories: why an ad
  hoc task belongs to a mission rather than a new kind of object, and the administrative
  category this mission sits in.
- [../../index.md](../../index.md), Administrative missions: the four examples that
  motivated the category, including "a tidy up thread looking for work that needs a
  nudge", the closest of the four to this mission's shape.

## Decision authority

Yours: how to execute a task once Jim raises it, and at what level of thoroughness.

Not yours: whether a task belongs here versus an existing operational mission's plan.
If a raised task is clearly in scope for a mission already in the tree, add it to that
mission's plan instead of here, and say so rather than duplicating it silently.

## Constraints

- One ad hoc task per plan row, so it can be tracked and closed independently of any
  other.
- A task that grows an objective and a multi-step plan of its own has outgrown this
  mission. Break it out as its own mission, with its own briefing, rather than letting
  it stay here.

## Out of scope

- Capturing an idea Jim wants recorded but not acted on. That is M-ADMIN-01.
- Judging whether a raised task is worth doing. Jim raising it is the decision.
- Work already specified on an existing mission's plan.

## Plan

| ID | Task | Objective, in short | Delegated to | Blocked by | Status |
|---|---|---|---|---|---|
| T-01 | Create this mission's briefing | M-ADMIN-02 exists in `missions/administrative/`, and appears in `index.md`'s administrative missions table | none | none | DONE |
| T-02 | Strip the tsk `README.md` down to a four-step quickstart | `README.md` keeps only the strapline, the four numbered steps, and a short docs table; prerequisites, installation, upgrading and CI move to `docs/user-guide/installation.md` (new), linked from `docs/index.md` | none | none | DONE |
| T-03 | Work the `swe:feedback` / `swe:send-feedback` loop for the "load-bearing" banned-word report, surface two real gaps in the feedback tooling itself | Logged three feedback entries; `/swe:send-feedback` correctly declined to cluster a lone entry; a bundled `wrong-fix` entry was split into two on request; filed [jimbarritt/claude-plugins#4](https://github.com/jimbarritt/claude-plugins/issues/4) (single-entry threshold is the wrong design; no verdict exists for feedback about the skills themselves) and [#5](https://github.com/jimbarritt/claude-plugins/issues/5) (a target repo must be attached to the session before an issue can be filed against it, which is session-harness scoping, not a GitHub constraint) | none | none | DONE |
| T-04 | Set `$TSK_LEDGER_WT` in every session, including after `/clear` | Raised 2026-10-05: the variable was unset after `/clear`. Cause: Claude Code reads plugin hooks once, when its process starts, and the repo hook installed the tsk plugin during startup, so the plugin's `SessionStart` hook never ran in that process. Fix on `main` in `de50d9e`: `ops/local/claude-session-start.sh` builds `tsk` from `cli/` when it is not at the workspace version, then runs `tsk thread session-start` on every event. The binary claims each event by session ID and source under `<state-root>/session-start/`, so when the plugin hook also runs, the second run exits with no output. The plugin hook accepts a `tsk` at or above its required version. tsk 0.2.1, plugin 0.1.4. Confirmed: a later session started with `$TSK_LEDGER_WT` set. Follows on from M-BOOT-04 T-07, the cut-over to the binary | none | none | DONE |

## First behaviour

When Jim raises a task in a session that is not part of the mission that session is
already working, add it to the plan table above as a new task, execute it, and record
its status. Do not let it interrupt or get folded into the current mission's own plan or
thread beyond this addition.

## Report on completion

No end point. This mission reports as each ad hoc task closes, by updating that task's
row in the plan table above.
