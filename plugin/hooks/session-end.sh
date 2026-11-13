#!/usr/bin/env bash
set -uo pipefail

# The SessionEnd hook: publish the ledger when the session ends, best effort. The publish
# rule (ADR 0019) keeps record writes local, so this is where a session's records go out
# if it never ran `tsk thread pause`. It never fails and never blocks the end of the
# session for long: a missing tsk, a missing ledger, no network or a slow origin all
# end with exit status 0. A container reclaimed before the hook runs loses the records.
#
INPUT="$(cat 2>/dev/null || true)"

command -v tsk >/dev/null 2>&1 || exit 0

# Record on this machine that the session ended, so its robot is free again (tsk 0.60.0).
printf '%s' "$INPUT" | tsk session end >/dev/null 2>&1 || true

# Ask the local store first (no network): a push with nothing to send still costs a
# round trip to origin.
STATUS="$(tsk ledger status --local 2>/dev/null)" || exit 0
case "$STATUS" in
  *"not pushed"*) ;;
  *) exit 0 ;;
esac

# A detached push, so the hook exits at once: Claude Code cancels a SessionEnd hook that
# is still running when it exits (tsk 0.63.0).
tsk ledger push --background "End of session" >/dev/null 2>&1 || true
exit 0
