# Installation

## Prerequisites

tsk is written in Rust. You need the Rust toolchain installed before building or
installing.

**rustup** (recommended: official installer, works everywhere):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Homebrew** (macOS):
```bash
brew install rust
```

**mise** (if you use mise for toolchain management):
```bash
mise use -g rust@latest
```

Once installed, verify with:
```bash
rustc --version
cargo --version
```

## Installation

```bash
cargo install --git https://github.com/jimbarritt/tsk tsk-bin --tag v0.2.0 --locked
```

This installs the `tsk` binary. 0.2.0 is not on crates.io yet. `tskd` is retired
(ADR 0012) and is not needed.

The Claude Code plugin installs from the marketplace in the tsk repo:

```bash
claude plugin marketplace add jimbarritt/tsk
claude plugin install tsk@tsk
```

The plugin's `SessionStart` hook installs the `tsk` version the plugin requires when
`tsk` is missing or at another version. With `TSK_SOURCE` set to a path, it installs
from that source tree with `cargo install --path`.

## Upgrading

Run the install command again with the new tag: `cargo install` replaces the existing
binary.

## CI

CodeQL static analysis runs on every push to `main` and weekly. Rust requires an
advanced setup (`.github/workflows/codeql.yml`) because CodeQL must compile the code
to analyse it: the default GitHub setup does not support Rust.
