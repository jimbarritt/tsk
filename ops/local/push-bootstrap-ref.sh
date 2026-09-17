#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/bootstrap-wt-lib.sh"

if [ "$#" -ne 1 ]; then
  echo "usage: push-bootstrap-ref.sh <commit message>" >&2
  exit 1
fi

MESSAGE="$1"
WT="$(bootstrap_wt_path)"

if [ ! -d "$WT" ]; then
  echo "error: $WT does not exist. Run ops/local/fetch-bootstrap-ref.sh first." >&2
  exit 1
fi

cd "$WT"
git add -A
git commit -m "$MESSAGE"
git fetch origin refs/heads/tsk/bootstrap
git push origin HEAD:refs/heads/tsk/bootstrap
