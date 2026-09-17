#!/usr/bin/env bash
set -euo pipefail

# Resolve the current binding: cloud session ID first, worktree marker
# otherwise. Prints "cloud:<thread-id>" or "worktree:<thread-id>" and
# exits 0 on a hit; prints nothing and exits 1 on a miss.
#
# Used by the SessionStart hook and by /start-thread and /resume-thread.

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

thread_resolve_binding
