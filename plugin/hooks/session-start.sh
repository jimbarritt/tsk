#!/usr/bin/env bash
set -uo pipefail

# The default ledger backend runs the dolt binary. Install it first. A failure
# is a warning in the session context and does not stop session start.
DOLT_MSG="$("${CLAUDE_PLUGIN_ROOT}/hooks/install-dolt.sh" | tr '\n' ' ' | sed 's/ *$//')"

REQUIRED="$(tr -d '[:space:]' <"${CLAUDE_PLUGIN_ROOT}/tsk-version")"

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
  MESSAGE="tsk $REQUIRED or later is required and could not be installed automatically. Run \`$INSTALL_CMD\`, then run \`tsk thread session-start </dev/null\` (or restart the session) to complete session start. $DOLT_MSG"
  ESCAPED="$(printf '%s' "$MESSAGE" | sed 's/\\/\\\\/g; s/"/\\"/g')"
  printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$ESCAPED"
  exit 0
fi

if [ -n "$DOLT_MSG" ]; then
  # Add the dolt warning to the context that tsk prints.
  TSK_OUTPUT="$(tsk thread session-start)"
  TSK_MSG="$(printf '%s' "$TSK_OUTPUT" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
  jq -n --arg context "$TSK_MSG $DOLT_MSG" '{
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: $context
    }
  }'
  exit 0
fi

exec tsk thread session-start
