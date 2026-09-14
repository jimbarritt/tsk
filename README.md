# tsk - work with a clear context
tsk is about navigating software delivery, for humans and agents.

It keeps track of where you and your agents are, individually or as *teams*, alongside your human colleagues and theirs.

It's your "sat nav" for work. Keep your human cognitive context and that of your agents clear and keep track of all the work threads you are context switching to.

## The foundations

At the core of the domain of tsk are four dimensions which are facets of any software engineering delivery. Where tsk 
is different is that it models all four of these dimensions explicitly. Other tools such as Linear or Jira 
only model some parts of these dimensions, and end up too abstract, in the wrong direction, to give
 a coherent abstraction.

tsk is very opinionated but within a very specific abstraction. It has a lot of flexibility but in the right dimensions.

The four dimensions are:

- Navigation
- Deltas
- Product
- Scale

For a full index of documentation, see [docs/index.md](docs/index.md).

Term definitions, rejected alternatives, and the reasons behind them are in
[docs/domain/ubiquitous-language.md](docs/domain/ubiquitous-language.md).

## Prerequisites

tsk is written in Rust. You need the Rust toolchain installed before building or installing.

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

## Upgrading

Same command: `cargo install` replaces the existing binaries.

```bash
cargo install tsk-bin tskd
```

This installs two binaries: `tsk` (CLI + TUI) and `tskd` (daemon).

## Getting started

See [docs/user-guide/getting-started.md](docs/user-guide/getting-started.md) for
running the daemon, creating and managing threads, global storage layout, binding
threads to projects, running tests, building from source, and publishing.

See [docs/user-guide/state-models.md](docs/user-guide/state-models.md) for the
task and thread state models, diversions, and how the daemon and client fit
together.

## CI

CodeQL static analysis runs on every push to `main` and weekly. Rust requires an advanced setup (`.github/workflows/codeql.yml`) because CodeQL must compile the code to analyse it: the default GitHub setup does not support Rust.

