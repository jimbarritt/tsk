#!/usr/bin/env bash
# commit-msg: in a Claude Code cloud session, writes the Agent-Session trailer (ADR 0029,
# decision 3) in place of the Claude-Session trailer the harness adds, and drops the
# Co-Authored-By: Claude trailer (M-TSK-ATTRIB). Outside a cloud session it changes nothing.
# Installed as .git/hooks/commit-msg by ops/local/claude-session-start.sh.
set -uo pipefail

msg="${1:-}"
[ -f "$msg" ] || exit 0

sid="${CLAUDE_CODE_REMOTE_SESSION_ID:-}"
case "$sid" in
  cse_*) ;;
  *) exit 0 ;;
esac

sed -i -E '/^Claude-Session:/d; /^Co-Authored-By: Claude/Id; /^Agent-Session:/d' "$msg"
git interpret-trailers --in-place --if-exists replace \
  --trailer "Agent-Session: urn:tsk:session:claude-cloud:${sid}" "$msg" || exit 0
exit 0
