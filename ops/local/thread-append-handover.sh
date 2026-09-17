#!/usr/bin/env bash
set -euo pipefail

# Backing script for /pause-thread. Appends one continuation event to
# threads/<id>/continuation-state.jsonl and pushes.
#
# The commit-on-tsk/bootstrap field is captured as the branch's HEAD
# immediately before this event is appended (the state the thread was
# working against when it paused), not the commit this push produces —
# that commit cannot describe its own tree. See the mission's plan note
# on this field for the reasoning.
#
# Usage: thread-append-handover.sh <thread-id> <mission-briefing-link> <task-id> <whats-next>

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

if [ "$#" -ne 4 ]; then
  echo "usage: thread-append-handover.sh <thread-id> <mission-briefing-link> <task-id> <whats-next>" >&2
  exit 1
fi

THREAD_ID="$1"
MISSION_LINK="$2"
TASK_ID="$3"
WHATS_NEXT="$4"

WT="$(thread_refresh_wt)"
LOG="$WT/threads/$THREAD_ID/continuation-state.jsonl"

if [ ! -f "$LOG" ]; then
  echo "error: no such thread: $THREAD_ID (expected $LOG)" >&2
  exit 1
fi

COMMIT_ON_BOOTSTRAP="$(git -C "$WT" rev-parse HEAD)"
COMMIT_ON_MAIN="$(git rev-parse HEAD)"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
WRITTEN_BY="$(thread_actor_urn)"

jq -nc \
  --arg mission_link "$MISSION_LINK" \
  --arg task_id "$TASK_ID" \
  --arg whats_next "$WHATS_NEXT" \
  --arg commit_on_bootstrap "$COMMIT_ON_BOOTSTRAP" \
  --arg commit_on_main "$COMMIT_ON_MAIN" \
  --arg timestamp "$TIMESTAMP" \
  --arg written_by "$WRITTEN_BY" \
  '{
    mission_link: $mission_link,
    task_id: $task_id,
    whats_next: $whats_next,
    commit_on_bootstrap: $commit_on_bootstrap,
    commit_on_main: $commit_on_main,
    timestamp: $timestamp,
    written_by: $written_by
  }' >> "$LOG"

"$(dirname "${BASH_SOURCE[0]}")/push-bootstrap-ref.sh" "Pause thread $THREAD_ID: $TASK_ID"

echo "paused:$THREAD_ID"
