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

With `tsk` installed, one command runs the same steps, then refreshes the marketplace and
updates the plugin:

```bash
tsk install-plugin claude-cli
```

It runs, through the `claude` binary on `PATH`:

1. `claude plugin marketplace add jimbarritt/claude-plugins`
2. `claude plugin install tsk@jimbarritt-claude-plugins --scope project -y`
3. `claude plugin marketplace update jimbarritt-claude-plugins`
4. `claude plugin update tsk@jimbarritt-claude-plugins --scope project`

It prints one line per step. A second run changes nothing and exits 0. When `claude` is
not on `PATH`, or a step fails, it exits 1 and names the step with the step's stderr.
`TSK_CLAUDE_BIN` set to a path runs that binary in place of the one on `PATH`.

| Option | Default | Meaning |
|---|---|---|
| `--scope <project\|local\|user>` | `project` | The scope the plugins are installed and updated at. |
| `--plugin <name>` | `tsk` | A plugin to install and update. Repeat it for more than one: `--plugin swe --plugin tsk`. |
| `--marketplace <source>` | `jimbarritt/claude-plugins` | The marketplace source passed to `marketplace add`. |
| `--marketplace-name <name>` | `jimbarritt-claude-plugins` | The name the marketplace is registered under. |

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
