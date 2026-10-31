# tsk plugin

The tsk plugin for Claude Code and GitHub Copilot CLI. It contains the hooks and skills that run the tsk harness
in a session. They call the `tsk` binary and nothing else, so they have no dependency on
this repository. They contain the minimum logic: see
[ADR 0011](../docs/adr/0011-logic-lives-in-the-binary-not-the-plugin.md).

| Component | Location |
|---|---|
| Manifest | `.claude-plugin/plugin.json` |
| Skills | `skills/<name>/SKILL.md` |
| Hooks, Claude Code | `hooks/hooks.json` |
| Hooks, GitHub Copilot CLI | `hooks.json` |

| Skill | Runs |
|---|---|
| `tsk` | Reference for the `tsk` command line: records (missions, tasks, decisions, questions), documents, threads. |
| `start-thread` | `tsk thread start`, only for a harness instance with no bound thread (otherwise use `switch-mission` or `resume-thread`), `tsk actor join` for a supervising human, with a mission ID or with no mission; `tsk thread skip` to work with no thread |
| `pause-thread` | `tsk thread pause` |
| `resume-thread` | `tsk thread resume` |
| `switch-thread` | `tsk thread detach`, `tsk thread stop` when the line of work is finished, then the `resume-thread` skill |
| `switch-mission` | `tsk thread pause` on the current focus, `tsk thread set mission`, and the session title: changes the focus of the bound thread and keeps the thread |
| `detach-thread` | `tsk thread detach`, then `tsk thread skip` if asked |
| `stop-thread` | `tsk thread stop` |
| `register-repo` | `tsk nexus register-repo` |
| `construct-robot` | `tsk actor list`, then `tsk actor register --kind robot` with a name the robot picks in the style of the Culture ships (the actor becomes the primary actor of the session), `tsk ledger push`, and the session title |
| `regenerate-cloud-instance` | `tsk thread pause`, `tsk ledger push`, then `create_session` for a new cloud session that resumes the thread and joins the same robot, and a `to archive-` prefix on the old session's title |

| Hook | Runs |
|---|---|
| `SessionStart` | `hooks/session-start.sh`: `tsk thread session-start` |
| `Stop` | `hooks/stop.sh`: `tsk thread guard`. Asks for a thread when none is bound. When the ledger store contains unpushed changes it publishes them in the background (`tsk ledger push --background`) and prints nothing; a failure shows as a warning on the next `tsk` command |
| `PreToolUse` | `hooks/pre-tool-use.sh`: `tsk ledger guard`. Denies a `Bash` call that runs `git` or `dolt` against the ledger store, its refs (`tsk/ledger`, `tsk/docs`, `refs/tsk/*`) or its branches, and an `Edit`, `Write` or `NotebookEdit` call on a file in the ledger store (ADR 0028). Allows every `tsk` command. Fails open: a missing or older `tsk` allows the call. `TSK_LEDGER_GUARD=off` in the environment that starts the agent switches it off |
| `SessionEnd` | `hooks/session-end.sh`: `tsk ledger push "End of session"` under a timeout when `tsk ledger status --local` shows unpushed work. Best effort, always exits 0 |

Records are published at thread start, thread pause or stop, the end of the session and
when asked ([ADR 0019](../docs/adr/0019-publish-less-often-by-rule.md)).

The `SessionStart` hook passes the plugin's own version to `tsk thread session-start
--plugin-version`. When that version is lower than the plugin version the binary requires,
the session context includes a request for a plugin update (`/plugin update tsk`). The expected version
is the constant `EXPECTED_PLUGIN_VERSION` in `cli/src/thread/session_start.rs`. Bump it
with the `version` in `.claude-plugin/plugin.json`: a unit test fails when they differ.

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
- `sessionEnd` runs `hooks/session-end.sh`.
- The hook commands find the plugin directory from `PLUGIN_ROOT`, `COPILOT_PLUGIN_ROOT`
  or `CLAUDE_PLUGIN_ROOT`, and otherwise from Copilot's `installed-plugins` directory.
  The scripts find their own directory, so they read no harness variable.

Copilot installs the plugin from the marketplace in this repo,
`.github/plugin/marketplace.json`, named `tsk`, as `tsk@tsk`. Copilot does not accept the
`git-subdir` source of the `jimbarritt/claude-plugins` entry. `tsk install-plugin
copilot-cli` runs the install.

