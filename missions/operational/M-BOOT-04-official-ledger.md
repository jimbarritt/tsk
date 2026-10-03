# Mission: The official ledger

| Field | Value |
|---|---|
| ID | M-BOOT-04 |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | M-BOOT-02 |

## Objective

Kind: attainable

- The `tsk` binary is up and running and does the work the bootstrap scripts in
  `ops/local/` do today: fetch and push the missions, the thread commands, and the
  external event queue.
- The harness does little more than ensure tsk is installed. The hooks, skills,
  `CLAUDE.md` and justfile call the binary, and the bootstrap scripts are removed.
- The official ledger exists. Its location, tree layout and manifest format are decided
  here and proven by tests.
- The missions, threads and external events are held in the ledger, and agents execute
  from there.
- `tsk/bootstrap` is tagged and no longer written to.

This mission absorbed M-BOOT-05 (migration off the bootstrap branch) on 2026-10-03. Its
end state and M-BOOT's objective are the same.

## Purpose

Parent: M-BOOT, bootstrap tsk self hosting. The harness runs on bash scripts over the
`tsk/bootstrap` branch. That arrangement is scaffolding. The binary is what tsk ships,
and the 2026-10-02 decision on M-BOOT puts the binary before the plugin.

## Intelligence

- M-BOOT, Decisions, 2026-10-02: the binary comes before the plugin, and the ledger
  needs a home outside the managed repo. A nexus tracking repo, namespaced by managed
  repo, is a requirement.
- `future-missions-tbd.md`, "tsk metadata in the nexus, not the repo".
- `docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md` (on `main`):
  a cloud session can write only `refs/heads/*`. The ledger is a branch, not a custom
  ref.
- `docs/adr/0010` (on `main`): the ledger stays a branch, not a directory on `main`.
- `docs/domain/persistence-and-sync.md` and
  `docs/adr/0007-event-log-as-source-of-truth.md` (on `main`): the persistence design.
- `docs/domain/session-continuation-design.md` (on `main`): the thread and continuation
  design the bash harness implements.
- `ops/local/` on `main`: the behaviour the binary reproduces. `fetch-bootstrap-ref.sh`,
  `push-bootstrap-ref.sh`, `bootstrap-wt-*.sh`, `thread-*.sh`, `mint-token-lib.sh`,
  `claude-session-start.sh`, and the external event scripts
  (`append-external-event.sh`, `read-new-external-events.sh`,
  `advance-external-events-watermark.sh`, `poll-security-alerts.sh`).
- `docs/kb/claude-code-plugin-packaging.md` (on `main`): packaging facts for the plugin
  that follows this mission.

## Decision authority

Jim decides the ledger location, tree layout, manifest format, Rust git library, and
the command names.

## Constraints

- Sessions run as automated as possible but are started by Jim. No cloud agent
  orchestration. Cloud agents come later (M-BOOT-03, deferred).
- Only features needed for self hosting are in scope.
- The harness keeps working at every step. A script stays until the binary replaces it.
- Tests are required for each new command: unit and end to end.

## Out of scope

- Cloud agents and unattended runs (M-BOOT-03, deferred).
- The Claude Code plugin. It follows the binary.
- The run record format and the transcripts hook (M-KSOBR).
- The latency harness (`run-latency-harness.py`), which is not scaffolding.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Select the Rust git library | `git2`, `gitoxide` or the `git` binary chosen, with the reason recorded | none | DONE |
| T-02 | Decide the ledger location and layout | Location (nexus repo, namespaced by managed repo), branch name, tree layout and manifest format documented | none | DONE |
| T-03 | Add `tsk ledger fetch` | Fetches the missions and materialises them at a fixed worktree path, refreshing in place. Prints the path. Replaces `fetch-bootstrap-ref.sh` and `bootstrap-wt-path.sh` | T-01 | TODO |
| T-04 | Add `tsk ledger push` | Commits the worktree changes, fetches the latest state, and pushes, with a bounded compare and swap retry. Replaces `push-bootstrap-ref.sh` | T-03 | TODO |
| T-05 | Move the thread commands into the binary | Start, pause with handover, resume, detach, stop, switch, list and binding resolution run through `tsk`. Replaces `thread-*.sh` and `mint-token-lib.sh` | T-04 | TODO |
| T-06 | Move the external event queue into the binary | Append, read new events, and advance the watermark run through `tsk`. Replaces the external event scripts | T-04 | TODO |
| T-07 | Switch the harness to the binary | Hooks, skills, `CLAUDE.md` and the justfile call `tsk`. The `SessionStart` hook ensures tsk is installed. The replaced scripts are removed | T-05, T-06 | TODO |
| T-08 | Migrate to the ledger | Missions, threads and external events held in the ledger at the T-02 location, with history preserved or deliberately dropped | T-02, T-07 | TODO |
| T-09 | Retire `tsk/bootstrap` | The branch is tagged, `CLAUDE.md` points at the ledger, and no bootstrap scaffolding remains | T-08 | TODO |
| T-10 | Hold a ledger in the nexus | `tsk config attach-nexus <url>` records the nexus in the user config. A managed repo's entry in `nexus.json` with `"ledger": "nexus"` holds its ledger on a namespaced branch in the nexus, and `tsk ledger fetch` and `tsk ledger push` work against it. The tsk-nexus README and `docs/domain/territory-and-nexus.md` say the nexus holds ledgers as an option | T-09 | TODO |

**Essential task**: T-09. Its end state and M-BOOT's objective are the same.

## Decisions

- 2026-10-03, T-01: the binary calls the `git` executable through
  `std::process::Command`. No git library. `gitoxide` has no push. `git2` adds a C build
  dependency and needs its own credential handling, which does not reuse the user's
  credential helpers or the cloud sandbox proxy configuration. The `git` binary is
  already required wherever the harness runs, and each script step maps to one call.
  The code parses machine-readable forms only: plumbing commands (`rev-parse`,
  `for-each-ref`) and porcelain modes (`worktree list --porcelain -z`,
  `status --porcelain=v2 -z`, `push --porcelain`, `log --format=... -z`). T-04 detects a
  rejected compare and swap from the `push --porcelain` status flag, not from error
  text.
- 2026-10-03: the fetch and push commands are named `tsk ledger fetch` and
  `tsk ledger push`.
- 2026-10-03, T-02: the ledger location is set per managed repo. The default is a
  branch in the managed repo itself. The option is a namespaced branch in the nexus,
  for a repo the operator cannot push to. tsk's own ledger stays in the tsk repo. The
  nexus is `https://github.com/jimbarritt/tsk-nexus`, attached with
  `tsk config attach-nexus <url>`, which writes the user config. The nexus option is
  T-10, the last task of this mission, so tsk runs in a work repo from Monday
  2026-10-05.
- 2026-10-03, T-02: the in-repo ledger is `refs/heads/tsk/ledger`. A nexus ledger is
  `refs/heads/ledgers/<host>/<owner>/<repo>`, with the identity derived from the managed
  repo's `origin` URL, normalised so HTTPS and SSH map to the same name. The choice per
  repo is in the nexus: the repo's entry in `nexus.json` carries `"ledger": "nexus"` or
  `"ledger": "repo"`. tsk finds the entry by matching the normalised `origin` URL
  against each entry's `url`. No attached nexus, no entry, or no `ledger` field means
  in-repo. The binary always passes full ref names to git. Tried first, not final: Jim
  expects a repo may need an entry in `nexus.json` before tsk works with it. For T-10 the
  entry is added to `nexus.json` by hand, with no registration command.
- 2026-10-03, T-02: the ledger tree is the `tsk/bootstrap` tree unchanged (`index.md`,
  `future-missions-tbd.md`, `missions/`, `threads/`, `external-events/`), the same in
  both locations, plus `.tsk-ledger.toml` at the root holding `version = 1`. A binary
  that reads a version it does not support stops and reports it. T-08 starts
  `tsk/ledger` from the current `tsk/bootstrap` commit, so history carries over. The
  ADR 0007 event log is not part of this layout.

## Open decisions

- Whether `tsk` commands go through `tskd` or run in the client alone. The daemon owns
  all state today.
- Whether encoding the mission briefing and its format in the binary (an M-BOOT
  objective line) is part of this mission or a later one.
- Whether T-03 and T-04 target `tsk/bootstrap` first and move to the ledger in T-08, or
  target the ledger from the start.
