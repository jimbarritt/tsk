#!/usr/bin/env bash
set -euo pipefail

# Polls GitHub's Dependabot, code scanning and secret scanning alert APIs for one
# repository, and appends each open alert as one event to the external event queue
# on tsk/bootstrap (M-BOOT-02 T-19), in a single fetch/append/push cycle covering
# every alert this run finds.
#
# Meant to run under a GitHub Action's own GITHUB_TOKEN, scoped with
# security-events: read: that token is separate from the Claude GitHub App
# installation, which cannot read these endpoints, so it is not subject to that gap.
#
# Usage: poll-security-alerts.sh <owner/repo>
# Reads the token from the GITHUB_TOKEN environment variable.

if [ "$#" -ne 1 ]; then
  echo "usage: poll-security-alerts.sh <owner/repo>" >&2
  exit 1
fi

REPO="$1"
: "${GITHUB_TOKEN:?GITHUB_TOKEN must be set}"
SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"

fetch() {
  curl -sS -H "Authorization: Bearer $GITHUB_TOKEN" -H "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/$REPO/$1?state=open&per_page=100"
}

WT="$("$SCRIPT_DIR/fetch-bootstrap-ref.sh")"
QUEUE="$WT/external-events/queue.ndjson"
mkdir -p "$(dirname "$QUEUE")"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
APPENDED=0

append_alerts() {
  local event_type="$1" alerts="$2" count

  if ! echo "$alerts" | jq -e 'type == "array"' >/dev/null 2>&1; then
    echo "warning: $event_type response was not a list, skipping ($(echo "$alerts" | jq -c '.message // .' 2>/dev/null))" >&2
    return 0
  fi

  count="$(echo "$alerts" | jq 'length')"
  for i in $(seq 0 $((count - 1))); do
    echo "$alerts" | jq -c \
      --arg source "github" \
      --arg event_type "$event_type" \
      --arg action "polled" \
      --arg repo "$REPO" \
      --arg received_at "$TIMESTAMP" \
      ".[$i] as \$p | {source: \$source, event_type: \$event_type, action: \$action, repo: \$repo, received_at: \$received_at, payload: \$p}" \
      >>"$QUEUE"
    APPENDED=$((APPENDED + 1))
  done
}

append_alerts "dependabot_alert" "$(fetch dependabot/alerts)"
append_alerts "code_scanning_alert" "$(fetch code-scanning/alerts)"
append_alerts "secret_scanning_alert" "$(fetch secret-scanning/alerts)"

if [ "$APPENDED" -eq 0 ]; then
  echo "no open alerts queued"
  exit 0
fi

"$SCRIPT_DIR/push-bootstrap-ref.sh" "External event queue: polled $APPENDED open security alert(s) on $REPO"
echo "queued:$APPENDED"
