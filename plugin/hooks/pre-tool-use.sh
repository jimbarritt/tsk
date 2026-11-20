#!/usr/bin/env bash
# PreToolUse: denies a tool call that changes the ledger without tsk (ADR 0028). It fails
# open: a tsk that has no `ledger guard` (older than 0.36.5) exits non-zero here, and a
# non-zero exit of a hook would block the tool call.
set -uo pipefail

command -v tsk >/dev/null 2>&1 || exit 0
if [ "${1:-}" = "copilot" ]; then
  decision="$(tsk ledger guard --format copilot 2>/dev/null)" || exit 0
else
  decision="$(tsk ledger guard 2>/dev/null)" || exit 0
fi
[ -z "$decision" ] || printf '%s\n' "$decision"
exit 0
