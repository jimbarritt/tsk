# tsk plugin

The Claude Code plugin for tsk. It holds the hooks and skills that run the tsk harness
in a session. They call the `tsk` binary and nothing else, so they have no dependency on
this repository. They hold the minimum logic: see
[ADR 0011](../docs/adr/0011-logic-lives-in-the-binary-not-the-plugin.md).

| Component | Location |
|---|---|
| Manifest | `.claude-plugin/plugin.json` |
| Skills | `skills/<name>/SKILL.md` |
| Hooks | `hooks/hooks.json` |

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
