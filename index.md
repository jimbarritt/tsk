# tsk bootstrap ref index

This branch is the data store for tsk's own missions and tasks. This file is the entry
point: read it first, then the briefing for the mission you are working.

There is no `plan.md`. "Plan" is a domain term attached to the execution of a single
mission, not the name of a file holding the state of all of them. See
`docs/domain/ubiquitous-language.md` in the tsk repo. To ask where things stand, ask for
mission status at the level of scale you mean.

## Current mission

**M-BOOT-02: harness.** Full briefing:
[missions/operational/M-BOOT-02/M-BOOT-02-briefing.md](missions/operational/M-BOOT-02/M-BOOT-02-briefing.md).
Its intelligence is gathered in
[missions/operational/M-BOOT-02/intel-index.md](missions/operational/M-BOOT-02/intel-index.md).

Parent mission: **M-BOOT, bootstrap tsk self hosting.** Reached when the missions and
tasks for building tsk are held in tsk's own ledger and agents execute them from there,
with no bootstrap scaffolding remaining. This is a bootstrap in the compiler sense:
reached when tsk can host its own development, not when tsk is feature complete. Full
briefing: [missions/operational/M-BOOT.md](missions/operational/M-BOOT.md).

## Mission tree

| ID | Mission | Objective | Status | Blocked by |
|---|---|---|---|---|
| [M-BOOT](missions/operational/M-BOOT.md) | Bootstrap tsk self hosting | tsk hosts its own development, no bootstrap scaffolding left | TODO | none |
| [M-BOOT-01](missions/operational/M-BOOT-01-substrate.md) | Substrate | Every place the bootstrap needs exists and holds its first content | ✓ DONE | none |
| [M-BOOT-02](missions/operational/M-BOOT-02/M-BOOT-02-briefing.md) | Harness | A local and a cloud session both load the harness and read a briefing | IN PROGRESS | M-BOOT-01 |
| [M-BOOT-02-01](missions/operational/M-BOOT-02-01-continuation-harness.md) | Continuation harness | `/start-thread`, `/pause-thread`, `/resume-thread` work end to end | ✓ DONE | M-BOOT-02 |
| [M-BOOT-02-02](missions/operational/M-BOOT-02-02-thread-binding-resilience.md) | Thread binding resilience | A session never ends up unbound after `SessionStart` fires, even when distracted first | ✓ DONE | M-BOOT-02 |
| [M-BOOT-03](missions/operational/M-BOOT-03-operation.md) | Operation | One unattended run produces a pull request and a run record | TODO | M-BOOT-02 |
| M-BOOT-04 | The official ledger | No breakout briefing yet | TODO | M-BOOT-03 |
| M-BOOT-05 | Migration off the bootstrap ref | No breakout briefing yet | TODO | M-BOOT-04 |
| [M-BOOT-06](missions/operational/M-BOOT-06-mission-control.md) | Mission Control | A single command sets up a tmux session with a list of Claude sessions, the selected session, and a terminal | TODO | none |
| [M-LAB](missions/operational/M-LAB-ai-lab-notes.md) | AI lab notes (journalling plugin) | Skeleton, most fields TBD | TODO | none |
| [M-STORY](missions/operational/M-STORY-tsk-story-deck.md) | tsk story deck | A Marp deck in `docs/slide-decks/overview-for-engineers/` tells tsk's domain and features as a product, for other engineers | TODO | none |

Essential mission: M-BOOT-05. Its objective and M-BOOT's objective are the same state.

## Administrative missions

Missions that keep the work itself in order rather than building the artefacts. Kept in
`missions/administrative/` and out of the tree above, because these do not sequence and
nothing blocks them. Where one has no end point it has no DONE, so the tree's status
column does not apply and this table omits it.

| ID | Mission | Objective |
|---|---|---|
| [M-ADMIN-01](missions/administrative/M-ADMIN-01-idea-capture.md) | Idea capture | An idea Jim states in a session reaches the ledger rather than being lost |
| [M-ADMIN-02](missions/administrative/M-ADMIN-02-adhoc-tasks.md) | Ad hoc tasks | A task Jim raises mid-session, unrelated to that session's mission, is tracked to done rather than lost |
| [M-ADMIN-03](missions/administrative/M-ADMIN-03-dependency-vulnerability-watch.md) | Dependency vulnerability watch | An open Dependabot alert on tsk's default branch gets checked and fixed rather than sitting unaddressed |

This is the answer to a thread that has no mission, settled 2026-09-17. A session kept
open to capture ideas, a tidy up thread looking for work that needs a nudge, a reviewer
watching missions for drift, a mission distilling trends out of mission reports: each is
a mission, not a thread without one. The harness needs nothing added for them.
`thread-start.sh` validates one thing, that the briefing path exists, so an
administrative mission takes a thread, a pause and a resume like any other. Rationale:
`docs/domain/mission-model.md` in the tsk repo, under mission categories.

Only M-ADMIN-01 has a briefing. The other three examples are recorded there as examples,
not as missions.

Ideas for missions that are not shaped into briefings yet are kept in
[future-missions-tbd.md](future-missions-tbd.md). Nothing there is committed to.

Full task breakdowns are in each mission's own briefing under `missions/`. All tsk
development is tracked through this mission tree. The backlog that predates it is kept
at the foot of this file and becomes tsk's own once M-BOOT-05 completes.

## Domain design reference

Distilled design decisions for tsk's own domain model live in the tsk repo's
`docs/domain/` directory: `ubiquitous-language.md`, `domain-model-overview.md`,
`mission-model.md`, `bootstrap-rationale.md`, `territory-and-nexus.md`,
`persistence-and-sync.md`, and `mission-briefing-template.md`.

---

## Checkpoint: Session 2026-09-14

**What was completed this session:**
- `jimbarritt/tsk-nexus`: added `nexus.json` at the root, indexing the
  `agentic-engineering` territory with `tsk` and `ksobr` (the latter not yet created)
- `jimbarritt/tsk-nexus`: added a `README.md` explaining the territory/nexus concept,
  linking back to `docs/domain/territory-and-nexus.md` in the tsk repo
- Both committed and pushed to `origin/main`
- M-BOOT-01 (substrate) marked complete: all eight tasks done, T-02 closed out

**State of the project:**
M-BOOT-01 is fully done. M-BOOT-02 (harness) is in progress: T-01 is done, and the
mission briefings now live in the repository's `tsk/bootstrap` branch rather than a
home directory a cloud session cannot read.

**Immediate next priorities:**
1. Incorporate the mission briefing template into the harness (T-02)
2. Define the run record and thread state formats (T-03, T-04)

## Legacy backlog (pre-bootstrap)

Out of scope until self hosting completes, per M-BOOT's constraints. Carried over
unchanged from the previous plan format so nothing is lost. Once M-BOOT-05 completes,
these become tasks recorded in tsk itself rather than in this file.

### Done

**Threads**
- `thread create`, `thread list`, `thread switch-to`
- `thread update` (slug, description, priority: renames dir, updates index.md)
- `thread wait` / `thread resume`: Waiting state with optional reason
- Thread directory scaffolded with `index.md` on create; updated on slug, priority or
  description change

**Tasks**
- `task create`, `task list`, `task start`, `task block`, `task complete`,
  `task cancel`, `task update`
- State machine: `not-started → in-progress → done`; `in-progress ↔ blocked`; any →
  `cancelled`
- Diversion pattern: `--thread <id>` on all task commands targets a non-active thread
  without switching

**TUI**
- Thread list grouped by section (Active / Priority & Incidents / Background)
- Vim keybindings: `j`/`k`, `ctrl-d`/`ctrl-u`, `gg`/`G`
- Mouse scroll
- Status bar: active thread id and slug, `? help` hint
- Help popup: `?` toggles keybinding overlay

**CLI / infra**
- `--version` / `-V` on `tsk` and `tskd`
- `tsk context` outputs `agent-context.md`
- JSON output throughout

### Bugs

| ID    | Severity | Description |
|-------|----------|-------------|
| BUG-1 | crash    | TUI panics on terminal resize when description contains multi-byte characters (e.g. em dash). Root cause: byte indexing instead of char indexing for truncation. Also: terminal not restored on panic, needs a panic hook. |
| BUG-2 | visual   | Right-hand column border misaligned for rows with truncated slugs: leading `│` of next row is consumed into the slug cell. Same underlying byte-width accounting issue as BUG-1. |
| BUG-3 | visual   | Missing `│` separator after the ID column: ID and slug columns run together. |
| BUG-4 | usability | Responsive column hiding not implemented. At narrow widths everything is crushed. Should progressively hide: description, then state, then show only id, slug and priority. |
| BUG-5 | medium   | TUI does not update in real time when thread state changes from outside (e.g. agent calling CLI in another pane). Requires TUI restart. Fix: watch `tsk/threads/index.json` for changes and re-render. |

### Features / next work

**TUI: task view.** The TUI currently shows threads only. Tasks exist in the CLI but are
not visible in the TUI.
- Show tasks for the selected/active thread in the TUI
- Keyboard shortcut to toggle task view (missing from help popup, the original issue)
- FR-2: auto-focus task view when `switch-to` is called, showing thread context
  immediately after a context switch. `ctrl-o` navigates back to the thread list.

**FR-6: cross-thread "today" view.** Show urgent tasks across all threads in a single
view.
- Flag a task as urgent (priority flag, due date of today, or `tsk task flag <id> urgent`)
- `tsk today`: a flat list of urgent or due-today tasks across all threads
- TUI today view as an alternative tab or pane

**Configuration file.** `tsk.toml` or `.tskrc` for per-project and per-user settings.
First planned setting: `show_status_bar = true/false`.
