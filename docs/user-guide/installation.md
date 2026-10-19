# Installation

## Homebrew

On macOS (Apple silicon or Intel) or Linux (x86_64 or arm64) with Homebrew:

```bash
brew install jimbarritt/tap/tsk
```

The formula installs a prebuilt `tsk` binary, with `dolt` and `jq` as dependencies. It
needs no Rust toolchain. `brew upgrade tsk` upgrades it. The release workflow writes the
formula, `Formula/tsk.rb` in `jimbarritt/homebrew-tap`, for every release.

With Homebrew, skip the prerequisites and the `cargo install` step, and go to
[Installation](#installation) for the plugin.

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

**dolt** (version 2.5.0 or later): the default ledger backend stores the ledger in a Dolt
database and runs the external `dolt` binary. Both `SessionStart` hooks install 2.5.0
when no `dolt` 2.5.0 or later is installed: the plugin's hook in any repo, and the tsk
repo's own script. Each runs `plugin/hooks/install-dolt.sh`, which installs to `/usr/local/bin` when
that directory is writable, otherwise to `$HOME/.local/bin`, and sets a Dolt identity when
none is set. By hand, download the archive for your platform from
`https://github.com/dolthub/dolt/releases/download/v2.5.0/` (`dolt-linux-amd64.tar.gz`,
`dolt-linux-arm64.tar.gz`, `dolt-darwin-amd64.tar.gz` or `dolt-darwin-arm64.tar.gz`),
unpack it and copy `bin/dolt` to a directory on your `PATH`. Then set an identity:
```bash
dolt version
dolt config --global --add user.name tsk
dolt config --global --add user.email tsk@localhost
```
Every ledger needs `dolt`. The files backend, which did not, was removed in 0.16.0.

Once the Rust toolchain is installed, verify with:
```bash
rustc --version
cargo --version
```

## Installation

```bash
cargo install tsk-bin --locked
```

This installs the `tsk` binary. `tskd` and the TUI were retired (ADR 0012) and removed in 0.16.0.

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
| `--no-hook` | off | Skip writing the `SessionStart` hook into the repo. |

After the four steps succeed, the command writes the repo's `SessionStart` hook into
`<repo root>/.claude/settings.json`. The repo root comes from
`git rev-parse --show-toplevel`, so the command must run inside a git repo, or take
`--no-hook`. The hook runs `tsk thread session-start --install-plugin claude-cli`. The
command creates the file and `.claude/` when absent, keeps every other key and hook
entry, and does not add a second hook when a `SessionStart` command already contains
`tsk thread session-start`. A settings file that is not valid JSON is an error, and the
command leaves it as it was. Commit `.claude/settings.json` so every clone and every
cloud session of the repo has the hook.

The hook installs the plugin only when `CLAUDE_CODE_REMOTE` is `true`, which is the case
in a Claude Code cloud session. In any other session the flag does nothing. A failed
install step does not stop the session start. It adds a warning to the session context.

`--scope user` does not write the hook, because a user-scope install is not about one
repo. `copilot-cli` does not write it either, because Copilot reads the plugin's hooks.

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
cloud session, a thread binds to the code worktree through a row of the ledger's
`bindings` table.

The Copilot hooks are untested in a live Copilot session. The install, the skills and
the hook commands are tested on their own.

## Upgrading

```bash
tsk self-update
```

`tsk self-update` finds the latest release tag on GitHub and compares it with the running
version. When the running version is the latest, it prints that and exits 0. Otherwise it
upgrades by the method the running binary was installed with:

| Install method | How tsk detects it | What `self-update` runs |
|---|---|---|
| Homebrew | The binary path contains `/Cellar/`, `/homebrew/` or `/linuxbrew/`. | `brew upgrade jimbarritt/tap/tsk` |
| Cargo | The binary is under `$CARGO_HOME/bin` or `~/.cargo/bin`. | `cargo install tsk-bin --locked --version <latest>` |
| Release binary | Any other path. | Downloads the archive for the platform from the GitHub release, then replaces the running binary. |

The release binary method needs `curl` and `tar`. It writes the new binary next to the old
one and renames it over the old one. When the directory is not writable, the command names
the path and asks you to rerun it with sufficient permissions, for example with `sudo`.

`tsk self-update --check` only reports. It prints whether a newer release exists, and never
installs.

`tsk thread session-start` adds a sentence to the session context when a newer release
exists: `tsk <new> is available (running <old>). Run `tsk self-update` to upgrade.` The
lookup stops after 5 seconds, and a failure adds nothing.

The lookup runs at most once in 24 hours. `tsk` keeps the time and the result of the
last lookup in `update-check`, in the tsk state directory
(`${XDG_STATE_HOME:-$HOME/.local/state}/tsk/`). Inside the day, a session start reads
that file and makes no network call. When the file says a newer release exists, the
sentence above is followed by `The release check is cached: it last ran <n> hours ago,
and runs once a day. Set TSK_UPDATE_CHECK=always to check now.` A new release can reach
you up to a day late. `tsk self-update` always asks the network.

`TSK_UPDATE_CHECK` changes the rule:

- `always` looks up the release at every session start.
- `never` skips the lookup. `TSK_NO_UPDATE_CHECK=1` does the same.

### Automatic update for a newer ledger

A ledger that a newer tsk has upgraded stops an older tsk with `unsupported ledger version
N`. From 0.18.0, tsk handles that itself. It prints `warning: this ledger is at layout N
and this tsk (X) supports up to M. Updating tsk now.` to stderr, runs the update above,
and runs the same command again with the updated binary. The command keeps its stdin and
stdout and its exit status. A Cargo install compiles during this, which takes about a
minute.

- `TSK_AUTO_UPDATED=1` marks the second run. If that run still meets an unsupported layout,
  it stops with the original error and `tsk was updated but still does not support layout
  N; update it by hand: tsk self-update`.
- When the update fails, or leaves the version unchanged, tsk prints that error and then
  the original one, and exits 1.
- `tsk thread session-start` and `tsk thread guard` never update. They keep their timeouts.
- `TSK_AUTO_UPDATE=never` or `TSK_NO_UPDATE_CHECK=1` switches it off. `always` is the
  default.
- Only the unsupported-layout error starts an update. A network error or a merge conflict
  does not.
- A tsk newer than the ledger reads an old layout as before and upgrades it on its first
  write. A read upgrades nothing.
- A binary released before 0.18.0 cannot do this. Run `tsk self-update` for it.

In a Claude Code cloud session, `tsk thread session-start --install-plugin claude-cli`
runs the upgrade before it installs the plugin. A failed upgrade adds a warning to the
context and the session still starts. The session in which tsk upgrades itself carries on
with the old binary. The next command uses the new one. No upgrade runs outside cloud
sessions.

You can also upgrade with `cargo install tsk-bin --locked` or `brew upgrade jimbarritt/tap/tsk`
directly. The plugin's `SessionStart` hook also upgrades `tsk` when it is older than the
version the plugin requires (`plugin/tsk-version`).

`tsk install-plugin claude-cli` updates the plugin. Claude Code loads plugin hooks and
skills when its process starts, so a new plugin version takes effect in the next new
session, not after `/clear`.

## First session in a repo

1. In the repo's root, run `tsk install-plugin claude-cli`.
2. Start `claude`. The plugin's `SessionStart` hook installs `dolt` and `tsk` when needed,
   then runs `tsk thread session-start`.
3. With no ledger ref on `origin`, the fetch creates a new, local ledger and prints a
   note that the ref does not exist yet. The first `tsk ledger push` creates it. A new
   ledger uses a custom ref, `refs/tsk/ledger`, which GitHub does not list as a branch.
   Run `tsk config ledger-refs` to see the refs in use. A repo used from Claude Code
   cloud sessions needs branches, because the cloud proxy refuses to push a custom ref:
   run `tsk config ledger-refs branches`, commit `.tsk/config.toml`, and do it before the
   first push. In a cloud session a new ledger takes branches by itself and says so.

## CI

CodeQL static analysis runs on every push to `main` and weekly. Rust requires an
advanced setup (`.github/workflows/codeql.yml`) because CodeQL must compile the code
to analyse it: the default GitHub setup does not support Rust.
