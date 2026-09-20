#!/usr/bin/env bash
set -euo pipefail

# Reads the external event queue on tsk/bootstrap (M-BOOT-02 T-19) and prints the
# events past the watermark, one JSON object to stdout:
#   { "new_count": N, "total_count": M, "events": [ ... ] }
# "events" is the queue's own envelope objects, oldest first, from line
# watermark+1 through the end. Prints { "new_count": 0, "total_count": M, "events": [] }
# if there is nothing new. Does not write the watermark: pair with
# advance-external-events-watermark.sh once the caller has actually processed
# "total_count" events, so a crash mid-processing does not lose events.
#
# Usage: read-new-external-events.sh

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"
WT="$("$SCRIPT_DIR/fetch-bootstrap-ref.sh")"
QUEUE="$WT/external-events/queue.ndjson"
WATERMARK_FILE="$WT/external-events/watermark.json"

if [ ! -f "$QUEUE" ]; then
  echo '{"new_count":0,"total_count":0,"events":[]}'
  exit 0
fi

TOTAL="$(wc -l <"$QUEUE" | tr -d '[:space:]')"

WATERMARK=0
if [ -f "$WATERMARK_FILE" ]; then
  WATERMARK="$(jq -r '.processed_through // 0' "$WATERMARK_FILE")"
fi

if [ "$WATERMARK" -ge "$TOTAL" ]; then
  printf '{"new_count":0,"total_count":%s,"events":[]}\n' "$TOTAL"
  exit 0
fi

tail -n "+$((WATERMARK + 1))" "$QUEUE" | jq -sc \
  --argjson total "$TOTAL" \
  '{new_count: length, total_count: $total, events: .}'
