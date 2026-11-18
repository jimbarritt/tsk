#!/usr/bin/env bash
# Download the `tsk` binary of the last release before the ledger home and the new nexus ref
# names to DIR/tsk (v0.69.0, or the tag in TSK_OLD_TAG), for the test that runs an older tsk
# against the nexus of a newer one (ADR 0035, tests/migrate_home.rs). A later release reads
# the marker, so the last release is not the one the test needs.
#
#   bash ops/local/fetch-last-release.sh <dir>
#
# Needs `gh`, logged in. Picks the archive for this machine: Linux or macOS, x86_64 or aarch64.
# Prints the path of the binary. Exits 1 with a message when it cannot get one: the caller sets
# TSK_REQUIRE_OLD_BIN=1, so a missing binary fails the run.
set -euo pipefail

dir="${1:?usage: fetch-last-release.sh <dir>}"
repo="${GITHUB_REPOSITORY:-jimbarritt/tsk}"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64 | Linux-arm64) target="aarch64-unknown-linux-gnu" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  *)
    echo "fetch-last-release.sh: no release archive for $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

tag="${TSK_OLD_TAG:-v0.69.0}"
mkdir -p "$dir"
gh release download "$tag" --repo "$repo" --pattern "tsk-*-$target.tar.gz" --dir "$dir" --clobber
tar -xzf "$dir"/tsk-*-"$target".tar.gz -C "$dir" tsk
"$dir/tsk" --version >&2
echo "$dir/tsk"
