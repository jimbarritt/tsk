#!/usr/bin/env bash
# Sets the agent state of this tmux pane (waiting, working, error or none), which the
# session chooser of tsk tmux install shows. Does nothing outside tmux. It fails open: a
# tsk with no tmux command (older than 0.43.0) exits non-zero here, and this script still
# exits 0.
set -uo pipefail

cat >/dev/null 2>&1 || true
[ -n "${TMUX_PANE:-}" ] || exit 0
command -v tsk >/dev/null 2>&1 || exit 0
tsk tmux state "${1:-none}" >/dev/null 2>&1 || true
exit 0
