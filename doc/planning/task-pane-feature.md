# Feature: Task Pane

*Planned: 2026-03-31*

---

## Overview

The TUI currently shows a single pane — the **threads pane** — listing all work threads
grouped by state and priority. This feature adds a second pane — the **task pane** — that
shows the tasks belonging to a selected thread.

The task pane is accessed by navigating into a thread from the threads pane. Pressing Esc
or Ctrl-O returns to the threads pane.

---

## Hierarchy model (future)

Each thread can contain **tasks**. Each task can contain **steps**. Each step can contain
**sub-steps**. The hierarchy stops there — no deeper nesting.

```
Thread
  └── Task
        └── Step           (future delta)
              └── Sub-step (future delta)
```

Steps and sub-steps are **deferred** — this delta implements only the task level. The data
model and UI for steps/sub-steps will be designed separately.

---

## Delta: Task Pane (this delta)

### Navigation

| Action | Effect |
|--------|--------|
| `gt` (two-key sequence) | Go to task pane for the **active thread** |
| Select thread + `Enter` | Go to task pane for the **selected thread** |
| Click on a thread row | Go to task pane for the **clicked thread** |
| `Esc` or `Ctrl-O` (in task pane) | Return to threads pane |

The threads pane gains a **selection cursor** (highlighted row) that can be moved with
`j`/`k`/arrows. The cursor is required for Enter and click navigation.

### Task pane layout

```
┌─────────────────────────────────────────┐
│  #0003 deploy-v2  PRIO                  │  ← Thread summary box
└─────────────────────────────────────────┘

> Tasks
┌──────┬───────────┬──────────────────────┐
│  1   │ ▶         │ Write migration       │  ← in-progress (top)
│  2   │ ⏳        │ Wait for review       │  ← blocked
│  3   │ ○         │ Update docs           │  ← not-started
│  4   │ ✓         │ Set up CI pipeline    │  ← done (greyed)
│  5   │ ✗         │ Old approach          │  ← cancelled (greyed)
└──────┴───────────┴──────────────────────┘
```

### Task attributes displayed

| Column | Description |
|--------|-------------|
| Index | Dynamic 1-based index. Updates when sort order changes. |
| Status | Icon representing current task state. |
| Title | Task description text. |

### Status symbols

| State | Symbol | Notes |
|-------|--------|-------|
| `not-started` | `○` | Open circle |
| `in-progress` | `▶` | Play icon |
| `blocked` | `⏳` | Hourglass (waiting/blocked) |
| `done` | `✓` | Checkmark |
| `cancelled` | `✗` | Cross |

### Sort order

Tasks are auto-sorted by state priority (not by `seq`):

1. `in-progress` — at the top
2. `blocked` — next
3. `not-started` — middle
4. `done` — bottom, greyed out
5. `cancelled` — bottom, greyed out

The dynamic index updates after sorting, so agents/humans can reference tasks by their
visible position number.

### Scrolling

The task pane is scrollable using the same keybindings as the threads pane (`j`/`k`,
`Ctrl-D`/`Ctrl-U`, `gg`/`G`, mouse wheel).

### Daemon commands

All task operations go through `tskd`:

- `tsk task create <desc> [--thread <id>]` — create a task
- `tsk task list [--thread <id>]` — list tasks
- `tsk task start <task-id> [--thread <id>]` — start a task
- `tsk task block <task-id> <reason> [--thread <id>]` — block a task
- `tsk task complete <task-id> [--thread <id>]` — complete a task
- `tsk task cancel <task-id> [--thread <id>]` — cancel a task
- `tsk task update <task-id> [flags] [--thread <id>]` — update task metadata

These commands already exist in `tskd`. The TUI calls `task.list` to fetch tasks for
the selected thread.

---

## Future deltas (not in scope)

- **Steps & sub-steps** — hierarchical breakdown within tasks. Needs data model design.
- **Task reordering** — manual drag/reorder in TUI, updating `seq` values.
- **Task creation from TUI** — inline task creation without leaving the TUI.
- **Single in-progress enforcement** — starting a new task auto-pauses others (requires
  state model change: current model has no "paused" task state).
