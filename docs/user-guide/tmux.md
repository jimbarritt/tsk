# Agents in tmux: the session chooser and the sidebar

`tsk tmux install` sets up tmux to show each agent's state, mission and robot in the session chooser (`prefix s`), and in a sidebar that stays open beside your work (`prefix b`). It also names each tmux session after the mission code of its thread. It needs tsk 0.45.0 or later for the sidebar, and the tsk plugin 0.3.44 or later.

## Quick start

On a machine with tmux and Homebrew, in the repo you work in:

```bash
brew install jimbarritt/tap/tsk     # or: tsk self-update
tsk tmux install
claude plugin install tsk@jimbarritt-claude-plugins --scope user
```

Install the plugin once, at user scope. For Copilot CLI, run `tsk install-plugin copilot-cli` in the repo instead.

Start a new agent session in a tmux pane. Then:

- `prefix s` opens the session chooser.
- `prefix b` opens the sidebar. `j` and `k` select a session, `Enter` switches to it, and `q` closes the sidebar.

## Install

```bash
tsk tmux install            # writes the settings and adds one line to your tmux config
tsk tmux install --print    # prints the settings and changes no file
```

The command:

1. Writes `${XDG_CONFIG_HOME:-~/.config}/tsk/tmux.conf`. tsk overwrites this file on each install, so put your own settings in your tmux config.
2. Adds a `source-file` line for that file at the end of your tmux config. The config is `$XDG_CONFIG_HOME/tmux/tmux.conf` or `~/.config/tmux/tmux.conf` when one exists, else `~/.tmux.conf`. `--tmux-conf <path>` names another. A second install finds the line and does not add it again.
3. Loads the settings at once when a tmux server runs.

The plugin's hooks set the state. Install the Claude Code plugin once, at user scope, with `claude plugin install tsk@jimbarritt-claude-plugins --scope user`, and update it with `claude plugin update tsk@jimbarritt-claude-plugins --scope user`. For Copilot CLI, run `tsk install-plugin copilot-cli` in the repo. Start a new agent session to load the new hooks.

Run `tsk tmux install` again after a tsk update that changes the settings. tsk 0.63.0 changed the chooser format to columns.

## What the chooser shows

```
(0) + M-TSK-IDEA:         ● Idea capture              Retracted Before Anyone Noticed   tsk
(1) + M-TSK-SCALE:        ◐ Large scale…              Other Robot                       tmp
(2) + notes:                                                                            root
```

| Field | Source |
|---|---|
| The tmux session name, padded to 18 characters | tmux |
| `●` waits for input, `◐` works, `✗` ended with an error | `@tsk_state`, set by the plugin's hooks |
| The mission name, cut to 24 characters and padded to 24 | `@tsk_mission_name`, set from the thread's mission |
| The robot name, padded to 32 characters | `@tsk_actor`, set from the actor of the session |
| The last component of the working directory | tmux |

The padding lines up the robot and the directory on every line (tsk 0.63.0). It needs tmux 3.2 or later. A config written by an earlier `tsk tmux install` has the format without columns: run `tsk tmux install` again. A session with no agent shows its directory alone, in the same column.

A robot that a tmux pane shows while the pane runs `claude` is taken: the robot question of a new session on this machine does not offer it. See [session-start.md](session-start.md#a-terminal-session).

## The sidebar

`prefix b` opens the sidebar: a pane on the left that lists every tmux session.

```
 tsk sessions

▸1 ● M-TSK-IDEA
     Idea capture
     Retracted Before Anyone No…

 2 ◐ M-TSK-SCALE
     Large scale…
     Other Robot
```

- `▸` marks the session you are in. The state glyphs are the same as in the chooser.
- In the sidebar, `j` and `k` (or the arrow keys) select the next and the previous session, `Enter` switches to the selected session, a digit from `1` to `9` switches to that session, and `q` closes the sidebar.
- `prefix b` opens the sidebar when it is closed, gives it the focus when it is open, and closes it when it has the focus.
- While it is open, the sidebar follows you. Two tmux hooks, `client-session-changed` and `session-window-changed`, run `join-pane`, which puts the same pane into each session and window you switch to, and the focus stays on your work. The list program keeps running, so its selection stays.
- The list refreshes every second.
- The width is 32 columns. Put `set -g @tsk_sidebar_width 40` in your tmux config, before the `source-file` line, to change it.

## When the options change

- `tsk thread start`, `resume`, `set mission`, `detach` and `stop`, and the session start, run `tsk tmux sync` on the pane. Run `tsk tmux sync` by hand after any other change, such as `tsk actor register`.
- The hooks set the state: a prompt or a tool call sets `working`; a finished turn or a notification, such as a permission prompt, sets `waiting`; the end of the session removes it.
- Outside tmux, `tsk tmux sync` and `tsk tmux state` do nothing.

## Session names

`tsk tmux sync` renames the tmux session to the mission code of the thread. A rename to a name that another session already has fails, and the session keeps its name. To keep your own session names, put `set -g @tsk_rename_session off` in your tmux config after the `source-file` line.

## Limits

- One sidebar serves the tmux server. With two clients attached to different sessions, the sidebar is in the session of the client that switched last.
- The sidebar and the hooks run `tsk` from the `PATH` of the tmux server. A server started before `tsk` was on `PATH` needs a restart.
- The options are per pane. The session line of the chooser shows the active pane of the session's active window.
- The plugin sets no Copilot hook for a permission prompt, so a Copilot pane shows `◐` while it waits for a permission answer.
- After a crash that ends the agent without a `SessionEnd` event, the pane keeps the last state. `tsk tmux state none` removes it.

## Remove

Delete the `source-file` line and the comment above it from your tmux config, and delete `${XDG_CONFIG_HOME:-~/.config}/tsk/tmux.conf`.
