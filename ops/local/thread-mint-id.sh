#!/usr/bin/env bash
set -euo pipefail

# Mint a new thread ID: 8-character lowercase base36, checked against
# threads/ on a freshly fetched tsk/bootstrap for collision. Prints the ID.
# Does not scaffold the thread directory or write any binding — see
# thread-start.sh for the composed operation.

source "$(dirname "${BASH_SOURCE[0]}")/thread-lib.sh"

thread_refresh_wt >/dev/null
thread_mint_id
