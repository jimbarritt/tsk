#!/usr/bin/env bash
# Build the `tsk` binary of the last release before the ledger home and the new nexus ref
# names, and write it to DIR/tsk, for the test that runs an older tsk against the nexus of a
# newer one (ADR 0035, tests/migrate_home.rs). The binary is built from the commit of release
# 0.69.0, or from the commit in TSK_OLD_COMMIT. The release archives of the earlier repository
# no longer exist, and GitHub blocks their tags in this one.
#
#   bash ops/local/fetch-last-release.sh <dir>
#
# Needs `git` and `cargo`. Prints the path of the binary. Exits 1 with a message when it
# cannot build one: the caller sets TSK_REQUIRE_OLD_BIN=1, so a missing binary fails the run.
set -euo pipefail

dir="${1:?usage: fetch-last-release.sh <dir>}"
commit="${TSK_OLD_COMMIT:-b594c8514fe1648d5574c65db10c661bfe01a0a3}"

mkdir -p "$dir"
src="$dir/source"
rm -rf "$src"
git fetch --quiet --depth 1 origin "$commit"
git worktree add --quiet --detach "$src" "$commit"
cargo build --quiet --release --locked -p tsk-bin --manifest-path "$src/Cargo.toml" --target-dir "$dir/target"
cp "$dir/target/release/tsk" "$dir/tsk"
"$dir/tsk" --version >&2
echo "$dir/tsk"
