#!/usr/bin/env bash
set -euo pipefail

# Backing script for /stop-thread. Detaches the current binding, if it points
# at the target thread, deletes the thread's own directory, and purges every
# cloud-session lookup entry still pointing at it — a thread that no longer
# exists cannot be a valid binding target for anyone.
#
# A worktree marker in some OTHER worktree that still names this thread
# cannot be reached or cleaned up from here: it is local metadata inside
# that worktree's own git directory, invisible to this script. It goes
# stale silently, the same limitation the binding design already accepts
# for worktree markers generally (no separate lookup map exists for them).
#
# Usage: thread-stop.sh [<thread-id>]
# With no argument, targets whatever this session or worktree is currently
# bound to. Given an explicit thread ID, targets that thread instead,
# regardless of the current binding — used by /switch-thread, which
# detaches first and then stops the thread it just left by ID.

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

WT="$(thread_refresh_wt)"
CURRENT_BINDING="$(thread_resolve_binding "$WT" || true)"

if [ "$#" -eq 1 ]; then
  THREAD_ID="$1"
else
  if [ -z "$CURRENT_BINDING" ]; then
    echo "error: no thread binding found for this session or worktree, and no thread id given; nothing to stop" >&2
    exit 1
  fi
  THREAD_ID="${CURRENT_BINDING#*:}"
fi

DIR="$WT/threads/$THREAD_ID"
if [ ! -d "$DIR" ]; then
  echo "error: no such thread: $THREAD_ID" >&2
  exit 1
fi

if [ "$CURRENT_BINDING" = "worktree:$THREAD_ID" ]; then
  rm -f "$(thread_git_dir)/tsk-thread-id"
fi

# Purge every cloud-session entry pointing at this thread, not only the
# current session's — another session may still be bound to the thread
# being deleted, and that binding must not survive it.
LOOKUP="$(thread_cloud_lookup_path "$WT")"
if [ -f "$LOOKUP" ]; then
  TMP="$(mktemp)"
  jq --arg id "$THREAD_ID" 'with_entries(select(.value.thread_id != $id))' "$LOOKUP" > "$TMP"
  mv "$TMP" "$LOOKUP"
fi

rm -rf "$DIR"

"$(dirname "${BASH_SOURCE[0]}")/push-bootstrap-ref.sh" "Stop thread $THREAD_ID"

echo "stopped:$THREAD_ID"
