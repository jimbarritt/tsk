# Mission: Thread binding resilience

| Field | Value |
|---|---|
| ID | M-BOOT-02-02 |
| Territory | agentic research |
| Assignee | unassigned |
| Blocked by | none |

## Scope

Supervised interactive only, same boundary as M-BOOT-02-01. A human is present. This
mission does not build or touch any unattended trigger; that is M-BOOT-03's territory.

## Objective

- After every `SessionStart` firing, a fresh session or a `/clear` inside an existing
  one, the session ends up bound to a thread: via `/start-thread` when no binding was
  found, or via `/resume-thread` when one was.
- This holds even when other work happens immediately after the hook fires. The
  specific failure closed is the one found 2026-09-17/18: the hook's own
  `additionalContext` correctly named the required action, and it was still skipped
  once conversation moved elsewhere, leaving the session unbound for its entire
  duration.
- The not-found case presents as a structured question (`AskUserQuestion`, selectable
  options plus free text), not a plain message: "I couldn't find a thread for this
  session, shall we start one? What mission do you want to work on?", with an initial
  inference offered as a candidate option.
- Proven by deliberately reproducing the failure, an unrelated request immediately
  after a session starts or is cleared, and confirming a thread ends up bound anyway.

## Purpose

Parent: M-BOOT-02, harness. Delegated from T-14, itself found while working M-BOOT-02
directly: raised as a short bugfix, then discovered during its own investigation to
need mission-level scope. See `docs/domain/ubiquitous-language.md`'s Task scope entry
for that distinction.

## Intelligence

- `docs/domain/session-continuation-design.md` (in the tsk repo, on `main`): the
  original binding and hook design. This mission extends it; the design doc should be
  corrected wherever this mission's findings show it wrong, the same way M-BOOT-02-01
  corrected the commit-on-`tsk/bootstrap` field.
- `missions/operational/M-BOOT-02-01/M-BOOT-02-01-continuation-harness-report.md`: what
  was built, what broke, and the general lesson about the bootstrap worktree's path
  costing a supervisor's attention. The same category of cost, an agent's own attention
  drifting past a directive, is what this mission addresses.
- `docs/kb/agent-context-self-regulation-and-unattended-handoff.md`: the `/goal`
  mechanism, a candidate lever for a check that persists across turns rather than
  firing once at session start.
- `docs/domain/ubiquitous-language.md`: Task scope, Actor, Thread, Thread continuation.

## Decision authority

Jim decides the resilience mechanism's design. Whether a repeated check (for instance
via `/goal`), a different hook, or something else closes the gap is not fixed in
advance; investigate before choosing.

## Constraints

- Hooks must be bash. Cloud virtual machines are Linux.
- Where `docs/domain/session-continuation-design.md` marks a point deferred, leave it
  deferred unless this mission's own findings require otherwise.

## Out of scope

- Any unattended or autonomous trigger for pausing or resuming. M-BOOT-03's territory.
- Rebuilding `/start-thread`, `/pause-thread` or `/resume-thread`'s own internals,
  unless directly implicated in the resilience gap.
- The thread-to-Plan relationship and the other points M-BOOT-02-01 left deferred,
  unless this mission's findings require touching them.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Reproduce the failure deliberately | A fresh session or a `/clear`, followed immediately by an unrelated request, is confirmed to leave the session unbound under the current mechanism | none | TODO |
| T-02 | Design the persistence mechanism | A design for a check that holds until a thread is bound, not a one-shot session-start nudge, evaluated against at least `/goal` as a candidate | T-01 | TODO |
| T-03 | Build the `AskUserQuestion` not-found prompt | The not-found case presents as a structured question with an inferred candidate option, not plain text | none | TODO |
| T-04 | Implement the persistence mechanism | T-02's design built and wired in | T-02 | TODO |
| T-05 | Prove both scenarios end bound despite distraction | Fresh session and `/clear`, each followed by unrelated work first, both end with a thread bound | T-03, T-04 | TODO |

**Essential task**: T-05. A mechanism that isn't proven against the exact distraction
that caused this mission has not closed the gap.

## First behaviour

Take ownership of the plan above, adding implied tasks, before any code. Read
`docs/domain/session-continuation-design.md` and the M-BOOT-02-01 report in full first.

## Execution constraints

Files: `ops/local/claude-session-start.sh`, `ops/local/thread-*.sh`,
`.claude/skills/{start,pause,resume}-thread/`, consistent with what M-BOOT-02-01 built.

## Report on completion

Outcome: done, failed, or blocked, with attempt count. Your account: what you did, what
this briefing failed to give you, what you found wrong in it. Write it to
`missions/operational/M-BOOT-02-02/M-BOOT-02-02-thread-binding-resilience-report.md`.
