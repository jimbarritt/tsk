#!/usr/bin/env bash
set -euo pipefail

# Backing script for /start-thread. Resolves the current binding; if one
# already exists, this is a resume, not a start (exit 2, print the
# existing binding so the caller can hand off to /resume-thread). If none
# exists, mints a new thread ID, scaffolds threads/<id>/, writes the
# binding, and pushes once.
#
# The mission argument is validated independently here, not trusted from
# the agent's own resolution: the briefing path must exist in the
# bootstrap worktree.
#
# Usage: thread-start.sh <mission-id> <mission-briefing-path-relative-to-wt>

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

if [ "$#" -ne 2 ]; then
  echo "usage: thread-start.sh <mission-id> <mission-briefing-path-relative-to-wt>" >&2
  exit 1
fi

MISSION_ID="$1"
BRIEFING_PATH="$2"

EXISTING="$(thread_resolve_binding || true)"
if [ -n "$EXISTING" ]; then
  echo "resume-required:${EXISTING#*:}"
  exit 2
fi

WT="$(thread_wt)"

if [ ! -f "$WT/$BRIEFING_PATH" ]; then
  echo "error: mission briefing not found at threads-relative path '$BRIEFING_PATH' (checked $WT/$BRIEFING_PATH)" >&2
  exit 1
fi

THREAD_ID="$(thread_mint_id)"

"$(dirname "${BASH_SOURCE[0]}")/thread-scaffold.sh" "$THREAD_ID" "$BRIEFING_PATH"
thread_bind_current "$THREAD_ID" "$WT"

cd "$WT"
git add -A
git commit -m "Start thread $THREAD_ID for mission $MISSION_ID"
git fetch origin refs/heads/tsk/bootstrap
git push origin HEAD:refs/heads/tsk/bootstrap

echo "started:$THREAD_ID"
