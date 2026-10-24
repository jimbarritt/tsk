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

# `.claude/settings.json`'s `extraKnownMarketplaces` and `enabledPlugins` only
# declare intent; neither registers the marketplace with the CLI, installs the
# plugin, nor upgrades an already-installed one. All steps below are needed,
# in order, so a fresh clone (a cloud session included) gets a working,
# current plugin with no manual step. Each is idempotent: running it again
# when already done is a no-op that exits 0. `install` on an already-installed
# plugin does not upgrade it even when the marketplace offers a newer
# version, so a separate `update` step is required, after a marketplace
# refresh so it sees a version newer than the cached one.
#
# The steps cost about 4 seconds for each plugin, which is most of a session
# start, and their effect is for the next Claude Code process: this process
# has read its plugins already. So:
#
# - Both plugins come from one marketplace, which is added once and refreshed
#   once, not once for each plugin. A failure still gives each plugin the
#   warning it gave before.
# - The steps run in the background, while the dolt check and the tsk session
#   start below run. The script waits for them before it prints, so a warning
#   reaches the context as before.
# - After a run with no warning, the steps are skipped for the next
#   TSK_PLUGIN_ENSURE_MINUTES minutes (60 when unset; 0 never skips). A newer
#   plugin version is then picked up within that time.
PLUGIN_LOG="$(mktemp)"
PLUGIN_MSG_FILE="$(mktemp)"
PLUGIN_STAMP="${XDG_STATE_HOME:-${HOME:-/nonexistent}/.local/state}/tsk/plugin-ensure-ok"

plugin_steps_are_due() {
  local minutes="${TSK_PLUGIN_ENSURE_MINUTES:-60}"
  [ "$minutes" -gt 0 ] 2>/dev/null || return 0
  [ -f "$PLUGIN_STAMP" ] || return 0
  [ -z "$(find "$PLUGIN_STAMP" -mmin "-$minutes" 2>/dev/null)" ]
}

# Prints one line per warning, in the shape the context carries.
ensure_plugins() {
  local marketplace_source="$1" marketplace="$2"
  shift 2
  local plugins=("$@") plugin failed="" warned=0
  warn() {
    printf ' WARNING: %s\n' "$1"
    warned=1
  }
  if ! claude plugin marketplace add "$marketplace_source" >"$PLUGIN_LOG" 2>&1; then
    for plugin in "${plugins[@]}"; do
      warn "adding the $marketplace marketplace failed: $(tr '\n' ' ' <"$PLUGIN_LOG")"
    done
    return
  fi
  for plugin in "${plugins[@]}"; do
    if ! claude plugin install "$plugin@$marketplace" --scope project -y >"$PLUGIN_LOG" 2>&1; then
      warn "installing the $plugin plugin failed: $(tr '\n' ' ' <"$PLUGIN_LOG")"
      failed="$failed $plugin"
    fi
  done
  local refreshed=1
  if ! claude plugin marketplace update "$marketplace" >"$PLUGIN_LOG" 2>&1; then
    refreshed=0
    for plugin in "${plugins[@]}"; do
      case " $failed " in *" $plugin "*) continue ;; esac
      warn "refreshing the $marketplace marketplace cache failed: $(tr '\n' ' ' <"$PLUGIN_LOG")"
    done
  fi
  if [ "$refreshed" = 1 ]; then
    for plugin in "${plugins[@]}"; do
      case " $failed " in *" $plugin "*) continue ;; esac
      if ! claude plugin update "$plugin@$marketplace" --scope project >"$PLUGIN_LOG" 2>&1; then
        warn "updating the $plugin plugin failed: $(tr '\n' ' ' <"$PLUGIN_LOG")"
      fi
    done
  fi
  if [ "$warned" = 0 ]; then
    mkdir -p "$(dirname "$PLUGIN_STAMP")" 2>/dev/null && : >"$PLUGIN_STAMP" 2>/dev/null
  fi
}

PLUGIN_JOB=""
if plugin_steps_are_due; then
  ensure_plugins jimbarritt/claude-plugins jimbarritt-claude-plugins swe tsk >"$PLUGIN_MSG_FILE" &
  PLUGIN_JOB=$!
fi

# Claude Code reads plugin hooks once, when its process starts. A plugin
# installed or updated above has no hooks in this process, and `/clear`
# starts a new session in the same process without reading them again. So
# this script runs the tsk session start itself, every time, rather than
# depending on the plugin's hook. When the plugin's hook also runs, the
# binary claims the event by session ID and source, and the second run exits
# with no output.
TSK_MSG=""
# tsk's ledger runs the dolt binary, so install it before tsk
# runs. The installer prints a WARNING on failure and always exits 0.
DOLT_MSG="$("$REPO_ROOT/plugin/hooks/install-dolt.sh" | tr '\n' ' ')"
# tsk comes from the latest release, not from this checkout, which a resumed session
# can leave days behind. The installer does nothing when the tsk first on the path is at
# or above plugin/tsk-version, so a development build from `just build-install` stays.
TSK_INSTALL_MSG="$("$REPO_ROOT/plugin/hooks/install-tsk.sh" | tr '\n' ' ')"
# The installer ran in a subshell, so a PATH change it made is lost. Find the binary again.
for bin_dir in "${TSK_INSTALL_DIR:-}" /usr/local/bin "${HOME:-/nonexistent}/.local/bin"; do
  [ -n "$bin_dir" ] && [ -x "$bin_dir/tsk" ] || continue
  case ":$PATH:" in *":$bin_dir:"*) ;; *) PATH="$bin_dir:$PATH" ;; esac
done
if command -v tsk >/dev/null 2>&1; then
  TSK_OUTPUT="$(printf '%s' "$INPUT" | tsk thread session-start)"
  TSK_MSG="$(printf '%s' "$TSK_OUTPUT" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
else
  TSK_MSG="WARNING: tsk is not installed. Run \`plugin/hooks/install-tsk.sh\`, then run \`tsk thread session-start </dev/null\`."
fi

# The plugin steps end before the context is printed, so a warning reaches it.
if [ -n "$PLUGIN_JOB" ]; then
  wait "$PLUGIN_JOB" 2>/dev/null || true
fi
PLUGIN_MSG="$(tr '\n' ' ' <"$PLUGIN_MSG_FILE" | sed 's/ *$//')"
rm -f "$PLUGIN_LOG" "$PLUGIN_MSG_FILE"

CONTEXT="$(printf '%s %s' "$TSK_MSG" "$TSK_INSTALL_MSG $PLUGIN_MSG $DOLT_MSG" | sed 's/^ *//; s/ *$//')"
if [ -n "$CONTEXT" ]; then
  jq -n --arg context "$CONTEXT" '{
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: $context
    }
  }'
fi
