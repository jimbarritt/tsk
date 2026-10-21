#!/usr/bin/env bash
set -uo pipefail

# The SessionEnd hook: publish the ledger when the session ends, best effort. The publish
# rule (ADR 0019) keeps record writes local, so this is where a session's records go out
# if it never ran `tsk thread pause`. It never fails and never blocks the end of the
# session for long: a missing tsk, a missing ledger, no network or a slow origin all
# end with exit status 0. A container reclaimed before the hook runs loses the records.
#
# TSK_SESSION_END_TIMEOUT_SECS (default 8) bounds the push.
cat >/dev/null 2>&1 || true

command -v tsk >/dev/null 2>&1 || exit 0

# Ask the local store first (no network): a push with nothing to send still costs a
# round trip to origin.
STATUS="$(tsk ledger status --local 2>/dev/null)" || exit 0
case "$STATUS" in
  *"not pushed"*) ;;
  *) exit 0 ;;
esac

LIMIT="${TSK_SESSION_END_TIMEOUT_SECS:-8}"
if command -v timeout >/dev/null 2>&1; then
  timeout "$LIMIT" tsk ledger push "End of session" >/dev/null 2>&1 || true
else
  tsk ledger push "End of session" >/dev/null 2>&1 || true
fi
exit 0
