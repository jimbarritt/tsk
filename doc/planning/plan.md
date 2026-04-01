# tsk — plan

## Done

### Threads
- `thread create`, `thread list`, `thread switch-to`
- `thread update` (slug, description, priority — renames dir + updates index.md)
- `thread wait` / `thread resume` — Waiting state with optional reason
- Thread directory scaffolded with `index.md` on create; updated on slug/priority/description change

### Tasks
- `task create`, `task list`, `task start`, `task block`, `task complete`, `task cancel`, `task update`
- State machine: `not-started → in-progress → done`; `in-progress ↔ blocked`; any → `cancelled`
- Diversion pattern: `--thread <id>` on all task commands targets a non-active thread without switching

### TUI
- Thread list grouped by section (Active / Priority & Incidents / Background)
- Vim keybindings: `j`/`k`, `ctrl-d`/`ctrl-u`, `gg`/`G`
- Mouse scroll
- Status bar: active thread id + slug, `? help` hint
- Help popup: `?` toggles keybinding overlay

### CLI / infra
- `--version` / `-V` on `tsk` and `tskd`
- `tsk context` outputs `agent-context.md`
- JSON output throughout

---

## Bugs

| ID    | Severity | Description |
|-------|----------|-------------|
| BUG-1 | crash    | TUI panics on terminal resize when description contains multi-byte characters (e.g. em dash). Root cause: byte indexing instead of char indexing for truncation. Also: terminal not restored on panic (need panic hook). |
| BUG-2 | visual   | Right-hand column border misaligned for rows with truncated slugs — leading `│` of next row is consumed into the slug cell. Same underlying byte-width accounting issue as BUG-1. |
| BUG-3 | visual   | Missing `│` separator after the ID column — ID and slug columns run together. |
| BUG-4 | usability | Responsive column hiding not implemented. At narrow widths everything is crushed. Should progressively hide: description → state → show only id/slug/priority. |
| BUG-5 | medium   | TUI does not update in real time when thread state changes from outside (e.g. agent calling CLI in another pane). Requires TUI restart. Fix: watch `tsk/threads/index.json` for changes and re-render. |

---

## Features / next work

### TUI — task view
The TUI currently shows threads only. Tasks exist in the CLI but are not visible in the TUI.

- Show tasks for the selected/active thread in the TUI
- Keyboard shortcut to toggle task view (missing from help popup — the original issue)
- FR-2: Auto-focus task view when `switch-to` is called — show thread context immediately after a context switch. `ctrl-o` to navigate back to thread list.

### FR-6 — Cross-thread "today" view
Surface urgent tasks across all threads in a single view.
- Flag a task as urgent (priority flag, due-date of today, or `tsk task flag <id> urgent`)
- `tsk today` — flat list of urgent/due-today tasks across all threads
- TUI today view as an alternative tab/pane

### Configuration file
`tsk.toml` or `.tskrc` — per-project and per-user settings.
First planned setting: `show_status_bar = true/false`.
