#!/usr/bin/env bash
set -euo pipefail

# Producer-side script for the external event queue (M-BOOT-02 T-19, design settled
# 2026-09-19). Appends one envelope line to external-events/queue.ndjson on
# tsk/bootstrap and pushes. Built for the security-event GitHub Action, but usable by
# any external event source that can supply a JSON payload file.
#
# Reuses fetch-bootstrap-ref.sh / push-bootstrap-ref.sh rather than its own git
# commands, per CLAUDE.md's rule against a second copy of the ref-collision and
# concurrent-push handling those scripts already carry.
#
# Usage: append-external-event.sh <source> <event-type> <action> <repo> <payload-json-file>

if [ "$#" -ne 5 ]; then
  echo "usage: append-external-event.sh <source> <event-type> <action> <repo> <payload-json-file>" >&2
  exit 1
fi

SOURCE="$1"
EVENT_TYPE="$2"
ACTION="$3"
REPO="$4"
PAYLOAD_FILE="$5"

if [ ! -f "$PAYLOAD_FILE" ]; then
  echo "error: no such payload file: $PAYLOAD_FILE" >&2
  exit 1
fi

WT="$("$(dirname "${BASH_SOURCE[0]}")/fetch-bootstrap-ref.sh")"
QUEUE="$WT/external-events/queue.ndjson"
mkdir -p "$(dirname "$QUEUE")"

TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

jq -nc \
  --arg source "$SOURCE" \
  --arg event_type "$EVENT_TYPE" \
  --arg action "$ACTION" \
  --arg repo "$REPO" \
  --arg received_at "$TIMESTAMP" \
  --slurpfile payload "$PAYLOAD_FILE" \
  '{
    source: $source,
    event_type: $event_type,
    action: $action,
    repo: $repo,
    received_at: $received_at,
    payload: $payload[0]
  }' >> "$QUEUE"

"$(dirname "${BASH_SOURCE[0]}")/push-bootstrap-ref.sh" "External event queue: $SOURCE $EVENT_TYPE ($ACTION) on $REPO"

echo "queued"
