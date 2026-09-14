# tsk justfile
# Install: brew install just

# Build and install tsk and tskd to ~/.cargo/bin
build-install:
    cargo install --path cli
    cargo install --path daemon

# Run all tests
test:
    cargo test --workspace

# Build (debug)
build:
    cargo build --workspace

# Remove installed binaries
uninstall:
    cargo uninstall tsk-bin
    cargo uninstall tskd

# Bump the workspace version (requires cargo-edit: cargo install cargo-edit)
bump VERSION:
    cargo set-version --workspace {{VERSION}}

# Publish all crates to crates.io
# core must be published first; sleep gives crates.io time to index it
# before tsk-bin and tskd resolve the version dependency
publish:
    cargo publish --package tsk-core
    sleep 30
    cargo publish --package tsk-bin
    cargo publish --package tskd

# Fetch the bootstrap ref and print the worktree path it materialises at
fetch-refs:
    @ops/local/fetch-bootstrap-ref.sh

# Commit and push staged/unstaged changes in the bootstrap ref worktree
push-refs MESSAGE:
    ops/local/push-bootstrap-ref.sh "{{MESSAGE}}"
