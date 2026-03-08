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
