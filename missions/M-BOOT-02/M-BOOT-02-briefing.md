# Mission: Harness

| Field | Value |
|---|---|
| ID | M-BOOT-02 |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | M-BOOT-01 |

## Objective

Kind: attainable

- A local Claude Code session in the tsk repo loads the harness without error.
- A test cloud session clones the tsk repo, loads the harness, and reads a briefing from
  the repository.
- The `SessionEnd` hook pushes that session's transcript to the transcripts repo.
- The session writes thread state at its end, recording which tasks are done and where
  to resume.

## Purpose

Parent: M-BOOT, bootstrap tsk self hosting. An agent executes inside the harness. The
harness is the envelope. The mission briefing sits inside it.

## Intelligence

See [intel-index.md](intel-index.md).

## Decision authority

Jim decides the harness structure, the run record format, and what belongs to ksobr
rather than tsk.

## Constraints

- Hooks must be bash. Cloud virtual machines are Linux. PowerShell hooks do not run.
- Files must use LF line endings. A bash script with CRLF fails on Linux.
- No secrets in the repo. The cloud environment has no secrets store, and environment
  variables are visible to anyone who can edit the environment.
- Ways of working belong in the harness, not in a mission. That is ksobr domain.
- `CLAUDE.md` points at `docs/`. It does not restate the design.

## Out of scope

- Building ksobr beyond what this bootstrap needs.
- Resuming from thread state. M-BOOT-03 proves that.
- Level 3 reflection, a separate scoring pass. Deferred until there are enough runs to
  know what to score.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Move the missions into the repository | M-BOOT and its breakout briefings readable from a fresh clone. A cloud session cannot read Jim's home directory | none | DONE |
| T-02 | Incorporate the mission briefing template into the harness | The harness points a session at `docs/domain/mission-briefing-template.md` and states the first behaviour: take ownership of the plan before any other action. No copy in `.claude/`: tsk is the only repo running this harness, so the docs directory can be relied on. Revisit if another repo installs it | none | TODO |
| T-03 | Define the run record format | Level 1 outcome: done, failed or blocked, with attempt count. Level 2: the actor's account of what it did, what the briefing failed to give it, and what it found wrong | none | TODO |
| T-04 | Define the thread state format | Records which tasks are done, which is in progress, and where to resume. Written at the end of every session. Readable by a different actor. Open first: whether the definition of threads is right, see Open decisions | none | TODO |
| T-05 | Add the hook that writes thread state | Thread state written alongside the missions at session end | T-04 | TODO |
| T-06 | Add the `SessionEnd` hook for transcripts | Hook pushes the session transcript to `ksobr-transcripts`. The repo must be attached with `add_repo` first; that instruction lives in tsk's `CLAUDE.md` for now, as a stopgap. Decide whether the hook can do the attach itself, or whether attaching stays an instruction the agent follows | none | TODO |
| T-07 | Write `CLAUDE.md` | Points at `docs/` and the template. Holds ways of working | T-02 | TODO |
| T-08 | Configure `.claude/settings.json` | Hooks wired, permissions set, local session loads without error | T-05, T-06, T-07 | TODO |
| T-09 | Create the plugin marketplace repo | Harness and the language linter declared in `.claude/settings.json` and installed by a setup script | T-08 | TODO |
| T-10 | Configure the cloud environment | Network access, environment variables, and a setup script that installs the harness and the linter | T-09 | TODO |
| T-11 | Confirm GitHub repo access for cloud sessions | A test cloud session clones the tsk repo and reads a briefing | T-10 | TODO |
| T-12 | Configure `CLAUDE.md` with the Software English compact instructions | Agents in this repo write in Software English by default, in replies and in anything written into a file. Includes the one question at a time rule. Spec: https://github.com/jimbarritt/software-english. Overlaps T-07, which holds ways of working | none | TODO |

**Essential task**: T-11. Repo access denial is the most common cloud routine failure,
and nothing downstream works without it.

## Open decisions

- Where the missions live in the repository: a directory, or a git ref. T-01 decides.
- Run record location: with the missions, or the transcripts repo. T-03 needs it.
- Whether the definition of threads is right. Jim's current reading: a thread is more
  about the actors than about overall work status, which is not what the ubiquitous
  language entry says today. Settle this before T-04 defines a format on top of it.
- Where thread state lives once it has a format: its own artefact, or the sections of
  `index.md` that hold it today. Deferred until T-04. Until then `index.md` keeps the
  summary of missions and tasks with their status.
