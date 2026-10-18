# tsk - work with a clear context

tsk holds a repo's missions, threads and continuation state in a ledger, and connects
Claude Code and GitHub Copilot CLI sessions to it. The ledger is a Dolt database, synced
over git on the branch `tsk/ledger`. Missions, tasks, decisions and
questions are records in it, written through `tsk` commands and read with
`tsk mission list`. Research and reports are Markdown documents on the branch `tsk/docs`.

## Install with Homebrew

On macOS or Linux with [Homebrew](https://brew.sh):

```bash
brew install jimbarritt/tap/tsk
```

Homebrew installs `dolt` and `jq` with it. Then, from your repo's root, install the plugin
for your agent and start it:

```bash
cd your-repo
tsk install-plugin claude-cli    # or: tsk install-plugin copilot-cli
claude                           # or: copilot
```

The plugin's session start hook creates a new ledger for the repo and asks which mission
to work. The first `tsk ledger push` creates the `tsk/ledger` branch on `origin`. You
also need git, with credentials that can push to the repo, and Claude Code or GitHub
Copilot CLI, signed in.

To hold the ledger in a separate nexus repo instead, for a repo you cannot push a ledger
branch to, follow
[docs/user-guide/new-nexus-on-a-clean-machine.md](docs/user-guide/new-nexus-on-a-clean-machine.md).

## Use tsk in Claude Code cloud sessions

A cloud session starts in a new container, with no plugin installed. Two things make tsk
work there:

1. **Once per environment:** a setup script that installs `tsk` and `dolt`.
2. **Once per repo:** a `SessionStart` hook in the repo's `.claude/settings.json`, which
   installs the plugin at the start of every cloud session. `tsk install-plugin
   claude-cli` writes it. Commit the file.

Follow [docs/user-guide/new-cloud-environment.md](docs/user-guide/new-cloud-environment.md)
for both.

## Install without Homebrew

Install these first:

- **git**, with credentials that can push to the repo you work in.
- **curl**, **tar** and **jq**. The session start hooks use them.
- **Rust**, through rustup:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Claude Code** or **GitHub Copilot CLI**, signed in.

Then:

```bash
cargo install tsk-bin --locked
cd your-repo
tsk install-plugin claude-cli    # or: tsk install-plugin copilot-cli
claude                           # or: copilot
```

The plugin's session start hook installs `dolt` 1.88.2 when no `dolt` 1.88.2 or later is
installed. See [docs/user-guide/installation.md](docs/user-guide/installation.md) for each
step by hand.

## Work on tsk itself

**1. Clone this repo.**
```bash
git clone https://github.com/jimbarritt/tsk.git
cd tsk
```

**2. Start Claude Code.**
```bash
claude
```

**3. Say "hi".**
```
hi
```

The repo's `SessionStart` script installs the plugins and `dolt`, builds `tsk` from
`cli/`, fetches the ledger and asks which mission to work.

## Documentation

| Doc | Covers |
|---|---|
| [docs/index.md](docs/index.md) | Full documentation index |
| [docs/vision.md](docs/vision.md) | Why tsk exists, the four dimensions of work |
| [docs/user-guide/installation.md](docs/user-guide/installation.md) | Prerequisites, installing, upgrading, CI |
| [docs/user-guide/new-nexus-on-a-clean-machine.md](docs/user-guide/new-nexus-on-a-clean-machine.md) | Step by step: a new nexus, tsk and the plugin on a clean machine, and a repo's ledger held in the nexus |
| [docs/user-guide/new-cloud-environment.md](docs/user-guide/new-cloud-environment.md) | Step by step: a Claude cloud environment with tsk, the plugin and the ledger ready in every session |
| [docs/user-guide/getting-started.md](docs/user-guide/getting-started.md) | The ledger, thread and external event commands, building, publishing |
