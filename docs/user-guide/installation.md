# Installation

## Prerequisites

- **git**, with credentials that can push to the repo whose ledger you keep.
- **curl**, **tar** and **jq**. The `SessionStart` hooks use `curl` and `tar` to install
  `dolt`, and `jq` to read the hook input and write the hook output.
- **Claude Code** or **GitHub Copilot CLI**, for the plugin.
- **The Rust toolchain**, to install `tsk` with `cargo install`. Install it with one of:

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

**dolt** (version 1.88.2): the default ledger backend stores the ledger in a Dolt
database and runs the external `dolt` binary. Both `SessionStart` hooks install it when it
is absent or another version: the plugin's hook in any repo, and the tsk repo's own
script. Each runs `plugin/hooks/install-dolt.sh`, which installs to `/usr/local/bin` when
that directory is writable, otherwise to `$HOME/.local/bin`, and sets a Dolt identity when
none is set. By hand, download the archive for your platform from
`https://github.com/dolthub/dolt/releases/download/v1.88.2/` (`dolt-linux-amd64.tar.gz`,
`dolt-linux-arm64.tar.gz`, `dolt-darwin-amd64.tar.gz` or `dolt-darwin-arm64.tar.gz`),
unpack it and copy `bin/dolt` to a directory on your `PATH`. Then set an identity:
```bash
dolt version
dolt config --global --add user.name tsk
dolt config --global --add user.email tsk@localhost
```
A ledger set to the `git` backend (`[ledger] backend = "git"` in the tsk config, or
`TSK_LEDGER_BACKEND=git`) does not need `dolt`.

Once the Rust toolchain is installed, verify with:
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

### GitHub Copilot CLI

The same plugin runs in GitHub Copilot CLI. Copilot installs it from the marketplace file
in the tsk repo, `.github/plugin/marketplace.json`, named `tsk`:

```bash
copilot plugin marketplace add jimbarritt/tsk
copilot plugin install tsk@tsk
```

With `tsk` 0.4.0 or later installed, one command runs the same steps, then refreshes the
marketplace and updates the plugin:

```bash
tsk install-plugin copilot-cli
```

It runs, through the `copilot` binary on `PATH`:

1. `copilot plugin marketplace add jimbarritt/tsk`
2. `copilot plugin install tsk@tsk`
3. `copilot plugin marketplace update tsk`
4. `copilot plugin update tsk`

Copilot installs plugins per user, so `--scope` is refused for `copilot-cli`.
`TSK_COPILOT_BIN` set to a path runs that binary in place of the one on `PATH`.

Copilot runs the plugin's hooks from `plugin/hooks.json`:

- `sessionStart` installs `dolt` and `tsk` when needed and prints the ledger context, as
  the Claude Code hook does.
- `agentStop` blocks the end of a turn when the session has no thread binding.

Copilot loads plugins and hooks only in a folder you have trusted. Outside a Claude
cloud session, a thread binds to the code worktree through `.git/tsk-thread-id`.

The Copilot hooks are untested in a live Copilot session. The install, the skills and
the hook commands are tested on their own.

## Upgrading

```bash
cargo install tsk-bin --locked
```

`cargo install` replaces the installed binary with the latest version on crates.io. Add
`--version <version>` to install a given version. The plugin's `SessionStart` hook also
upgrades `tsk` when it is older than the version the plugin requires (`plugin/tsk-version`).

`tsk install-plugin claude-cli` updates the plugin. Claude Code loads plugin hooks and
skills when its process starts, so a new plugin version takes effect in the next new
session, not after `/clear`.

## First session in a repo

1. In the repo's root, run `tsk install-plugin claude-cli`.
2. Start `claude`. The plugin's `SessionStart` hook installs `dolt` and `tsk` when needed,
   then runs `tsk thread session-start`.
3. With no `tsk/ledger` branch on `origin`, the fetch creates a new, local ledger and
   prints a note that the branch does not exist yet. The first `tsk ledger push` creates
   the branch.

## CI

CodeQL static analysis runs on every push to `main` and weekly. Rust requires an
advanced setup (`.github/workflows/codeql.yml`) because CodeQL must compile the code
to analyse it: the default GitHub setup does not support Rust.
