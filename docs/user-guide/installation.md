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
cargo install tsk-bin --locked
```

This installs the `tsk` binary. `tskd` is retired (ADR 0012) and is not needed.

The Claude Code plugin installs from the `jimbarritt/claude-plugins` marketplace. Its
entry points at `plugin/` in the tsk repo:

```bash
claude plugin marketplace add jimbarritt/claude-plugins
claude plugin install tsk@jimbarritt-claude-plugins
```

The plugin's `SessionStart` hook installs the `tsk` version the plugin requires when
`tsk` is missing or older than that version. A newer `tsk` is accepted. With `TSK_SOURCE` set to a path, it installs
from that source tree with `cargo install --path`.

## Upgrading

Run the install command again with the new tag: `cargo install` replaces the existing
binary.

## CI

CodeQL static analysis runs on every push to `main` and weekly. Rust requires an
advanced setup (`.github/workflows/codeql.yml`) because CodeQL must compile the code
to analyse it: the default GitHub setup does not support Rust.
