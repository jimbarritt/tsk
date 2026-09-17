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

# A worker restart can drop the record of a run that already committed, or
# already committed and pushed, so this script has to be safe to run twice.
# Each step is therefore conditional on the state it finds, not on the
# assumption that it is the first run.
if git diff --cached --quiet; then
  echo "note: nothing to commit in $WT; checking whether an earlier run left a commit to push." >&2
else
  git commit -m "$MESSAGE"
fi

git fetch origin refs/heads/tsk/bootstrap

# A commit made but not pushed leaves a clean worktree, which
# fetch-bootstrap-ref.sh's dirty-tree guard does not catch: its reset --hard
# would orphan the commit. Push it rather than exiting on the failed commit.
if git merge-base --is-ancestor HEAD FETCH_HEAD; then
  echo "note: tsk/bootstrap already holds this worktree's state; nothing to push." >&2
  exit 0
fi

git push origin HEAD:refs/heads/tsk/bootstrap
