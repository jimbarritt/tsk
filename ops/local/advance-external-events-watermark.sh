#!/usr/bin/env bash
set -euo pipefail

# Advances the external event queue's watermark on tsk/bootstrap (M-BOOT-02 T-19) and
# pushes. Call this only after the caller has actually finished processing every event
# through <count> (from read-new-external-events.sh's total_count), so a crash
# mid-processing leaves the watermark behind rather than skipping events. Refuses to
# move the watermark backwards: it only ever advances.
#
# Usage: advance-external-events-watermark.sh <count>

if [ "$#" -ne 1 ]; then
  echo "usage: advance-external-events-watermark.sh <count>" >&2
  exit 1
fi

case "$1" in
  '' | *[!0-9]*)
    echo "advance-external-events-watermark.sh: count must be a non-negative integer, got '$1'" >&2
    exit 1
    ;;
esac
COUNT="$1"

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"
WT="$("$SCRIPT_DIR/fetch-bootstrap-ref.sh")"
WATERMARK_FILE="$WT/external-events/watermark.json"

CURRENT=0
if [ -f "$WATERMARK_FILE" ]; then
  CURRENT="$(jq -r '.processed_through // 0' "$WATERMARK_FILE")"
fi

if [ "$COUNT" -lt "$CURRENT" ]; then
  echo "advance-external-events-watermark.sh: refusing to move the watermark backwards ($CURRENT -> $COUNT)" >&2
  exit 1
fi
if [ "$COUNT" -eq "$CURRENT" ]; then
  echo "note: watermark already at $CURRENT; nothing to advance." >&2
  exit 0
fi

mkdir -p "$(dirname "$WATERMARK_FILE")"
jq -n --argjson processed_through "$COUNT" --arg updated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  '{processed_through: $processed_through, updated_at: $updated_at}' >"$WATERMARK_FILE"

"$SCRIPT_DIR/push-bootstrap-ref.sh" "External event queue: advance watermark $CURRENT -> $COUNT"
echo "watermark:$COUNT"
