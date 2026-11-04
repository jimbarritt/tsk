# tmux sessions named by mission, and a session chooser with agent state

Status: research note, 2026-11-04. tmux can show, in its session chooser, the worktree, the agent name and whether the agent waits for input. tmux reads the worktree itself. A hook sets the agent name and the state as per-session or per-pane user options. Tested on tmux 3.4.

Built in tsk 0.43.0 as `tsk tmux install`, `sync` and `state`: [docs/user-guide/tmux.md](../user-guide/tmux.md).

Raised by Jim, 2026-11-04: name tmux sessions after missions, which is close to the iOS app, and customise the session switching popup to show more information. The idea belongs to Mission Control (M-TSK-BOOT-06), and may belong in core tsk.

## Session names

- `tmux new-session -s M-TSK-IDEA` names a session, and `tmux rename-session -t <old> M-TSK-IDEA` renames one.
- tmux replaces `.` and `:` in a session name with `_`. A mission code uses letters, digits and hyphens, so it stays as typed.
- The default status line shows the name through `status-left` (`[#S] `), cut to `status-left-length`, 10 by default. The test showed `[M-TSK-IDE`. Set `status-left-length` to 30 or more.
- `switch-client -t M-TSK-IDEA` and `attach -t M-TSK-IDEA` take the name, so a mission code is also the address of its session.

## The session chooser

`prefix s` runs `choose-tree -Zs`. It lists sessions, and shows a live preview of the selected session's active pane. `-F` replaces the text after each item's name with a format string.

Options that apply here:

| Option | Effect |
|---|---|
| `-F <format>` | The text of each line |
| `-s` | Start with sessions collapsed |
| `-Z` | Zoom the pane while the chooser runs |
| `-O index\|name\|time` | Sort order |
| `-f <filter>` | Show only the items whose filter format is true, such as only the sessions that wait for input |

A binding in `~/.tmux.conf` replaces the default:

```tmux
bind s choose-tree -Zs -F '#{?#{==:#{@tsk_state},waiting},WAIT,#{?#{==:#{@tsk_state},working},busy,-}}  #{@tsk_actor}  #{b:pane_current_path}'
```

The test rendered these lines with that format:

```
(0) + M-TSK-IDEA: WAIT  Retracted Before Anyone Noticed  tsk
(1) + M-TSK-SCALE: busy  Other Robot  tmp
```

The preview below the list shows the agent's pane as it is, so a prompt that waits for an answer is visible there too.

## The source of each field

**The worktree.** `#{pane_current_path}` is the working directory of the active pane, and `#{b:pane_current_path}` its last component. tmux reads it from the process, so it needs no setup. An agent that changes directory in a subshell does not change it.

**The agent name.** A user option, a name that starts with `@`, set on the session (`tmux set -t M-TSK-IDEA @tsk_actor "<name>"`) or on the pane (`tmux set -p -t "$TMUX_PANE" @tsk_actor "<name>"`). A format reads a pane option first, then the window, the session and the global value. The tsk skills already run `tsk thread title` at thread start, resume and mission switch, so the same step can set the option.

**Whether the agent waits for input.** A user option such as `@tsk_state`, set by the harness's hooks. Mission Control T-06 settled the mapping for Claude Code:

| Event | State |
|---|---|
| `Notification`, `PermissionRequest`, `Stop` | waiting |
| `UserPromptSubmit`, `PostToolUse` | working (clears waiting) |
| `SessionEnd` with `reason` `error` | error |

The hook command is one line: `tmux set -p -t "$TMUX_PANE" @tsk_state waiting`. `$TMUX_PANE` is in the environment of every process in a tmux pane, so the hook needs no lookup. Per pane is the safer scope, because a session can contain more than one agent.

## A popup in place of the chooser

`display-popup` (tmux 3.2 and later) runs a command in a floating window, so a binding can open a chooser written as a program:

```tmux
bind s display-popup -E -w 80% -h 60% '<a program that lists sessions and switches>'
```

A program can read the ledger as well as tmux: the mission's status, the thread, the open questions. A `choose-tree` format can read only tmux values. The cost is the live preview, which `choose-tree` gives and a program has to rebuild, for example with `fzf --preview 'tmux capture-pane -ep -t {1}'`.

## Fit with ADR 0013

This section is the note's own inference.

ADR 0013 makes tmux one harness among several and keeps Mission Control a separate product. It allows a split here: tsk core prints the values (mission code, actor name, thread state) through a command, and a thin tmux adapter sets the options and the binding. The harness's hooks set the waiting state, not the ledger, so it stays in the adapter.

## The activity indicator in the session title

Jim started an activity indicator in the session title, and it needs tightening. Its code was not read for this note: the `jimbarritt/tsk-mission-control` repository was not available to this session. The options above give an indicator that the hooks set and the status line or chooser reads, in place of one written into a title.

## Not established

- Whether `#[fg=...]` style sequences render inside a `choose-tree -F` line. Not tested.
- Whether GitHub Copilot's CLI has hook events that map to the same states. Needed for T-10 of M-TSK-IDEA, a robot identity across harnesses.
- How a state stays correct after a crash, when no clearing event fires.

## References

- tmux manual, `choose-tree`, `display-popup`, FORMATS and user options: https://man.openbsd.org/tmux
- Mission Control report, T-06, on the tsk/docs branch: `missions/operational/M-TSK-BOOT-06/M-TSK-BOOT-06-mission-control-report.8be6d5e5.md`
- ADR 0013: [0013-tsk-works-in-any-harness-mission-control-stays-separate.md](../adr/0013-tsk-works-in-any-harness-mission-control-stays-separate.md)
