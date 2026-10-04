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

The plugin is installed from a marketplace entry of the `git-subdir` form, with `path`
set to `plugin`. See [docs/kb/claude-code-plugin-packaging.md](../docs/kb/claude-code-plugin-packaging.md).

The thread skills and the `SessionStart` and `Stop` hooks move here from `.claude/` at
the M-BOOT-04 cut-over (T-07), when they call `tsk` in place of the `ops/local` scripts.
