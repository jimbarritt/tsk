# Mission: Operation

| Field | Value |
|---|---|
| ID | M-BOOT-03 |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | M-BOOT-02 |

## Objective

Kind: attainable

- A consumption figure is recorded for each of three manually executed briefings.
- A cadence for the routine is calculated from those figures, and it keeps projected
  weekly consumption under the weekly cap with headroom for review sessions.
- A cloud routine exists at that cadence.
- One completed unattended run has produced a pull request, a run record, and thread
  state, with no human intervention.
- A second unattended run resumes the first run's thread from that state rather than
  starting the mission over.

## Purpose

Parent: M-BOOT, bootstrap tsk self hosting. Delegated missions cannot be handed out
until runs are known to work and their cost is known.

## Intelligence

- `docs/` in the tsk repo
- `docs/domain/mission-briefing-template.md` (in the tsk repo), including the worked example for the first delegated
  briefing
- Claude Pro has a rolling five hour session window and a weekly cap that resets at a
  fixed time for the account
- Claude Code, Claude Desktop and claude.ai share one allowance
- The minimum cloud routine interval is one hour

## Decision authority

Jim decides the cadence and the attempt limit.

## Constraints

- One mission per run. No more. A mission may take several runs; thread state is what
  makes the next run continue rather than restart.
- Every delegated mission holds a bounded attempt limit. On exceeding it, the run
  marks the mission blocked and stops.
- Execution runs on Sonnet 5. Review runs on Opus 5.

## Out of scope

- Building the official data ref. That is M-BOOT-04.
- Increasing the subscription plan.

## Tasks

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Write three briefings by hand | Three briefings in the repository, subjects in order: push a single actor's event log to a data ref, fetch and read the data ref, add the manifest | none | TODO |
| T-02 | Execute each briefing manually on Sonnet 5 | Each executed in one Claude Code thread, with a run record | T-01 | TODO |
| T-03 | Correct the briefing format from the run records | Every defect found in a run record is fixed in the template or in `docs/`, not in the code | T-02 | TODO |
| T-04 | Record consumption per run | A figure from Settings then Usage for each run | T-02 | TODO |
| T-05 | Calculate the cadence | Projected weekly consumption under the weekly cap, with headroom for review sessions | T-04 | TODO |
| T-06 | Create the cloud routine | Routine exists on Sonnet 5 at the calculated cadence | T-05 | TODO |
| T-07 | Define the run loop | Pull, read the queue, take the first unblocked mission or resume an open thread, implement, run tests, push a branch, open a pull request, update status, write the run record and the thread state | T-06 | TODO |
| T-08 | Prove one unattended run | One run produces a pull request, a run record, and thread state, with no human intervention | T-07 | TODO |
| T-09 | Prove resumption | A second run continues the first run's thread from its state rather than restarting the mission | T-08 | TODO |
| T-10 | Review each run | Maintained: every pull request merged or returned within one day, every blocked mission decided | T-08 | Maintained |

**Essential task**: T-03. The purpose of the manual runs is to find defects in the
briefing format, not to ship the storage layer.

**Maintained task**: T-10 is a condition held true while the bootstrap runs, not a step
toward the objective.

## Open decisions

- Whether "Jim with Opus 5" counts as a different actor from Jim for the delegation
  test. The answer determines whether T-02 runs are delegated missions or Jim's own
  tasks.
