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
cargo install tsk-bin tskd
```

This installs two binaries: `tsk` (CLI + TUI) and `tskd` (daemon).

## Upgrading

Same command: `cargo install` replaces the existing binaries.

```bash
cargo install tsk-bin tskd
```

## CI

CodeQL static analysis runs on every push to `main` and weekly. Rust requires an
advanced setup (`.github/workflows/codeql.yml`) because CodeQL must compile the code
to analyse it: the default GitHub setup does not support Rust.
