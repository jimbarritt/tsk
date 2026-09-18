#!/usr/bin/env bash
set -uo pipefail

exec "$CLAUDE_PROJECT_DIR/ops/local/thread-binding-guard.sh"
