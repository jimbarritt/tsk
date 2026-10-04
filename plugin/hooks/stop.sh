#!/usr/bin/env bash
set -uo pipefail

command -v tsk >/dev/null 2>&1 || exit 0
exec tsk thread guard
