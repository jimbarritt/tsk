#!/usr/bin/env bash
set -euo pipefail

# Lists every thread on tsk/bootstrap: id, mission briefing link, and, if
# the thread has been paused at least once, the latest continuation state
# entry's whats-next text and timestamp. One JSON object per line, most
# recently updated first; a thread with no continuation state entries yet
# sorts last.
#
# Used by /switch-thread to offer existing threads when none is named.
#
# Usage: thread-list.sh

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

WT="$(thread_refresh_wt)"

for DIR in "$WT"/threads/*/; do
  [ -d "$DIR" ] || continue
  THREAD_ID="$(basename "$DIR")"
  MISSION_LINK="$(grep -oP '(?<=Mission briefing: \[)[^]]*' "$DIR/index.md" 2>/dev/null | head -n1 || true)"
  STORE="$DIR/continuation-state.jsonl"
  LATEST="{}"
  if [ -s "$STORE" ]; then
    LATEST="$(tail -n1 "$STORE")"
  fi
  jq -nc \
    --arg id "$THREAD_ID" \
    --arg mission_link "$MISSION_LINK" \
    --argjson latest "$LATEST" \
    '{id: $id, mission_link: $mission_link, latest_whats_next: ($latest.whats_next // null), latest_timestamp: ($latest.timestamp // null)}'
done | jq -sc 'sort_by(.latest_timestamp // "") | reverse | .[]'
