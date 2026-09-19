#!/usr/bin/env bash
set -euo pipefail

# Backing script for /detach-thread. Removes only the current session's or
# worktree's own binding from its thread: the cloud lookup entry, or the
# worktree marker file. The thread itself, its continuation state, and any
# other actor's binding to it are untouched.
#
# Prints "detached:<thread-id>" and exits 0 on success. Exits 1, printing
# nothing to stdout, if there is no binding to remove.
#
# Usage: thread-detach.sh

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

WT="$(thread_refresh_wt)"
BINDING="$(thread_resolve_binding "$WT" || true)"

if [ -z "$BINDING" ]; then
  echo "error: no thread binding found for this session or worktree; nothing to detach" >&2
  exit 1
fi

THREAD_ID="${BINDING#*:}"

if [ -n "${CLAUDE_CODE_REMOTE_SESSION_ID:-}" ]; then
  LOOKUP="$(thread_cloud_lookup_path "$WT")"
  TMP="$(mktemp)"
  jq --arg k "$CLAUDE_CODE_REMOTE_SESSION_ID" 'del(.[$k])' "$LOOKUP" > "$TMP"
  mv "$TMP" "$LOOKUP"
  "$(dirname "${BASH_SOURCE[0]}")/push-bootstrap-ref.sh" "Detach $(thread_actor_urn) from thread $THREAD_ID"
else
  # Local-only: the worktree marker lives inside this worktree's own git
  # metadata, never on tsk/bootstrap, so there is nothing to push here.
  rm -f "$(thread_git_dir)/tsk-thread-id"
fi

echo "detached:$THREAD_ID"
