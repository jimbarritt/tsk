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
- Publishing the Claude Code plugin to a marketplace. The plugin's source tree, `plugin/` on `main`, is in scope from 2026-10-04.
- The run record format and the transcripts hook (M-KSOBR).
- The latency harness (`run-latency-harness.py`), which is not scaffolding.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Select the Rust git library | `git2`, `gitoxide` or the `git` binary chosen, with the reason recorded | none | DONE |
| T-02 | Decide the ledger location and layout | Location (nexus repo, namespaced by managed repo), branch name, tree layout and manifest format documented | none | DONE |
| T-03 | Add `tsk ledger fetch` | First, `docs/domain/ledger-layout.md` on `main` documents every ledger file, its format and its JSON fields, taken from the scripts. T-04 to T-07 build against it. Then: fetches the missions and materialises them at a fixed worktree path, refreshing in place. Prints the path. Replaces `fetch-bootstrap-ref.sh` and `bootstrap-wt-path.sh` | T-01 | DONE |
| T-04 | Add `tsk ledger push` | Commits the worktree changes, fetches the latest state, and pushes, with a bounded compare and swap retry. Replaces `push-bootstrap-ref.sh` | T-03 | DONE |
| T-05 | Move the thread commands into the binary | Start, pause with handover, resume, detach, stop, switch, list and binding resolution run through `tsk`. Replaces `thread-*.sh` and `mint-token-lib.sh`. The older daemon-backed `thread`, `task`, `context` and `where` commands are removed | T-04 | DONE |
| T-06 | Move the external event queue into the binary | Append, read new events, and advance the watermark run through `tsk`. Replaces the external event scripts | T-04 | DONE |
| T-07 | Cut over to the binary and the ledger | `tsk/ledger` created from the `tsk/bootstrap` tip with `.tsk-ledger.toml` added, so history carries over. Hooks, skills, `CLAUDE.md` and the justfile call `tsk`. The thread skills and the `SessionStart` and `Stop` hooks move from `.claude/` into `plugin/`, and this repo loads them from there. `ops/local/poll-security-alerts.sh` pipes its events to `tsk events append` in batch form. Prose uses "ledger worktree" and "code worktree", never "worktree" alone. The `SessionStart` hook ensures tsk is installed. The replaced scripts are removed | T-02, T-05, T-06 | DONE except removal of the replaced scripts: Jim runs the `git rm` command (main 96f8dd1, 47a5325, 7c593a8; `tsk/ledger` created and migrated) |
| T-08 | Migrate to the ledger | Merged into T-07 on 2026-10-03 | n/a | MERGED |
| T-09 | Retire `tsk/bootstrap` | The branch is tagged, `CLAUDE.md` points at the ledger, and no bootstrap scaffolding remains | T-07 | TODO |
| T-10 | Hold a ledger in the nexus | `tsk config attach-nexus <url>` records the nexus in the user config. A managed repo's entry in `nexus.json` with `"ledger": "nexus"` holds its ledger on a namespaced branch in the nexus, and `tsk ledger fetch` and `tsk ledger push` work against it. The tsk-nexus README and `docs/domain/territory-and-nexus.md` say the nexus holds ledgers as an option | T-09 | TODO |
| T-11 | Install the harness outside this repo | The hooks and skills have no dependency on the tsk repo and call only `tsk`. The plugin at `plugin/` installs them through a marketplace entry of the `git-subdir` form, so a session in another repo, such as a work repo, runs the harness. Optional: if it is not done by Monday 2026-10-05, the hooks and skills are copied by hand | T-10 | TODO |

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
- 2026-10-03: `tsk ledger fetch` and `tsk ledger push` target `tsk/ledger` from the
  start, resolved through the T-02 rules. Their e2e tests run against temporary bare
  repos. The harness stays on the scripts and `tsk/bootstrap` until the cut-over, so
  T-08 is merged into T-07: the harness cannot switch before `tsk/ledger` exists.
- 2026-10-03: the new commands run in the `tsk` client alone, with no `tskd`. The
  harness thread model takes the name `tsk thread`. T-05 removes the older daemon-backed
  `thread`, `task`, `context` and `where` commands and their tests. `tskd` stays in the
  workspace, unused. It existed for cross-project references, which a later mission
  revisits. Jim has a small number of live usages of the old commands in other
  projects. They are migrated after this mission, by hand.
- 2026-10-03: the scripts are the only definition of the ledger file formats today.
  T-03 writes them down first in `docs/domain/ledger-layout.md`. The
  `continuation-state.jsonl` field `commit_on_bootstrap` becomes `commit_on_ledger`.
  Existing entries are not rewritten: the binary reads either name and writes
  `commit_on_ledger`. `.git/tsk-clone-id` and `.git/tsk-thread-id` stay local to the
  clone in both ledger locations.
- 2026-10-03: encoding the mission briefing and its format in the binary is a later
  mission. The binary moves mission files without parsing them.
- 2026-10-04: command names accepted: `tsk ledger fetch | path | push`,
  `tsk thread start | pause | resume | detach | stop | list | binding | guard`,
  `tsk events append | read-new | advance-watermark`. Scaffolding a thread directory and
  minting a thread ID are internal to `tsk thread start`, not commands.
- 2026-10-04: the security alert poller is this repo's own extension of the external
  event queue, not part of tsk. It stays in `ops/local/`, which holds scripts local to
  this repo and is not distributed. It calls `tsk events append` and never writes the
  queue file. `tsk events append` takes a batch of events on stdin as NDJSON, with one
  fetch and one push per batch.
- 2026-10-04: "ledger worktree" names the per-clone linked worktree that holds the
  ledger branch. "Code worktree" names a worktree of the managed repo's code. Prose
  never uses "worktree" unqualified. Recorded in `docs/domain/ubiquitous-language.md`.
  ADR bodies are not rewritten.
- 2026-10-04: the Claude Code plugin's source tree is `plugin/` in the tsk repo:
  `.claude-plugin/plugin.json` (name `tsk`, semver `version` starting at 0.1.0, because the version is visible to the user; a release is a version change),
  `skills/`, `hooks/hooks.json`. Anything in `ops/local/` that tsk distributes moves out
  to a source directory. The concurrency gap on the shared ledger worktree is recorded in
  `future-missions-tbd.md`.
- 2026-10-04: the checks in the binary that are stricter than the scripts are accepted.
  Exit code 2 means a usage error.
- 2026-10-04: `cli/src/agent-context.md` moves into the plugin as a skill. A new
  command, `tsk thread session-start`, does the `SessionStart` work and prints the
  context message, so the unbound prompt text has one copy, in the binary. ADR 0011:
  all logic lives in the binary; plugin hooks, skills and scripts hold the minimum.
- 2026-10-04: `tskd` is retired (ADR 0012). Git ledgers, located through the nexus,
  are the only shared state. Its 43 tests move to `daemon/tests/`, so the later removal
  of the crate is clean. Rebuilding the TUI on the ledgers is a later mission. A local
  cache index is recorded in the ADR as a deferred extension.
- 2026-10-04: the T-03 to T-06 output details are accepted: `tsk ledger push` prints the
  resulting SHA, `tsk ledger path` does not mint a clone ID, event payloads keep their
  bytes, and the T-10 URL normalisation rules in `ledger-layout.md` stand. A managed
  repo whose `origin` is a local path or `file://` URL uses an in-repo ledger only.
  `commit_on_main` holds the commit of origin's default branch, not of a branch named
  `main`. The field name stays.
- 2026-10-04, note for T-10: the nexus e2e tests need an `origin` URL with a host. Set
  `origin` to a URL such as `https://example.test/owner/repo` and a git
  `url.<bare-repo-path>.insteadOf` rule. tsk reads the raw `remote.origin.url` from
  config, not `git remote get-url`, which applies the rewrite.
- 2026-10-04: `tsk thread pause` checks that `HEAD` is reachable from some branch on
  origin, not only the default branch, because work happens on feature branches and in
  other code worktrees. The entry records the branch at pause as `code_ref` next to the
  commit. A rename of `commit_on_main` to `code_commit` is to be decided with it. Every
  command takes its context from the directory it runs in; a branch switch needs no
  tracking, because each entry is a snapshot at pause. Done in T-07, replacing
  `ensure_on_origin_default_branch` in `cli/src/thread/ops.rs`. A session that moves
  into another code worktree resolves that code worktree's binding; this is recorded in
  the design doc, with no mechanism.
- 2026-10-04, T-10, changes the T-02 identity decision: each managed repo gets a minted
  repo ID, held in its nexus entry and in the ledger's `.tsk-ledger.toml`. A nexus ledger
  branch is `refs/heads/ledgers/<repo-id>`, not keyed by URL. A nexus entry has an
  optional `url`. With no URL it carries `"local": "<machine name>"`, and other machines
  that pull the nexus do not fetch it. On the first push the entry gains `url` and
  `local` is removed; the ID and the branch stay. A rename changes only `url`. Matching
  the normalised `origin` URL against entries stays, to find the entry for a clone with
  no local record of its ID. Local-only repos can be in the nexus.
- 2026-10-04, T-07: a continuation state entry holds its commits in a nested `git`
  object: `git.ledger.commit`, `git.code.ref` and `git.code.commit`. The flat fields
  `commit_on_bootstrap`, `commit_on_ledger` and `commit_on_main` are removed, with no
  read fallback in the binary. Existing entries are migrated once, on `tsk/ledger`, in
  a commit after the one that creates it. A migrated entry gets
  `git.code.ref = "refs/heads/main"`, because the old rule required its commit to be on
  origin's default branch.
- 2026-10-04, T-07: this repo holds a marketplace, `.claude-plugin/marketplace.json` at
  the repo root, with one entry, `tsk`, source `./plugin`. The repo's `SessionStart`
  script adds the marketplace and installs and updates `tsk@tsk` at project scope, as it
  does for `swe`. A change to `plugin/` takes effect after a `version` bump in
  `plugin.json`.
- 2026-10-04, T-07: the plugin installs the binary, and the plugin holds the `tsk`
  version it requires. Its `SessionStart` hook compares `tsk --version` with that
  version. When `tsk` is not on `PATH` or the versions differ, it runs
  `cargo install --path "$TSK_SOURCE"` when `TSK_SOURCE` is set, otherwise
  `cargo install tsk-bin --version <required>`. The workspace version moves to 0.2.0,
  unpublished, and the plugin requires 0.2.0. This repo sets `env.TSK_SOURCE` to its `cli/`
  in `.claude/settings.json`, and `just build-install` updates the binary. When the
  install fails or `tsk` is still not found, the hook exits with the install command and
  the command that runs the session start again.

## Open decisions

- Whether the `tsk` marketplace moves from this repo to `jimbarritt/claude-plugins`.
  Decided later.

