#!/usr/bin/env bash
set -uo pipefail

# The SessionStart hook for Claude Code and the sessionStart hook for GitHub
# Copilot CLI. `--format copilot` prints Copilot's output shape,
# {"additionalContext": "..."}. Without it, the output is Claude Code's,
# {"hookSpecificOutput": {...}}. The script finds the plugin directory from its
# own path, so it needs no harness-specific variable such as CLAUDE_PLUGIN_ROOT.
FORMAT="claude"
if [ "${1:-}" = "--format" ] && [ -n "${2:-}" ]; then
  FORMAT="$2"
fi
PLUGIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Prints one hook output object for the context in $1, in the harness's shape.
emit() {
  if [ "$FORMAT" = "copilot" ]; then
    jq -cn --arg context "$1" '{additionalContext: $context}'
  else
    jq -cn --arg context "$1" '{
      hookSpecificOutput: {
        hookEventName: "SessionStart",
        additionalContext: $context
      }
    }'
  fi
}

# The ledger runs the dolt binary. Install it first. A failure
# is a warning in the session context and does not stop session start.
DOLT_MSG="$("$PLUGIN_DIR/hooks/install-dolt.sh" | tr '\n' ' ' | sed 's/ *$//')"

# The version of this plugin, so tsk can warn when it is older than the plugin the
# binary expects. Omitted when jq or the manifest is missing.
PLUGIN_VERSION="$(jq -r '.version // empty' "$PLUGIN_DIR/.claude-plugin/plugin.json" 2>/dev/null)"
PLUGIN_VERSION_ARGS=()

REQUIRED="$(tr -d '[:space:]' <"$PLUGIN_DIR/tsk-version")"

# The tsk binary comes from the latest release, with no compile. The installer does
# nothing when the tsk first on the path is at or above the required version (a
# development build included), and prints a WARNING line on failure.
TSK_INSTALL_MSG="$("$PLUGIN_DIR/hooks/install-tsk.sh" | tr '\n' ' ' | sed 's/ *$//')"
# The installer ran in a subshell, so a PATH change it made is lost. Find the binary again.
for bin_dir in "${TSK_INSTALL_DIR:-}" "${TSK_HOMEBREW_BIN_DIR:-/opt/homebrew/bin}" /usr/local/bin "${HOME:-/nonexistent}/.local/bin"; do
  [ -n "$bin_dir" ] && [ -x "$bin_dir/tsk" ] || continue
  case ":${PATH:-}:" in *":$bin_dir:"*) ;; *) PATH="$bin_dir:${PATH:-}" ;; esac
done

installed_version() {
  command -v tsk >/dev/null 2>&1 && tsk --version 2>/dev/null | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1
}

installed_is_current() {
  local installed
  installed="$(installed_version)"
  [ -n "$installed" ] &&
    [ "$(printf '%s\n%s\n' "$REQUIRED" "$installed" | sort -t. -k1,1n -k2,2n -k3,3n | head -n 1)" = "$REQUIRED" ]
}

if ! installed_is_current; then
  emit "tsk $REQUIRED or later is required and could not be installed automatically. Run \`plugin/hooks/install-tsk.sh\` from the plugin directory or \`tsk self-update\`, then run \`tsk thread session-start </dev/null\` (or restart the session) to complete session start. $TSK_INSTALL_MSG $DOLT_MSG"
  exit 0
fi

# Pass the version only to a tsk that has the flag, so an older binary still starts.
if [ -n "$PLUGIN_VERSION" ] &&
  tsk thread session-start --help 2>/dev/null | grep -q -- '--plugin-version'; then
  PLUGIN_VERSION_ARGS=(--plugin-version "$PLUGIN_VERSION")
fi

if [ -z "$DOLT_MSG$TSK_INSTALL_MSG" ] && [ "$FORMAT" != "copilot" ]; then
  exec tsk thread session-start ${PLUGIN_VERSION_ARGS[@]+"${PLUGIN_VERSION_ARGS[@]}"}
fi

# Add the dolt and install warnings to the context that tsk prints, and convert it to the
# harness's shape. tsk prints nothing when another hook already claimed the event.
TSK_OUTPUT="$(tsk thread session-start ${PLUGIN_VERSION_ARGS[@]+"${PLUGIN_VERSION_ARGS[@]}"})"
[ -n "$TSK_OUTPUT" ] || exit 0
TSK_MSG="$(printf '%s' "$TSK_OUTPUT" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
emit "$(printf '%s %s %s' "$TSK_MSG" "$TSK_INSTALL_MSG" "$DOLT_MSG" | sed 's/^ *//; s/ *$//')"
