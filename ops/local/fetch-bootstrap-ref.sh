#!/usr/bin/env bash
set -euo pipefail

WT="$(git rev-parse --path-format=absolute --git-common-dir)/tsk-bootstrap-wt"

git fetch origin refs/tsk/bootstrap

if [ -d "$WT" ]; then
  git -C "$WT" reset --hard FETCH_HEAD >/dev/null
else
  git worktree add --detach "$WT" FETCH_HEAD >/dev/null
fi

echo "$WT"
