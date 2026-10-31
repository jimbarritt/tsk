# Mission: Bootstrap tsk self hosting

| Field | Value |
|---|---|
| ID | M-BOOT |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | none |

## Objective

- The missions and tasks for building tsk are held in tsk's own ledger.
- Agents execute them from there.
- No bootstrap scaffolding remains.
- The harness does little more than ensure tsk is installed. It delegates the rest of
  the work to the tsk binary.
- An agent never interacts with the ledger directly. It goes through tsk.
- The mission briefing and its format are encoded in the tsk binary rather than held as
  documents an agent reads.

This is a bootstrap in the compiler sense. The objective is reached when tsk has enough
capability to host its own development. It is not reached when tsk is feature complete.
Everything after this point is tracked by tsk.

What is temporary is `tsk/bootstrap` itself, the markdown briefings on it, and the
harness reading them by hand. That arrangement is scaffolding and an exploration of the
design space, removed at M-BOOT-04.

What persists is the official ledger under its own name, which M-BOOT-04 decides.
`docs/domain/persistence-and-sync.md` records `refs/tsk/data` as the name from the
original design. Whether it can stay a custom ref is open: per
`docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md` the cloud
sandbox proxy refuses to write anything outside `refs/heads/*`, which is what forced
the bootstrap onto a branch, and the same constraint applies to the real ref.

## Purpose

Parent mission: TBD. Something like a version one of tsk. This mission is the first
stepping stone in realising the tsk vision.

## Intelligence

- `docs/domain/` (in the tsk repo): distilled design decisions, findings and open
  questions
- `docs/domain/mission-briefing-template.md` (in the tsk repo): the briefing format
- `jimbarritt/dotfiles`, `home/claude/skills/plan-format/PLAN-FORMAT.md`: the existing
  plan format and its gaps
- Claude Code cloud environment documentation
- DoltHub write up on git remotes as Dolt remotes

## Decision authority

Jim decides everything within this objective, except where a task defers a decision to
a delegated mission. M-BOOT-04 decides the official ledger layout.

## Decisions

**2026-10-02: the binary comes before the plugin.** The order is M-BOOT-03, M-BOOT-04,
then M-BOOT-05, then a Claude Code plugin built on the binary. A plugin of skills over
the bash scripts is not written first. Work on the binary starts the weekend of
2026-10-03.

Reason: the plugin exists to run tsk in other repos, including repos Jim does not
control, such as a work repo. The bootstrap scripts push a `tsk/bootstrap` branch to the
origin of the repo they manage. That needs push permission on that repo. A plugin built
on those scripts fails in a repo where the operator cannot push a branch.

Consequences:

- The official ledger needs a home outside the managed repo. A nexus tracking repo,
  namespaced by managed repo, is a requirement, not an option. See "tsk metadata in the
  nexus, not the repo" in `future-missions-tbd.md`. M-BOOT-04 decides the ledger's
  location as well as its layout.
- The plugin installs the binary separately and does not bundle it, because tsk also
  runs standalone. A Homebrew recipe is part of that. At session start the plugin checks
  that the binary is installed and tells the user how to install it. A binary-backed
  skill stops and reports the problem when the binary is absent or too old. This is
  settled for now. Jim will review it later.
- Packaging facts are in `docs/kb/claude-code-plugin-packaging.md` in the tsk repo.

**2026-10-03: close M-BOOT-02, defer M-BOOT-03, merge M-BOOT-05 into M-BOOT-04.**
M-BOOT-02 closes with what is done, and every unfinished task is deferred. M-BOOT-03,
unattended cloud runs, is deferred until after the binary. M-BOOT-04 is about getting
the `tsk` binary up and running in place of the bootstrap scripts, and it absorbs
M-BOOT-05's migration and retirement of `tsk/bootstrap`. Sessions run as automated as
possible but are started by Jim. This replaces the order in the 2026-10-02 decision:
the order is now M-BOOT-04, then the plugin, with M-BOOT-03 later.

## Constraints

- Only features needed for self hosting are in scope. Anything else becomes a task
  recorded in tsk after self hosting.
- The exception is tooling Jim uses to run the bootstrap work itself. It is in scope
  as a sub-mission. The case is M-BOOT-06, Mission Control. Amended 2026-09-25.
- Stay within Claude Pro subscription limits. See Doctrine.

## Out of scope

- Completing tsk's feature set.
- The territory filter command line interface.
- The nexus link direction question.
- Building ksobr beyond the harness this bootstrap needs.
- The Claude Code plugin, until the binary exists. See Decisions.
- Cloud agent orchestration, until M-BOOT-03 is picked up again.

## Plan

| ID | Task | Objective | Delegated to | Blocked by | Status |
|---|---|---|---|---|---|
| [M-BOOT-01](M-BOOT-01-substrate.md) | Substrate | Every place exists and holds its first content; `docs/` is sufficient for an agent with only the repo clone | none | none | DONE |
| [M-BOOT-02](M-BOOT-02/M-BOOT-02-briefing.md) | Harness | A local session and a test cloud session both load the harness and read a briefing | none | M-BOOT-01 | DONE |
| [M-BOOT-03](M-BOOT-03-operation.md) | Operation | One unattended run produces a pull request and a run record | none | M-BOOT-02 | DEFERRED |
| [M-BOOT-04](M-BOOT-04-official-ledger.md) | The official ledger | The `tsk` binary replaces the bootstrap scripts, the missions move into the ledger, and `tsk/bootstrap` is tagged | none | M-BOOT-02 | DONE |
| [M-BOOT-07](M-BOOT-07-session-migration.md) | Session migration | Every session runs on the binary, the plugin and `tsk/ledger`, and the `tsk/bootstrap` refs are deleted | none | M-BOOT-04 | IN PROGRESS |
| [M-BOOT-06](M-BOOT-06-mission-control.md) | Mission Control | A single command sets up a tmux session with a list of Claude sessions, the selected session, and a terminal | none | none | TODO |

**Essential task**: M-BOOT-07, since 2026-10-06, when M-BOOT-04 closed and its
migration items moved to M-BOOT-07. Its objective and this mission's objective are the
same state.

M-BOOT-04 will decompose into at least these candidate tasks:

- Ledger tree layout.
- Manifest format.
- Ledger location: a nexus repo outside the managed repo, namespaced by managed repo.
- Push and pull protocol, including the compare and swap retry loop.
- Rust git library selection: `git2`, `gitoxide`, or the `git` binary.

`docs/domain/mission-briefing-template.md` (in the tsk repo) contains a worked example for the first of these.

## Doctrine

**Model selection**

- Design decisions: Opus 5.
- Harness setup: Opus 5.
- Execution: Sonnet 5 by default.
- Review of agent output: Opus 5.
- Fable 5.1: one case only, the compare and swap retry loop design, if its failure modes
  need detailed reasoning before handover.

**Subscription limits**

- A rolling five hour session window.
- A weekly cap that resets at a fixed time for the account.
- Claude Code, Claude Desktop and claude.ai share one allowance.
- Check in Settings then Usage whether Fable access on Pro is usage credits only.

**Risk control**

Unbounded retry is the main risk to the weekly cap. A bounded attempt limit on every
delegated mission is the control.
