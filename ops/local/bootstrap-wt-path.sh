#!/usr/bin/env bash
set -euo pipefail

# Print the bootstrap worktree's path. Pure: no fetch, no reset, no side
# effects. Use this when you need the path. Use fetch-bootstrap-ref.sh only
# when you also want the worktree refreshed to origin's latest.

source "$(dirname "${BASH_SOURCE[0]}")/bootstrap-wt-lib.sh"

bootstrap_wt_path
