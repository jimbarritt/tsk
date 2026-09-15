# Mission: Bootstrap tsk self hosting

| Field | Value |
|---|---|
| ID | M-BOOT |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | none |

## Objective

Kind: attainable

- The missions and tasks for building tsk are held in tsk's own data ref.
- Agents execute them from there.
- No bootstrap scaffolding remains.
- The harness does little more than ensure tsk is installed. It delegates the rest of
  the work to the tsk binary.
- An agent never interacts with the data ref directly. It goes through tsk.
- The mission briefing and its format are encoded in the tsk binary rather than held as
  documents an agent reads.

This is a bootstrap in the compiler sense. The objective is reached when tsk has enough
capability to host its own development. It is not reached when tsk is feature complete.
Everything after this point is tracked by tsk.

What is temporary is `tsk/bootstrap` itself, the markdown briefings on it, and the
harness reading them by hand. That arrangement is scaffolding and an exploration of the
design space, removed at M-BOOT-05.

What persists is the official data ref under its own name, which M-BOOT-04 decides.
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
a delegated mission. M-BOOT-04 decides the official data ref layout.

## Constraints

- Only features needed for self hosting are in scope. Anything else becomes a task
  recorded in tsk after self hosting.
- Stay within Claude Pro subscription limits. See Doctrine.

## Out of scope

- Completing tsk's feature set.
- The territory filter command line interface.
- The nexus link direction question.
- Building ksobr beyond the harness this bootstrap needs.

## Plan

| ID | Task | Objective | Delegated to | Blocked by | Status |
|---|---|---|---|---|---|
| [M-BOOT-01](M-BOOT-01-substrate.md) | Substrate | Every place exists and holds its first content; `docs/` is sufficient for an agent with only the repo clone | none | none | TODO |
| [M-BOOT-02](M-BOOT-02/M-BOOT-02-briefing.md) | Harness | A local session and a test cloud session both load the harness and read a briefing | none | M-BOOT-01 | TODO |
| [M-BOOT-03](M-BOOT-03-operation.md) | Operation | One unattended run produces a pull request and a run record | none | M-BOOT-02 | TODO |
| M-BOOT-04 | The official data ref | Ref name, tree layout, manifest format, and push and pull protocol exist and are proven by tests | Cloud agents | M-BOOT-03 | TODO |
| M-BOOT-05 | Migration off the bootstrap ref | Queue held in tsk's own data ref, agents execute from it, bootstrap ref deleted or tagged | Cloud agents | M-BOOT-04 | TODO |

**Essential task**: M-BOOT-05. Its objective and this mission's objective are the same
state.

M-BOOT-04 will decompose into at least these candidate tasks:

- Data ref tree layout.
- Manifest format.
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
