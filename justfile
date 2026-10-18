# tsk justfile
# Install: brew install just

# Build and install tsk to ~/.cargo/bin
build-install:
    cargo install --path cli

# Run all tests
test:
    cargo test --workspace

# Build (debug)
build:
    cargo build --workspace

# Remove the installed binary
uninstall:
    cargo uninstall tsk-bin

# Bump the workspace version (requires cargo-edit: cargo install cargo-edit)
bump VERSION:
    cargo set-version --workspace {{VERSION}}

# Publish tsk-bin to crates.io
publish:
    cargo publish --package tsk-bin

# Fetch the ledger branch and print the ledger worktree path
ledger-fetch:
    @tsk ledger fetch

# Commit and push staged/unstaged changes in the ledger worktree
ledger-push MESSAGE:
    tsk ledger push "{{MESSAGE}}"
