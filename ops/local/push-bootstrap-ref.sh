#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: push-bootstrap-ref.sh <commit message>" >&2
  exit 1
fi

MESSAGE="$1"
WT="$(git rev-parse --path-format=absolute --git-common-dir)/tsk/bootstrap-ref-wt"

if [ ! -d "$WT" ]; then
  echo "error: $WT does not exist. Run ops/local/fetch-bootstrap-ref.sh first." >&2
  exit 1
fi

cd "$WT"
git add -A
git commit -m "$MESSAGE"
git fetch origin refs/heads/tsk/bootstrap
git push origin HEAD:refs/heads/tsk/bootstrap
