#!/usr/bin/env bash
set -euo pipefail

# Resolve the current binding: cloud session ID first, worktree marker
# otherwise. Prints "cloud:<thread-id>" or "worktree:<thread-id>" and
# exits 0 on a hit; prints nothing and exits 1 on a miss.
#
# Used by the SessionStart hook and by /start-thread and /resume-thread.
#
# Usage: thread-resolve-binding.sh [<already-fetched-wt>]
# With no argument, fetches before resolving (the safe default). Pass a
# worktree path only when the caller fetched it in this same invocation
# — see thread_resolve_binding in thread-lib.sh.

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

thread_resolve_binding "${1:-}"
