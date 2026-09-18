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

# Two sessions can hold this worktree's location concurrently (each its own
# clone, hence its own $WT) and push to tsk/bootstrap between each other's
# fetch and push. Rebase onto origin's tip and retry, bounded, rather than
# rejecting on the first non-fast-forward.
MAX_ATTEMPTS=5
for attempt in $(seq 1 "$MAX_ATTEMPTS"); do
  git fetch origin refs/heads/tsk/bootstrap

  # A commit made but not pushed leaves a clean worktree, which
  # fetch-bootstrap-ref.sh's dirty-tree guard does not catch: its reset --hard
  # would orphan the commit. Push it rather than exiting on the failed commit.
  if git merge-base --is-ancestor HEAD FETCH_HEAD; then
    echo "note: tsk/bootstrap already holds this worktree's state; nothing to push." >&2
    exit 0
  fi

  if ! git merge-base --is-ancestor FETCH_HEAD HEAD; then
    if ! git rebase FETCH_HEAD; then
      git rebase --abort
      echo "error: rebase onto origin's tsk/bootstrap conflicted." >&2
      echo "       Resolve it in $WT: git rebase FETCH_HEAD, fix the conflicts," >&2
      echo "       git rebase --continue, then re-run this script." >&2
      exit 1
    fi
  fi

  if git push origin HEAD:refs/heads/tsk/bootstrap; then
    exit 0
  fi

  echo "note: push rejected (attempt $attempt/$MAX_ATTEMPTS); origin moved again, retrying." >&2
done

echo "error: failed to push to tsk/bootstrap after $MAX_ATTEMPTS attempts." >&2
exit 1
