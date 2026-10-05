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

# `.claude/settings.json`'s `extraKnownMarketplaces` and `enabledPlugins` only
# declare intent; neither registers the marketplace with the CLI, installs the
# plugin, nor upgrades an already-installed one. All steps below are needed,
# in order, so a fresh clone (a cloud session included) gets a working,
# current plugin with no manual step. Each is idempotent: running it again
# when already done is a no-op that exits 0. `install` on an already-installed
# plugin does not upgrade it even when the marketplace offers a newer
# version, so a separate `update` step is required, after a marketplace
# refresh so it sees a version newer than the cached one.
PLUGIN_MSG=""
PLUGIN_LOG="$(mktemp)"

ensure_plugin() {
  local marketplace_source="$1" marketplace="$2" plugin="$3"
  if ! claude plugin marketplace add "$marketplace_source" >"$PLUGIN_LOG" 2>&1; then
    PLUGIN_MSG="$PLUGIN_MSG WARNING: adding the $marketplace marketplace failed: $(tr '\n' ' ' <"$PLUGIN_LOG")"
  elif ! claude plugin install "$plugin@$marketplace" --scope project -y >"$PLUGIN_LOG" 2>&1; then
    PLUGIN_MSG="$PLUGIN_MSG WARNING: installing the $plugin plugin failed: $(tr '\n' ' ' <"$PLUGIN_LOG")"
  elif ! claude plugin marketplace update "$marketplace" >"$PLUGIN_LOG" 2>&1; then
    PLUGIN_MSG="$PLUGIN_MSG WARNING: refreshing the $marketplace marketplace cache failed: $(tr '\n' ' ' <"$PLUGIN_LOG")"
  elif ! claude plugin update "$plugin@$marketplace" --scope project >"$PLUGIN_LOG" 2>&1; then
    PLUGIN_MSG="$PLUGIN_MSG WARNING: updating the $plugin plugin failed: $(tr '\n' ' ' <"$PLUGIN_LOG")"
  fi
}

ensure_plugin jimbarritt/claude-plugins jimbarritt-claude-plugins swe
ensure_plugin "$REPO_ROOT" tsk tsk
rm -f "$PLUGIN_LOG"

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

# Claude Code reads plugin hooks once, when its process starts. A plugin
# installed or updated above has no hooks in this process, and `/clear`
# starts a new session in the same process without reading them again. So
# this script runs the tsk session start itself, every time, rather than
# depending on the plugin's hook. When the plugin's hook also runs, the
# binary claims the event by session ID and source, and the second run exits
# with no output. This repo builds tsk from its own source, so a running
# container picks up a new tsk on its next session start.
TSK_MSG=""
WORKSPACE_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$REPO_ROOT/Cargo.toml" | head -n 1)"
INSTALLED_VERSION="$(command -v tsk >/dev/null 2>&1 && tsk --version 2>/dev/null | awk '{print $NF}')"
if [ "$INSTALLED_VERSION" != "$WORKSPACE_VERSION" ]; then
  cargo install --path "$REPO_ROOT/cli" --locked >&2 || true
fi
if command -v tsk >/dev/null 2>&1; then
  TSK_OUTPUT="$(printf '%s' "$INPUT" | tsk thread session-start)"
  TSK_MSG="$(printf '%s' "$TSK_OUTPUT" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
else
  TSK_MSG="WARNING: tsk is not installed and \`cargo install --path cli --locked\` failed. Run it, then run \`tsk thread session-start </dev/null\`."
fi

CONTEXT="$(printf '%s %s' "$TSK_MSG" "$PLUGIN_MSG" | sed 's/^ *//; s/ *$//')"
if [ -n "$CONTEXT" ]; then
  jq -n --arg context "$CONTEXT" '{
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: $context
    }
  }'
fi
