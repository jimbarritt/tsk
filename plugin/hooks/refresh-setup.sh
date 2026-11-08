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
#   script.sha256 in the state directory. With no change, it exits with no output.
# - Otherwise it runs the fetched script with the options in the state directory's args
#   file (one option per line, written by setup-environment.sh), then writes the new
#   SHA-256. A disk saved before the args file existed has none: robot attribution in the
#   user tsk config then gives --use-robot-attribution <email>.
#
# It prints one line for the session context when it runs the set-up, and a WARNING line
# when the fetch or the run fails. It always exits 0. A lock makes a second concurrent run
# (the repo hook and the plugin hook) wait, then find the hash current.
#
# TSK_SETUP_REFRESH=off switches it off. TSK_SETUP_URL overrides the script's URL.

[ "${TSK_SETUP_REFRESH:-on}" = "off" ] && exit 0
[ -f "${SETUP_DIR:-/usr/local/share/tsk-setup}/last-run" ] || exit 0

URL="${TSK_SETUP_URL:-https://raw.githubusercontent.com/jimbarritt/tsk/main/ops/cloud/setup-environment.sh}"
STATE_DIR="${XDG_STATE_HOME:-${HOME:-/nonexistent}/.local/state}/tsk/setup"
mkdir -p "$STATE_DIR" 2>/dev/null || exit 0

exec 9>"$STATE_DIR/lock"
flock -w 300 9 || { echo "WARNING: the environment set-up check timed out waiting for another run"; exit 0; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if ! curl -sSfL --max-time 20 "$URL" -o "$tmp/setup-environment.sh" 2>"$tmp/curl.log"; then
  echo "WARNING: fetching the environment set-up script failed: $(tr '\n' ' ' <"$tmp/curl.log")"
  exit 0
fi
new_hash="$(sha256sum "$tmp/setup-environment.sh" | cut -d ' ' -f 1)"
old_hash="$(cat "$STATE_DIR/script.sha256" 2>/dev/null)"
[ "$new_hash" = "$old_hash" ] && exit 0

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
exit 0
