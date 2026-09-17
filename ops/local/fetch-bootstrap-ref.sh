#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/bootstrap-wt-lib.sh"

bootstrap_migrate_legacy_wt

WT="$(bootstrap_wt_path)"

git fetch origin refs/heads/tsk/bootstrap
SHA="$(git rev-parse FETCH_HEAD)"

if [ -d "$WT" ]; then
  # reset --hard would discard uncommitted work. Refuse instead: callers
  # reach this script to resolve a path as well as to refresh, and silent
  # loss is the worse failure.
  if [ -n "$(git -C "$WT" status --porcelain 2>/dev/null)" ]; then
    echo "error: $WT holds uncommitted changes; refusing to reset over them." >&2
    echo "       Push them with ops/local/push-bootstrap-ref.sh, or discard them" >&2
    echo "       deliberately with: git -C \"$WT\" reset --hard" >&2
    echo "       To read the path without refreshing, use ops/local/bootstrap-wt-path.sh" >&2
    exit 1
  fi
  git -C "$WT" reset --hard "$SHA" >/dev/null
else
  mkdir -p "$(dirname "$WT")"
  git worktree add --detach "$WT" "$SHA" >/dev/null
fi

echo "$WT"
