#!/usr/bin/env bash
set -euo pipefail

# Scaffold threads/<id>/ on the bootstrap worktree: index.md linking the
# mission briefing, and an empty continuation-state.jsonl. Does not push —
# the caller (thread-start.sh) pushes once after all its writes.
#
# Usage: thread-scaffold.sh <thread-id> <mission-briefing-path-relative-to-wt>

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

if [ "$#" -ne 2 ]; then
  echo "usage: thread-scaffold.sh <thread-id> <mission-briefing-path-relative-to-wt>" >&2
  exit 1
fi

THREAD_ID="$1"
BRIEFING_PATH="$2"
WT="$(thread_wt)"
DIR="$WT/threads/$THREAD_ID"

if [ -e "$DIR" ]; then
  echo "error: threads/$THREAD_ID already exists" >&2
  exit 1
fi

mkdir -p "$DIR"

cat > "$DIR/index.md" <<EOF
# Thread $THREAD_ID

Mission briefing: [$BRIEFING_PATH]($BRIEFING_PATH)
EOF

: > "$DIR/continuation-state.jsonl"
