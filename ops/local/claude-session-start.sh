#!/usr/bin/env bash
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

# A shallow clone truncates history at an arbitrary boundary and marks the
# commit there as if it had no parent. Two shallow fetches at different times
# can truncate at two different points, so a later comparison between them
# (e.g. local main against origin/main) finds no common ancestor and looks
# exactly like a rewritten, unrelated history. It is neither: the real
# history is continuous on GitHub, just not present in this clone. Unshallow
# once, here, so no session ever has to tell the two apart.
if [ "$(git rev-parse --is-shallow-repository 2>/dev/null)" = "true" ]; then
  git fetch --unshallow origin >/dev/null 2>&1 || true
fi

INPUT="$(cat)"
SOURCE="$(printf '%s' "$INPUT" | jq -r '.source // empty')"

if [ "$SOURCE" = "startup" ]; then
  git stash push -u -m "session-start-autostash" >/dev/null 2>&1 || true
  git checkout main >/dev/null 2>&1 || true
  git pull origin main >/dev/null 2>&1 || true
  if git stash list 2>/dev/null | grep -q "session-start-autostash"; then
    git stash apply >/dev/null 2>&1 || true
  fi
fi

# The tsk and swe plugins belong at user scope: a cloud environment's setup script
# (ops/cloud/setup-environment.sh) installs them there, and on any other machine a person
# installs them once. This script installs nothing. It checks
# installed_plugins.json, and for each plugin with no user-scope install it adds a
# WARNING line with the install command to the context.
PLUGIN_MSG=""
PLUGINS_FILE="${CLAUDE_CONFIG_DIR:-${HOME:-/nonexistent}/.claude}/plugins/installed_plugins.json"
for plugin in tsk swe; do
  key="$plugin@jimbarritt-claude-plugins"
  if ! jq -e --arg k "$key" 'any(.plugins[$k][]?; .scope == "user")' "$PLUGINS_FILE" >/dev/null 2>&1; then
    PLUGIN_MSG="$PLUGIN_MSG WARNING: the $plugin plugin is not installed at user scope. Ask the person to install it: claude plugin marketplace add jimbarritt/claude-plugins && claude plugin install $key --scope user, then start a new session."
  fi
done

# Claude Code reads plugin hooks once, when its process starts. A plugin
# installed or updated during start-up has no hooks in this process, and `/clear`
# starts a new session in the same process without reading them again. So
# this script runs the tsk session start itself, every time, rather than
# depending on the plugin's hook. When the plugin's hook also runs, the
# binary claims the event by session ID and source, and the second run exits
# with no output.
TSK_MSG=""
# A cloud container runs its environment's set-up again when the set-up script on main
# changed since its disk was saved. It runs before the installers, which then bring dolt
# and tsk up to the versions this checkout needs.
SETUP_MSG="$("$REPO_ROOT/plugin/hooks/refresh-setup.sh" | tr '\n' ' ')"
# tsk's ledger runs the dolt binary, so install it before tsk
# runs. The installer prints a WARNING on failure and always exits 0.
DOLT_MSG="$("$REPO_ROOT/plugin/hooks/install-dolt.sh" | tr '\n' ' ')"
# tsk comes from the latest release, not from this checkout, which a resumed session
# can leave days behind. The installer does nothing when the tsk first on the path is at
# or above plugin/tsk-version, so a development build from `just build-install` stays.
TSK_INSTALL_MSG="$("$REPO_ROOT/plugin/hooks/install-tsk.sh" | tr '\n' ' ')"
# The installer ran in a subshell, so a PATH change it made is lost. Find the binary again.
for bin_dir in "${TSK_INSTALL_DIR:-}" "${TSK_HOMEBREW_BIN_DIR:-/opt/homebrew/bin}" /usr/local/bin "${HOME:-/nonexistent}/.local/bin"; do
  [ -n "$bin_dir" ] && [ -x "$bin_dir/tsk" ] || continue
  case ":${PATH:-}:" in *":$bin_dir:"*) ;; *) PATH="$bin_dir:${PATH:-}" ;; esac
done
if command -v tsk >/dev/null 2>&1; then
  TSK_OUTPUT="$(printf '%s' "$INPUT" | tsk thread session-start)"
  TSK_MSG="$(printf '%s' "$TSK_OUTPUT" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
else
  TSK_MSG="WARNING: tsk is not installed. Run \`plugin/hooks/install-tsk.sh\`, then run \`tsk thread session-start </dev/null\`."
fi

CONTEXT="$(printf '%s %s' "$TSK_MSG" "$SETUP_MSG $TSK_INSTALL_MSG $PLUGIN_MSG $DOLT_MSG" | sed 's/^ *//; s/ *$//')"
if [ -n "$CONTEXT" ]; then
  jq -n --arg context "$CONTEXT" '{
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: $context
    }
  }'
fi
