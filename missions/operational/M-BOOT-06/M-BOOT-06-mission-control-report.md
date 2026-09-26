# Report: M-BOOT-06, Mission Control

| Field | Value |
|---|---|
| Mission | M-BOOT-06 |
| Outcome | In progress |
| Actor | Cloud session, Sonnet 5 |
| Date started | 2026-09-25 |

## Outcome

In progress. This report is written as execution proceeds, per the mission model's rule
that a report takes addenda rather than being written only at the end.

## Decisions made during execution

**T-05 key bindings, approved by Jim 2026-09-25/26.** `j`/`k` move the list selection,
`Enter` switches the right pane to the selected session, `n` starts a new session with a
name prompt defaulting to the current git repo name. Vim style, matching the vim
keybindings already recorded for tsk's own TUI in the ledger's legacy backlog.

**T-08 key bindings, approved by Jim 2026-09-25/26.** `e` opens nvim, zoomed, in the
selected session's worktree. `prefix+v` (tmux prefix, `Ctrl-b` by default) restores the
three-pane layout, hiding nvim while it keeps running.

Reasoning for `prefix+v` over the two alternatives considered:
- `prefix+z` was ruled out before it was proposed: it is tmux's own real pane-zoom
  toggle, and nvim in this design runs in a dedicated window, not a zoomed pane, so
  reusing that key would be misleading as well as functionally wrong.
- `prefix+r` was proposed first and rejected. Stock tmux binds `prefix+r` by default to
  `refresh-client` (force a redraw). Scoping the override to the Mission Control
  session only, so `refresh-client` keeps working in every other tmux session on the
  machine, was offered as a way to keep `r`. Jim chose the second option instead: pick a
  letter tmux does not bind by default, so nothing is ever overridden, anywhere.
- `prefix+v` has no default binding in stock tmux, so it introduces no clash and needs
  no session-scoped key table.

**Opus review before starting.** An Opus 5.5 subagent reviewed the plan before work
began, per the mission's instruction to consult one when deciding how to proceed. Its
recommendations, folded into the plan below: pane IDs (`%N`) rather than indices; nvim as
a dedicated tmux window rather than a popup, so it survives independently of the
Mission Control app; state files under
`${XDG_STATE_HOME:-~/.local/state}/tsk-mission-control/`, keyed by tmux session ID; a
poll-based list redraw (`st_mtime_ns` comparison) rather than an OS-specific
file-watching dependency, per the macOS/Linux-parity constraint; and several caveats on
Claude Code hook events for T-06, recorded there rather than here.
