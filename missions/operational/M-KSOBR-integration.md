# Mission: ksobr integration

| Field | Value |
|---|---|
| ID | M-KSOBR |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | none |

Skeleton. Fields marked TBD are not yet decided.

## Objective

Kind: TBD

- Session transcripts are pushed to the transcripts repo by a `SessionEnd` hook.
- A run record format exists, and a run record is written for each run.

## Purpose

Parent mission: TBD. Related to M-BOOT, bootstrap tsk self hosting, but not a task of
it. M-BOOT puts building ksobr beyond the harness out of scope. These tasks were
deferred out of M-BOOT-02 on 2026-10-03 so the binary work in M-BOOT-04 could go first.

## Intelligence

- `jimbarritt/ksobr-transcripts`: the transcripts repo, created in M-BOOT-01 T-01.
- Transcripts are JSONL at `~/.claude/projects/<encoded-dir>/<sessionId>.jsonl`.
- `CLAUDE.md` on `main` carries a temporary instruction to push the transcript at
  session end, attaching the repo with `add_repo`.
- A run record is ksobr domain and retrospective. Thread state is tsk domain and tells
  the next actor where to resume. They are two artefacts, not one.
- Run record location, settled 2026-09-16 in M-BOOT-02: with the missions, at
  `missions/{id}/{id}-{slug}-report.md`.
- M-BOOT-02 T-17 (a `SessionEnd` hook that triggers the pause and handover) shares the
  hook constraints recorded in the M-BOOT-02 briefing.

## Decision authority

Jim.

## Constraints

- Hooks must be bash. Cloud virtual machines are Linux. PowerShell hooks do not run.
- Files must use LF line endings.
- No secrets in the repo.

## Out of scope

- TBD

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Define the run record format | Level 1 outcome: done, failed or blocked, with attempt count. Level 2: the actor's account of what it did, what the briefing failed to give it, and what it found wrong. Moved from M-BOOT-02 T-03 | none | TODO |
| T-02 | Add the `SessionEnd` hook for transcripts | Hook pushes the session transcript to the transcripts repo. Moved from M-BOOT-02 T-06 | none | TODO |

## Open decisions

- Whether these tasks run on the bash harness or wait for the binary (M-BOOT-04).
- Whether the journalling plugin (M-LAB) belongs in this mission or stays separate.
