# OpenSpec

Status: open source, MIT licence, version 1.13.0, read 2026-10-06. From Fission, in
`Fission-AI/OpenSpec`. First public release 2025-09-06. The site counts 68.0k GitHub
stars and "more than 265,000 developers a month".

`openspec.org` is a parked domain listed for sale on Namecheap. OpenSpec is at
[openspec.dev](https://openspec.dev/).

Sources: the [openspec.dev](https://openspec.dev/) home page, its
[installation](https://openspec.dev/docs) page and
[changelog](https://openspec.dev/changelog), and these files in `Fission-AI/OpenSpec`:
[README](https://github.com/Fission-AI/OpenSpec/blob/main/README.md),
[Core Concepts at a Glance](https://github.com/Fission-AI/OpenSpec/blob/main/docs/overview.md),
[Glossary](https://github.com/Fission-AI/OpenSpec/blob/main/docs/glossary.md),
[OPSX Workflow](https://github.com/Fission-AI/OpenSpec/blob/main/docs/opsx.md) and the
[Stores User Guide](https://github.com/Fission-AI/OpenSpec/blob/main/docs/stores-beta/user-guide.md).
Every page was fetched as raw HTML or Markdown and read as text. Quoted phrases were
matched against that text. Claims are OpenSpec's own and were not run independently.
The source code was not read.

## What it is

"OpenSpec is a lightweight agreement layer between you and your AI." A person and a
coding agent write down what a change does, review the same plan, and only then write
code. Its summary is "agree first, then build confidently".

It is a Node.js command-line tool, `openspec`, plus skills and slash commands that
`openspec init` writes into a project for each AI tool chosen. The workflow runs inside
the AI tool. The site names 39 supported tools, among them Claude Code, Codex, Cursor,
GitHub Copilot and Gemini CLI, and "+ 33 more".

## Model

| Concept | Meaning |
|---|---|
| Spec | How part of the system behaves "*right now*". Kept in `openspec/specs/`, one folder per domain such as `auth/`. "Specs are the truth." |
| Requirement | One behaviour, with an RFC 2119 keyword: "The system SHALL expire sessions after 30 minutes." |
| Scenario | A concrete, testable example of a requirement, in Given/When/Then form. "you could write an automated test from one" |
| Change | "One unit of work": a folder in `openspec/changes/<name>/` with a proposal, a design, a task list and delta specs |
| Delta spec | The spec edits in a change, in `ADDED`, `MODIFIED` and `REMOVED` sections. "You describe the diff, not the destination." |
| Artefacts | `proposal.md` (why and what), delta specs, `design.md` (how), `tasks.md` (a checklist). Created in that order, each feeding the next |
| Archive | Finishing a change. Its delta specs merge into `openspec/specs/`, and the folder moves to `changes/archive/YYYY-MM-DD-<name>/` |
| Schema | Which artefacts a workflow has and how they depend on each other. The default is `spec-driven`. A team can fork it or write its own |
| Project config | `openspec/config.yaml`: the default schema, `context:` added to every planning request, and rules per artefact |

The artefact order is "Enablers, not gates": it shows what becomes possible next, and
any artefact can be edited at any time.

## Workflow

| Command | What it does |
|---|---|
| `/opsx:explore` | Reads the code, sets out options, and shapes an idea into a plan. Writes no code |
| `/opsx:propose <name>` | The agent drafts the proposal, specs, design and tasks |
| `/opsx:apply` | The agent implements the tasks and checks them off |
| `/opsx:verify` | Validates the implementation against the artefacts. In the expanded profile only |
| `/opsx:sync` | Merges delta specs into the main specs without archiving |
| `/opsx:archive` | Archives a finished change |

The core profile installs `propose`, `explore`, `apply`, `update`, `sync` and
`archive`. The expanded profile adds `new`, `continue`, `ff`, `verify`, `bulk-archive`
and `onboard`.

## Stores

Beta since v1.5.0, 2026-06-28. "A **store** is the answer: a standalone repo whose whole
job is planning." It has the same `openspec/` folder of specs and changes, and an
identity file, `.openspec-store/store.yaml`.

- **Registration**: `openspec store register <path>` records a store on one machine by
  name. Every OpenSpec command then takes `--store <name>`.
- **Sharing**: "A store is just a git repo. You commit, push, pull, and review it
  yourself. OpenSpec never clones, syncs, or pushes anything on its own."
- **A code repo with all its planning in a store**: one line in `openspec/config.yaml`,
  `store: team-plans`, makes every command in that repo act on the store.
- **Reference**: a code repo declares a store it reads. It keeps its own specs, and gains
  an index of the store's specs.
- **Working context**: `openspec context` lists the repo's OpenSpec root and every store
  it references.
- **Workset**: a set of folders a person opens together, such as a store and the code
  repos it plans. Local to one machine and never committed.

## Positioning

OpenSpec compares itself with two products:

- GitHub's [Spec Kit](https://github.com/github/spec-kit): "Thorough but heavyweight.
  Rigid phase gates, lots of Markdown, Python setup."
- AWS's [Kiro](https://kiro.dev): "Powerful but you're locked into their IDE and
  limited to Claude models."

It collects anonymous usage statistics: command names and version. It is on unless
turned off with `openspec config set telemetry.enabled false`, `OPENSPEC_TELEMETRY=0` or
`DO_NOT_TRACK=1`.

## Comparison with tsk

| tsk term | OpenSpec |
|---|---|
| Product capability | A spec requirement. Both are persistent descriptions of what the product does, held for the life of the product. A tsk capability has acceptance criteria and a health state. An OpenSpec requirement has scenarios and no state |
| Acceptance criteria | Scenarios, in Given/When/Then form |
| Delta | A change, with its delta specs in `ADDED`, `MODIFIED` and `REMOVED` sections |
| `Delta Gate` | Archive. Both move a change into the record of what the product does. tsk's gate opens when the delta deploys to production and the system is healthy. OpenSpec archives when a person runs `/opsx:archive` |
| Mission briefing | A proposal and a design. Neither has an objective as a checkable end state, decision authority or constraints |
| Plan | `tasks.md`: a checklist the agent checks off |
| Mission report | None. `/opsx:verify` checks the code against the artefacts, and nothing records an account of what the inputs failed to give |
| Ledger | The `openspec/` folder in the code repo, on the same branch as the code |
| Nexus | A store: a separate git repo for planning, registered by name on each machine. A store holds the specs and changes. A nexus holds an index of repos, and each repo keeps its own ledger |
| Thread continuation | None. The artefacts are the shared state, and any session reads them |
| Actor | None. OpenSpec does not record who works a change |
| Harness | Any of 72 AI tools, through generated skills and slash commands. This matches ADR 0013: a thin adapter per harness |

## Not documented

- How `/opsx:verify` checks an implementation against the artefacts, and whether it runs
  any test.
- What happens when two people or two sessions edit one change at the same time.
- Who is behind Fission, and whether it sells anything.
