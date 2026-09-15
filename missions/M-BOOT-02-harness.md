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

- `docs/domain/mission-briefing-template.md` (in the tsk repo): the briefing format, with tsk and harness fields
  marked
- `docs/` in the tsk repo, produced by M-BOOT-01
- Claude Code cloud environment documentation
- Cloud sessions clone the repo. Repo `.claude/` files transfer with it. User level
  `~/.claude` files do not, which is why T-01 exists.
- Transcripts are JSONL at `~/.claude/projects/<encoded-dir>/<sessionId>.jsonl`
- Threads hold their own context and can be paused, suspended and resumed. Thread state
  is tsk domain, because the next session may be a different actor. A run record is
  ksobr domain and retrospective. They are two artefacts, not one.
- The plan format's Checkpoint section and `What's Next` pointer are thread state in
  Jim's current practice
- Setup script filesystem output is cached per environment, not per session
- Plugins declared in `.claude/settings.json` are recognised, not installed. An
  unattended session has nobody to accept an install prompt, so the setup script must
  install them explicitly.
- `cargo` and `rustc` are pre-installed on Anthropic-hosted cloud sessions, part of the
  base VM image rather than anything an environment's setup script installs. Confirmed
  2026-09-15: `~/.rustup/settings.toml` carries a March 2026 timestamp, months older than
  the setup-script cache's roughly seven-day expiry, and no `dpkg` entry for `cargo` or
  `rustc` exists, ruling out both an environment setup script and an `apt` install.
- `tsk-bin` and `tsk-core` are already published on crates.io (`0.1.7` as of 2026-04-01).
  The general harness's session init can install tsk with `cargo install tsk-bin` rather
  than building from source or fetching a GitHub release binary. Cost is small (~30s from
  a clean build), so it fits a `SessionStart` hook rather than needing setup-script
  caching. Note the local checkout is ahead of the published crate, at workspace version
  `0.2.0`.
- Cloud environment setup scripts are configured from the web (claude.ai/code) or the
  Desktop app only. The iOS/mobile app selects an existing environment but has no UI to
  create or edit one.
- SessionStart hook confirmed working in a session with only `tsk` attached (2026-09-15):
  `$TSK_BOOTSTRAP_WT` set correctly, resolving the open question left by the prior
  session's cross-repo failure (cwd above both `tsk` and `tsk-nexus`, so
  `tsk/.claude/settings.json` never loaded).
- `tsk` has no prebuilt binary on `PATH` by default; building from source or
  `cargo install` is required.

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

## Tasks

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Move the missions into the repository | M-BOOT and its breakout briefings readable from a fresh clone. A cloud session cannot read Jim's home directory | none | DONE |
| T-02 | Incorporate the mission briefing template into the harness | Template present in `.claude/`, and it states the first behaviour: write your own task list before any other action | none | TODO |
| T-03 | Define the run record format | Level 1 outcome: done, failed or blocked, with attempt count. Level 2: the actor's account of what it did, what the briefing failed to give it, and what it found wrong | none | TODO |
| T-04 | Define the thread state format | Records which tasks are done, which is in progress, and where to resume. Written at the end of every session. Readable by a different actor | none | TODO |
| T-05 | Add the hook that writes thread state | Thread state written alongside the missions at session end | T-04 | TODO |
| T-06 | Add the `SessionEnd` hook for transcripts | Hook pushes the session transcript to the transcripts repo | none | TODO |
| T-07 | Write `CLAUDE.md` | Points at `docs/` and the template. Holds ways of working | T-02 | TODO |
| T-08 | Configure `.claude/settings.json` | Hooks wired, permissions set, local session loads without error | T-05, T-06, T-07 | TODO |
| T-09 | Create the plugin marketplace repo | Harness and the language linter declared in `.claude/settings.json` and installed by a setup script | T-08 | TODO |
| T-10 | Configure the cloud environment | Network access, environment variables, and a setup script that installs the harness and the linter | T-09 | TODO |
| T-11 | Confirm GitHub repo access for cloud sessions | A test cloud session clones the tsk repo and reads a briefing | T-10 | TODO |

**Essential task**: T-11. Repo access denial is the most common cloud routine failure,
and nothing downstream works without it.

## Open decisions

- Where the missions live in the repository: a directory, or a git ref. T-01 decides.
- Run record location: with the missions, or the transcripts repo. T-03 needs it.
