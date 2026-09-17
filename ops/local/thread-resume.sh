#!/usr/bin/env bash
set -euo pipefail

# Backing script for /resume-thread <thread-id>. Loads the latest
# continuation state entry, binds the current surface to the thread (additive:
# proceeds even if the thread already has another binding, but warns),
# and prints the entry as JSON plus any warning, for the skill to present.
#
# Usage: thread-resume.sh <thread-id>

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

if [ "$#" -ne 1 ]; then
  echo "usage: thread-resume.sh <thread-id>" >&2
  exit 1
fi

THREAD_ID="$1"
WT="$(thread_refresh_wt)"
DIR="$WT/threads/$THREAD_ID"

if [ ! -d "$DIR" ]; then
  echo "error: no such thread: $THREAD_ID" >&2
  exit 1
fi

CURRENT_ACTOR="$(thread_actor_urn)"
PRIOR_ACTORS="$(thread_written_by_actors "$THREAD_ID" "$WT")"

WARNING=""
if [ -n "$PRIOR_ACTORS" ] && ! printf '%s\n' "$PRIOR_ACTORS" | grep -qxF "$CURRENT_ACTOR"; then
  OTHERS="$(printf '%s' "$PRIOR_ACTORS" | paste -sd, -)"
  WARNING="thread $THREAD_ID is already associated with other actor(s): $OTHERS"
fi

# Additive take-over: bind regardless of what was already found above.
if [ -n "${CLAUDE_CODE_REMOTE_SESSION_ID:-}" ]; then
  EXISTING="$(thread_resolve_binding || true)"
  if [ "$EXISTING" != "cloud:$THREAD_ID" ]; then
    thread_bind_cloud "$THREAD_ID" "$WT"
    "$(dirname "${BASH_SOURCE[0]}")/push-bootstrap-ref.sh" "Bind $CURRENT_ACTOR to thread $THREAD_ID"
  fi
else
  # Local-only: the worktree marker lives in this worktree's own git
  # metadata, never on tsk/bootstrap, so there is nothing to push.
  thread_bind_worktree "$THREAD_ID"
fi

STORE="$DIR/continuation-state.jsonl"
LATEST="{}"
if [ -s "$STORE" ]; then
  LATEST="$(tail -n1 "$STORE")"
fi

jq -nc \
  --arg thread_id "$THREAD_ID" \
  --argjson latest "$LATEST" \
  --arg warning "$WARNING" \
  '{thread_id: $thread_id, latest: $latest, warning: $warning}'
