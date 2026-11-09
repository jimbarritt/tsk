#!/usr/bin/env bash
set -uo pipefail

# Runs the cloud environment's set-up again when ops/cloud/setup-environment.sh on main
# differs from the copy that last ran in this container.
#
# A cloud session starts from a disk that the platform saved after the environment's
# setup script ran. The platform runs that script again only when it builds the
# environment again, and a change to setup-environment.sh on main does not cause a build.
# This script closes that gap at session start:
#
# - It runs only where setup-environment.sh ran: /usr/local/share/tsk-setup/last-run
#   exists. A local machine has no such file, so the script exits with no output.
# - It fetches setup-environment.sh from main and compares its SHA-256 with
#   script.sha256 in the state directory.
# - When they differ, it runs the fetched script with the options in the state directory's
#   args file (one option per line, written by setup-environment.sh), then writes the new
#   SHA-256. A disk saved before the args file existed has none: robot attribution in the
#   user tsk config then gives --use-robot-attribution <email>.
# - Then it compares the user-scope version of the tsk and swe plugins in
#   installed_plugins.json with the version in each plugin's manifest on main, and updates
#   an older install. A plugin release then reaches
#   a session started from an older disk. The new version loads in the next Claude Code
#   process, not in this one.
#
# It prints one line for the session context when it runs the set-up or updates a plugin,
# and a WARNING line when a step fails. It always exits 0. A lock makes a second concurrent
# run (the repo hook and the plugin hook) wait, then find everything current.
#
# TSK_SETUP_REFRESH=off switches it off. TSK_SETUP_URL overrides the script's URL, and
# TSK_PLUGIN_MANIFEST_URL_TSK and TSK_PLUGIN_MANIFEST_URL_SWE override the manifest URLs.

[ "${TSK_SETUP_REFRESH:-on}" = "off" ] && exit 0
[ -f "${SETUP_DIR:-/usr/local/share/tsk-setup}/last-run" ] || exit 0

# raw_url REPO PATH: the URL of PATH at the tip of main in REPO on GitHub. The CDN keeps a
# file under main for up to 5 minutes (max-age=300), and a query string does not change
# that, so the URL names the commit SHA that git ls-remote reads from github.com. With no
# SHA, it falls back to main.
raw_url() {
  local sha
  sha="$(GIT_TERMINAL_PROMPT=0 timeout 10 git ls-remote "https://github.com/$1" refs/heads/main 2>/dev/null | cut -f 1)"
  echo "https://raw.githubusercontent.com/$1/${sha:-main}/$2"
}

URL="${TSK_SETUP_URL:-$(raw_url jimbarritt/tsk ops/cloud/setup-environment.sh)}"
STATE_DIR="${XDG_STATE_HOME:-${HOME:-/nonexistent}/.local/state}/tsk/setup"
mkdir -p "$STATE_DIR" 2>/dev/null || exit 0

exec 9>"$STATE_DIR/lock"
flock -w 300 9 || { echo "WARNING: the environment set-up check timed out waiting for another run"; exit 0; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if ! curl -sSfL --max-time 20 --retry 2 --retry-delay 2 "$URL" -o "$tmp/setup-environment.sh" 2>"$tmp/curl.log"; then
  echo "WARNING: fetching the environment set-up script failed: $(tr '\n' ' ' <"$tmp/curl.log")"
  exit 0
fi
new_hash="$(sha256sum "$tmp/setup-environment.sh" | cut -d ' ' -f 1)"
old_hash="$(cat "$STATE_DIR/script.sha256" 2>/dev/null)"

MARKETPLACE=jimbarritt-claude-plugins
INSTALLED="${CLAUDE_CONFIG_DIR:-${HOME:-/nonexistent}/.claude}/plugins/installed_plugins.json"

manifest_url() {
  case "$1" in
    tsk) echo "${TSK_PLUGIN_MANIFEST_URL_TSK:-$(raw_url jimbarritt/tsk plugin/.claude-plugin/plugin.json)}" ;;
    swe) echo "${TSK_PLUGIN_MANIFEST_URL_SWE:-$(raw_url jimbarritt/claude-plugins swe/.claude-plugin/plugin.json)}" ;;
  esac
}

# older A B: A is a lower version than B.
older() {
  [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n 1)" = "$1" ]
}

# Updates each plugin whose user-scope install is older than its manifest on main. The
# plugins belong at user scope only; a project-scope install is left alone.
update_plugins() {
  command -v claude >/dev/null 2>&1 && command -v jq >/dev/null 2>&1 && [ -f "$INSTALLED" ] || return 0
  local plugin key installed latest refreshed=0
  for plugin in tsk swe; do
    key="$plugin@$MARKETPLACE"
    installed="$(jq -r --arg k "$key" '[.plugins[$k][]? | select(.scope == "user") | .version][0] // empty' "$INSTALLED")"
    [ -n "$installed" ] || continue
    latest="$(curl -sSfL --max-time 10 --retry 2 --retry-delay 2 "$(manifest_url "$plugin")" 2>/dev/null | jq -r '.version // empty' 2>/dev/null)"
    [ -n "$latest" ] && older "$installed" "$latest" || continue
    if [ "$refreshed" = 0 ]; then
      claude plugin marketplace update "$MARKETPLACE" >"$tmp/plugin.log" 2>&1 || {
        echo "WARNING: refreshing the $MARKETPLACE marketplace failed: $(tr '\n' ' ' <"$tmp/plugin.log")"
        return 0
      }
      refreshed=1
    fi
    if claude plugin update "$key" --scope user >"$tmp/plugin.log" 2>&1; then
      echo "Session start updated the $plugin plugin from $installed to $latest. This session runs $installed; a new session loads $latest."
    else
      echo "WARNING: updating the $plugin plugin failed: $(tr '\n' ' ' <"$tmp/plugin.log")"
    fi
  done
}

if [ "$new_hash" = "$old_hash" ]; then
  update_plugins
  exit 0
fi

args=()
if [ -f "$STATE_DIR/args" ]; then
  while IFS= read -r line; do
    [ -n "$line" ] && args+=("$line")
  done <"$STATE_DIR/args"
elif command -v tsk >/dev/null 2>&1; then
  attribution="$(tsk config attribution 2>/dev/null)"
  email="$(printf '%s\n' "$attribution" | sed -n 's/^attribution email: //p')"
  if printf '%s\n' "$attribution" | grep -qx 'attribution identity: robot' && [ -n "$email" ]; then
    args=(--use-robot-attribution "$email")
  fi
fi

if bash "$tmp/setup-environment.sh" ${args[@]+"${args[@]}"} >"$tmp/run.log" 2>&1; then
  printf '%s\n' "$new_hash" >"$STATE_DIR/script.sha256"
  echo "The environment set-up script on main changed since this container's disk was saved, so session start ran it again (${args[*]:-no options})."
  # Claude Code reads ~/.claude/CLAUDE.md when its process starts, which can be before
  # this hook writes it. Print the user-level instructions so this session has them.
  if [ -f "${HOME:-/nonexistent}/.claude/AGENTS.md" ]; then
    echo "User-level instructions from ~/.claude/AGENTS.md: $(tr '\n' ' ' <"$HOME/.claude/AGENTS.md")"
  fi
else
  echo "WARNING: running the environment set-up script again failed: $(tail -n 5 "$tmp/run.log" | tr '\n' ' ')"
fi
# The set-up updated the plugins when its script changed. A plugin release with no change
# to the script still needs the check.
update_plugins
exit 0
