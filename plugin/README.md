# tsk plugin

The tsk plugin for Claude Code and GitHub Copilot CLI. It holds the hooks and skills that run the tsk harness
in a session. They call the `tsk` binary and nothing else, so they have no dependency on
this repository. They hold the minimum logic: see
[ADR 0011](../docs/adr/0011-logic-lives-in-the-binary-not-the-plugin.md).

| Component | Location |
|---|---|
| Manifest | `.claude-plugin/plugin.json` |
| Skills | `skills/<name>/SKILL.md` |
| Hooks, Claude Code | `hooks/hooks.json` |
| Hooks, GitHub Copilot CLI | `hooks.json` |

| Skill | Runs |
|---|---|
| `tsk` | Reference for the `tsk` command line. |
| `start-thread` | `tsk thread start` |
| `pause-thread` | `tsk thread pause` |
| `resume-thread` | `tsk thread resume` |
| `switch-thread` | `tsk thread detach`, `tsk thread stop` when asked, then the `resume-thread` skill |
| `detach-thread` | `tsk thread detach` |
| `stop-thread` | `tsk thread stop` |
| `register-repo` | `tsk nexus register-repo` |

The plugin is installed from the `jimbarritt/claude-plugins` marketplace, as
`tsk@jimbarritt-claude-plugins`. Its entry has the `git-subdir` form, with `url` set to
this repo and `path` set to `plugin`. See [docs/kb/claude-code-plugin-packaging.md](../docs/kb/claude-code-plugin-packaging.md).

## GitHub Copilot CLI

Copilot CLI reads the same manifest and skills. Its hooks are in `hooks.json` at the
plugin root, in Copilot's format (`version` 1, the events `sessionStart` and
`agentStop`). Claude Code reads only `hooks/hooks.json`.

- `sessionStart` runs `hooks/session-start.sh --format copilot`, which prints Copilot's
  output shape, `{"additionalContext": "..."}`.
- `agentStop` runs `hooks/stop.sh`. `tsk thread guard` prints `decision` and `reason`,
  the fields Copilot's `agentStop` reads.
- The hook commands find the plugin directory from `PLUGIN_ROOT`, `COPILOT_PLUGIN_ROOT`
  or `CLAUDE_PLUGIN_ROOT`, and otherwise from Copilot's `installed-plugins` directory.
  The scripts find their own directory, so they read no harness variable.

Copilot installs the plugin from the marketplace in this repo,
`.github/plugin/marketplace.json`, named `tsk`, as `tsk@tsk`. Copilot does not accept the
`git-subdir` source of the `jimbarritt/claude-plugins` entry. `tsk install-plugin
copilot-cli` runs the install.

