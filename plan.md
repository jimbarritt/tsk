# tsk: plan

## What's Next

M-BOOT-01 (substrate) is complete: all eight tasks are done, including the nexus repo
(`jimbarritt/tsk-nexus`), which now holds `nexus.json` indexing the `agentic-engineering`
territory (`tsk` and `ksobr`). M-BOOT-02 (harness) is in progress: T-01 (move the
missions into the repository) is done, and T-02 (mission briefing template into the
harness) is next unblocked. See
[missions/M-BOOT-02-harness.md](missions/M-BOOT-02-harness.md).

**Needs testing: the SessionStart hook.** `.claude/hooks/session-start.sh` (wrapping
`ops/local/claude-session-start.sh`) is meant to check out and pull `main`, fetch
`refs/tsk/bootstrap`, and export its worktree path, automatically at session start. In
a session with both `tsk` and `tsk-nexus` attached it did not fire: `$TSK_BOOTSTRAP_WT`
and `$CLAUDE_PROJECT_DIR` were both empty, and the diagnostics log showed no trace of it
running. Likely cause: that session's cwd started at the parent directory above both
repo checkouts, not inside `tsk`, so `tsk/.claude/settings.json` wasn't loaded when
`SessionStart` fired. Untested: whether it fires correctly in a session with only `tsk`
attached, where the session should be rooted directly in the repo. Check these
variables to confirm: `$TSK_BOOTSTRAP_WT` (should point at the worktree path) and
`$CLAUDE_PROJECT_DIR` (should be the `tsk` repo root).

## Current mission

**M-BOOT-02: harness.** Full briefing:
[missions/M-BOOT-02-harness.md](missions/M-BOOT-02-harness.md).

Parent mission: **M-BOOT, bootstrap tsk self hosting.** Reached when the missions and
tasks for building tsk are held in tsk's own data ref and agents execute them from there,
with no bootstrap scaffolding remaining. This is a bootstrap in the compiler sense:
reached when tsk can host its own development, not when tsk is feature complete. Full
briefing: [missions/M-BOOT.md](missions/M-BOOT.md).

From this point, all tsk development is tracked through this mission tree rather than
through the Done/Bugs/Features list this plan used before. That old backlog is preserved
below and becomes tsk's own backlog once M-BOOT-05 completes.

### Mission tree

| ID | Mission | Status | Blocked by |
|---|---|---|---|
| [M-BOOT](missions/M-BOOT.md) | Bootstrap tsk self hosting | TODO | none |
| [M-BOOT-01](missions/M-BOOT-01-substrate.md) | Substrate | ✓ DONE | none |
| [M-BOOT-02](missions/M-BOOT-02-harness.md) | Harness | IN PROGRESS | M-BOOT-01 |
| [M-BOOT-03](missions/M-BOOT-03-operation.md) | Operation | TODO | M-BOOT-02 |
| M-BOOT-04 | The official data ref | TODO | M-BOOT-03 |
| M-BOOT-05 | Migration off the bootstrap ref | TODO | M-BOOT-04 |
| [M-LAB](missions/M-LAB-ai-lab-notes.md) | AI lab notes (journalling plugin) | Skeleton, TBD fields | none |

Essential mission: M-BOOT-05. Its objective and M-BOOT's objective are the same state.

Full task breakdowns are in each mission's own briefing under `missions/`. See
[index.md](index.md) for the full directory layout, and the tsk repo's `docs/domain/`
for the design decisions behind this structure.

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
mission briefings now live in the repository's `refs/tsk/bootstrap` ref rather than a
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

