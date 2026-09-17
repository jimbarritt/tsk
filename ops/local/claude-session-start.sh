#!/usr/bin/env bash
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

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

  BINDING="$(TSK_BOOTSTRAP_WT="$WT" ops/local/thread-resolve-binding.sh 2>/dev/null || true)"

  if [ -n "$BINDING" ]; then
    THREAD_ID="${BINDING#*:}"
    THREAD_MSG=" An existing thread binding was found: $BINDING. Run /resume-thread $THREAD_ID next."
  else
    THREAD_MSG=" No thread binding was found for this session or worktree. Ask directly which mission to work, then run /start-thread for it."
  fi

  jq -n --arg wt "$WT" --arg thread_msg "$THREAD_MSG" '{
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: ("tsk/bootstrap fetched and materialised at " + $wt + " (also exported as $TSK_BOOTSTRAP_WT). Read " + $wt + "/index.md next." + $thread_msg)
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
