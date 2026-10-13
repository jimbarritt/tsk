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

# The default ledger backend runs the dolt binary. Install it first. A failure
# is a warning in the session context and does not stop session start.
DOLT_MSG="$("$PLUGIN_DIR/hooks/install-dolt.sh" | tr '\n' ' ' | sed 's/ *$//')"

REQUIRED="$(tr -d '[:space:]' <"$PLUGIN_DIR/tsk-version")"

installed_version() {
  command -v tsk >/dev/null 2>&1 && tsk --version 2>/dev/null | awk '{print $NF}'
}

# A repo that builds tsk from its own source can install a newer tsk than the
# plugin requires, so any version at or above the required one is accepted.
installed_is_current() {
  local installed
  installed="$(installed_version)"
  [ -n "$installed" ] &&
    [ "$(printf '%s\n%s\n' "$REQUIRED" "$installed" | sort -V | head -n 1)" = "$REQUIRED" ]
}

if ! installed_is_current; then
  if [ -n "${TSK_SOURCE:-}" ]; then
    case "$TSK_SOURCE" in
      /*) SOURCE_PATH="$TSK_SOURCE" ;;
      *) SOURCE_PATH="${CLAUDE_PROJECT_DIR:-$PWD}/$TSK_SOURCE" ;;
    esac
    INSTALL_CMD="cargo install --path $SOURCE_PATH --locked"
  else
    INSTALL_CMD="cargo install tsk-bin --version $REQUIRED --locked"
  fi
  $INSTALL_CMD >&2 || true
fi

if ! installed_is_current; then
  emit "tsk $REQUIRED or later is required and could not be installed automatically. Run \`$INSTALL_CMD\`, then run \`tsk thread session-start </dev/null\` (or restart the session) to complete session start. $DOLT_MSG"
  exit 0
fi

if [ -z "$DOLT_MSG" ] && [ "$FORMAT" != "copilot" ]; then
  exec tsk thread session-start
fi

# Add the dolt warning to the context that tsk prints, and convert it to the
# harness's shape. tsk prints nothing when another hook already claimed the event.
TSK_OUTPUT="$(tsk thread session-start)"
[ -n "$TSK_OUTPUT" ] || exit 0
TSK_MSG="$(printf '%s' "$TSK_OUTPUT" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
emit "$(printf '%s %s' "$TSK_MSG" "$DOLT_MSG" | sed 's/^ *//; s/ *$//')"
