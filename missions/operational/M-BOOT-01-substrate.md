# Mission: Substrate

| Field | Value |
|---|---|
| ID | M-BOOT-01 |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | none |

## Objective

- Every place the bootstrap needs exists, and each holds its first content.
- The planning directory in Jim's home directory holds M-BOOT and its breakout
  briefings.
- The transcripts repo exists on GitHub.
- The nexus repo exists and indexes the tsk repo.
- `docs/` in the tsk repo holds every design decision currently held only in Claude web
  conversations, and a definition of the ubiquitous language.
- An agent with only the tsk repo clone can work from `docs/` without access to any
  Claude web conversation.

## Purpose

Parent: M-BOOT, bootstrap tsk self hosting. The harness cannot be built until the
places it writes to exist, and agents cannot execute without the intelligence.

## Intelligence

- `docs/domain/` (in the tsk repo): distilled design decisions, findings and open
  questions
- `docs/domain/mission-briefing-template.md` (in the tsk repo): the briefing format
- `jimbarritt/dotfiles`, `home/claude/skills/plan-format/PLAN-FORMAT.md`: the existing
  plan format and its gaps

## Decision authority

Jim decides the bootstrap substrate. The official ledger layout is not decided here.
M-BOOT-04 decides it.

## Constraints

- The language linter is installed locally before any file in `docs/` is written. It
  enforces clear documentation and removes Claude specific phrasing.
- These tasks run from a supervised Claude CLI session, not unattended.
- The bootstrap substrate is the planning directory in Jim's home directory. It is
  provisional. No git ref is created for it.
- A home directory planning file is unreadable by a cloud session, which clones only the
  repository. M-BOOT-02 moves the missions into the repository before the first cloud
  test.
- Distil the intelligence. Do not paste transcripts.
- Terminology in `docs/` must match the ubiquitous language. Flag any term that
  originated with Claude rather than Jim.

## Out of scope

- The official ledger layout.
- The nexus link direction question.
- The territory filter command line interface.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Create the transcripts repo | Repo exists on GitHub | none | DONE, `jimbarritt/ksobr-transcripts` |
| T-02 | Create the nexus repo | Repo exists and indexes the tsk repo | none | DONE, `jimbarritt/tsk-nexus` (`nexus.json` holds the `agentic-engineering` territory with `tsk` and `ksobr`) |
| T-03 | Put M-BOOT and its breakout briefings in the planning directory | All four briefings readable from the planning directory | none | DONE |
| T-04 | Install the language linter locally | Linter runs against a file in `docs/` and reports | none | DONE |
| T-05 | Enumerate the Claude web conversations holding tsk design work | A list exists, with what each conversation covers | none | DONE, see `.inbox/INDEX.md` |
| T-06 | Define the ubiquitous language | One file in `docs/` defines every adopted term, with the rejected alternatives and the reason | T-05 | DONE, see `docs/domain/ubiquitous-language.md` |
| T-07 | Distil the design decisions into `docs/` | One file per subject, each recording provenance and whether the decision is settled or provisional | T-04, T-06 | DONE for the material found so far (detail below); an ongoing backlog, not a one-time task |
| T-08 | Write the `docs/` index | Index lists every file and what it covers | T-07 | DONE, see `docs/index.md` |

**Essential task**: T-07. Without it every briefing has to restate the design, and
agents infer the missing parts.

T-07 is unbounded as originally scoped: new design material keeps arriving (this
mission alone has drawn from Claude web conversations, a home knowledge-base
directory, and a Google-hosted folder with more still unread). Declared done against
M-BOOT-01's own test instead: an agent with only the repo clone can proceed on the
missions currently active (M-BOOT-02, M-BOOT-04) without asking a Claude
conversation. Distilled so far: `docs/domain/ubiquitous-language.md`,
`docs/domain/mission-model.md`, `docs/domain/persistence-and-sync.md`,
`docs/domain/territory-and-nexus.md`, `docs/kb/background-theory.md`,
`docs/kb/product-and-scale-theory.md`, `docs/adr/0007-event-log-as-source-of-truth.md`,
`docs/decisions/beads-vs-tsk-viability.md`, `docs/vision.md`,
`docs/domain/mission-briefing-template.md`. Not yet distilled, and not blocking: the
Talwrn overlap comparison and most of the Google-hosted `tsk` knowledge-base folder
(see `.inbox/googledrive-tsk-kb-INDEX.md`); revisit each when a specific mission
needs it. Two confidential client case studies were discussed but deliberately not
written into `docs/`.

## Open decisions

- Where the missions live once a cloud session must read them. A directory in the
  repository, or a git ref. M-BOOT-02 T-01 decides and makes the move.
- Run record location. Not needed until M-BOOT-03 T-01, so it can stay open.
