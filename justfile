# tsk justfile
# Install: brew install just

# Build and install tsk to ~/.cargo/bin
build-install:
    cargo install --path cli

# Run all tests, with the local Dolt server on (the default set in .cargo/config.toml)
test:
    cargo test --workspace

# Run all tests with the local Dolt server off, the path of a user who sets TSK_DOLT_SERVER=off
test-server-off:
    TSK_DOLT_SERVER=off cargo test --workspace

# Run all tests, with the test of an older tsk run against v0.69.0, the last release before
# the new ref names (needs gh, and a login for it). CI does the same before every release.
test-with-old-release:
    #!/usr/bin/env bash
    set -euo pipefail
    dir="$(mktemp -d)"
    bash ops/local/fetch-last-release.sh "$dir"
    TSK_OLD_BIN="$dir/tsk" TSK_REQUIRE_OLD_BIN=1 cargo test --workspace

# Build (debug)
build:
    cargo build --workspace

# Remove the installed binary
uninstall:
    cargo uninstall tsk-bin

# Bump the workspace version in Cargo.toml and Cargo.lock (no cargo-edit needed)
bump VERSION:
    python3 ops/local/bump-version.py {{VERSION}}

# Publish tsk-bin to crates.io
publish:
    cargo publish --package tsk-bin

# Fetch the ledger branch and print the ledger worktree path
ledger-fetch:
    @tsk ledger fetch

# Commit and push staged/unstaged changes in the ledger worktree
ledger-push MESSAGE:
    tsk ledger push "{{MESSAGE}}"
