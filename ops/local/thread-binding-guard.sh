#!/usr/bin/env bash
set -uo pipefail

# Backing script for the Stop hook. Closes the gap found in M-BOOT-02-02: the
# SessionStart hook names the binding instruction once, and it can be skipped
# once conversation moves elsewhere, leaving the session unbound for its
# entire duration. This runs every turn instead of once, and blocks turn
# completion until a thread is bound.
#
# Deliberately not the SessionStart hook's fetch-then-check: a network fetch
# on every turn is slow and fragile, and unnecessary here — see
# thread_resolve_binding_local in thread-lib.sh.
#
# Relies on Claude Code's own Stop-hook safety valve (it overrides a hook
# that blocks eight times in a row with no progress) rather than tracking
# attempts itself: AskUserQuestion cannot fail to elicit a response, so
# reaching that cap here would mean the binding flow itself is broken, a
# case this script cannot repair by trying again a ninth time.
#
# Wired via .claude/hooks/thread-binding-guard.sh and .claude/settings.json's
# hooks.Stop.

REPO_ROOT="$(git rev-parse --show-toplevel)"
source "$REPO_ROOT/ops/local/thread-lib.sh"

# The hook's input JSON carries session and transcript fields this check
# does not need, but stdin must still be consumed.
cat >/dev/null

if thread_resolve_binding_local >/dev/null 2>&1; then
  exit 0
fi

REASON="$(thread_unbound_prompt_text)"
jq -nc --arg reason "$REASON" '{decision: "block", reason: $reason}'
