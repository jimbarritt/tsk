#!/usr/bin/env bash
set -euo pipefail

WT="$(git rev-parse --path-format=absolute --git-common-dir)/tsk/bootstrap-ref-wt"

git fetch origin refs/heads/tsk/bootstrap
SHA="$(git rev-parse FETCH_HEAD)"

if [ -d "$WT" ]; then
  git -C "$WT" reset --hard "$SHA" >/dev/null
else
  git worktree add --detach "$WT" "$SHA" >/dev/null
fi

echo "$WT"
