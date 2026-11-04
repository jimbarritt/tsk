# Agents in tmux: the session chooser

`tsk tmux install` sets up tmux to show each agent's state, mission and robot in the session chooser (`prefix s`), and names each tmux session after the mission code of its thread. It needs tsk 0.43.0 or later and the tsk plugin 0.3.44 or later.

## Install

```bash
tsk tmux install            # writes the settings and adds one line to your tmux config
tsk tmux install --print    # prints the settings and changes no file
```

The command:

1. Writes `${XDG_CONFIG_HOME:-~/.config}/tsk/tmux.conf`. tsk overwrites this file on each install, so put your own settings in your tmux config.
2. Adds a `source-file` line for that file at the end of your tmux config. The config is `$XDG_CONFIG_HOME/tmux/tmux.conf` or `~/.config/tmux/tmux.conf` when one exists, else `~/.tmux.conf`. `--tmux-conf <path>` names another. A second install finds the line and does not add it again.
3. Loads the settings at once when a tmux server runs.

Then update the plugin in each repo, so that its hooks set the state: `tsk install-plugin claude-cli` or `tsk install-plugin copilot-cli`. Start a new agent session to load the new hooks.

## What the chooser shows

```
(0) + M-TSK-IDEA: ● Idea capture  Retracted Before Anyone Noticed  tsk
(1) + M-TSK-SCALE: ◐ Large scale…  Other Robot  tmp
(2) + notes: root
```

| Field | Source |
|---|---|
| `●` waits for input, `◐` works, `✗` ended with an error | `@tsk_state`, set by the plugin's hooks |
| The mission name, cut to 24 characters | `@tsk_mission_name`, set from the thread's mission |
| The robot name | `@tsk_actor`, set from the actor of the session |
| The last component of the working directory | tmux |

A session with no agent shows its directory alone.

## When the options change

- `tsk thread start`, `resume`, `set mission`, `detach` and `stop`, and the session start, run `tsk tmux sync` on the pane. Run `tsk tmux sync` by hand after any other change, such as `tsk actor register`.
- The hooks set the state: a prompt or a tool call sets `working`; a finished turn or a notification, such as a permission prompt, sets `waiting`; the end of the session removes it.
- Outside tmux, `tsk tmux sync` and `tsk tmux state` do nothing.

## Session names

`tsk tmux sync` renames the tmux session to the mission code of the thread. A rename to a name that another session already has fails, and the session keeps its name. To keep your own session names, put `set -g @tsk_rename_session off` in your tmux config after the `source-file` line.

## Limits

- The options are per pane. The session line of the chooser shows the active pane of the session's active window.
- The plugin sets no Copilot hook for a permission prompt, so a Copilot pane shows `◐` while it waits for a permission answer.
- After a crash that ends the agent without a `SessionEnd` event, the pane keeps the last state. `tsk tmux state none` removes it.

## Remove

Delete the `source-file` line and the comment above it from your tmux config, and delete `${XDG_CONFIG_HOME:-~/.config}/tsk/tmux.conf`.
