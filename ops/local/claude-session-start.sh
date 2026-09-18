#!/usr/bin/env bash
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

# A shallow clone truncates history at an arbitrary boundary and marks the
# commit there as if it had no parent. Two shallow fetches at different times
# can truncate at two different points, so a later comparison between them
# (e.g. local main against origin/main) finds no common ancestor and looks
# exactly like a rewritten, unrelated history. It is neither: the real
# history is continuous on GitHub, just not present in this clone. Unshallow
# once, here, so no session ever has to tell the two apart.
if [ "$(git rev-parse --is-shallow-repository 2>/dev/null)" = "true" ]; then
  git fetch --unshallow origin >/dev/null 2>&1 || true
fi

source "$REPO_ROOT/ops/local/bootstrap-wt-lib.sh"
source "$REPO_ROOT/ops/local/thread-lib.sh"

INPUT="$(cat)"
SOURCE="$(printf '%s' "$INPUT" | jq -r '.source // empty')"

if [ "$SOURCE" = "startup" ]; then
  git stash push -u -m "session-start-autostash" >/dev/null 2>&1 || true
  git checkout main >/dev/null 2>&1 || true
  git pull origin main >/dev/null 2>&1 || true
  if git stash list 2>/dev/null | grep -q "session-start-autostash"; then
    git stash apply >/dev/null 2>&1 || true
  fi
fi

WT="$(ops/local/fetch-bootstrap-ref.sh 2>/dev/null || true)"

if [ -n "$WT" ] && [ -d "$WT" ]; then
  if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
    echo "export TSK_BOOTSTRAP_WT=\"$WT\"" >> "$CLAUDE_ENV_FILE"
  fi

  # fetch-bootstrap-ref.sh leaves the worktree alone when it holds a commit
  # origin does not have, rather than resetting over it. Surface that here:
  # its stderr is discarded above, and the agent is the one that can act on it.
  ORIGIN_SHA="$(git rev-parse FETCH_HEAD 2>/dev/null || true)"
  PENDING=""
  if [ -n "$ORIGIN_SHA" ]; then
    PENDING="$(bootstrap_wt_pending_commits "$WT" "$ORIGIN_SHA" || true)"
  fi

  if [ -n "$PENDING" ]; then
    PENDING_MSG=" WARNING: the worktree holds a commit that is not on origin's tsk/bootstrap, so it was left as it is rather than reset: $(printf '%s' "$PENDING" | tr '\n' ';'). A worker restart can end a turn between a commit and its push, which leaves exactly this state, so this may be your own work from a turn you hold no record of making. Do not assume another actor made it, and do not discard it on that basis. Read what it changes first, with: git -C '$WT' log -p $ORIGIN_SHA..HEAD. Then push it with ops/local/push-bootstrap-ref.sh, or discard it deliberately once you know what it is."
  else
    PENDING_MSG=""
  fi

  # $WT was just fetched above, so pass it through rather than fetching a
  # second time (M-BOOT-02, ad-hoc task: double fetch on every SessionStart).
  BINDING="$(ops/local/thread-resolve-binding.sh "$WT" 2>/dev/null || true)"

  if [ -n "$BINDING" ]; then
    THREAD_ID="${BINDING#*:}"
    THREAD_MSG=" An existing thread binding was found: $BINDING. Run /resume-thread $THREAD_ID next."
  else
    THREAD_MSG=" $(thread_unbound_prompt_text)"
  fi

  jq -n --arg wt "$WT" --arg thread_msg "$THREAD_MSG" --arg pending_msg "$PENDING_MSG" '{
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: ("tsk/bootstrap fetched and materialised at " + $wt + " (also exported as $TSK_BOOTSTRAP_WT). Read " + $wt + "/index.md next." + $pending_msg + $thread_msg)
    }
  }'
else
  jq -n '{
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: "Warning: the tsk/bootstrap branch could not be fetched automatically at session start (network or git error). Run `just fetch-refs` or `ops/local/fetch-bootstrap-ref.sh` manually before reading index.md."
    }
  }'
fi
