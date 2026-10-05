#!/usr/bin/env bash
set -uo pipefail

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
  MESSAGE="tsk $REQUIRED or later is required and could not be installed automatically. Run \`$INSTALL_CMD\`, then run \`tsk thread session-start </dev/null\` (or restart the session) to complete session start."
  ESCAPED="$(printf '%s' "$MESSAGE" | sed 's/\\/\\\\/g; s/"/\\"/g')"
  printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$ESCAPED"
  exit 0
fi

exec tsk thread session-start
