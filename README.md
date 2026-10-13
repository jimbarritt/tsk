# tsk - work with a clear context

tsk holds a repo's missions, threads and continuation state in a ledger, and connects
Claude Code and GitHub Copilot CLI sessions to it. The ledger is a Dolt database, synced
over git on the branch `tsk/ledger`.

## Prerequisites

Install these first, on macOS or Linux:

- **git**, with credentials that can push to the repo you work in.
- **curl**, **tar** and **jq**. The `SessionStart` hooks use them.
- **Rust**, through rustup:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Claude Code** or **GitHub Copilot CLI**, signed in.

The `SessionStart` hooks install the rest: the `tsk` binary when it is missing or out of
date, and `dolt` 1.88.2. See
[docs/user-guide/installation.md](docs/user-guide/installation.md) for each step by hand.

## Use tsk in your own repo

**1. Install tsk.**
```bash
cargo install tsk-bin --locked
```

**2. Install the plugin, from your repo's root.**

For Claude Code:
```bash
cd your-repo
tsk install-plugin claude-cli
```

For GitHub Copilot CLI:
```bash
cd your-repo
tsk install-plugin copilot-cli
```

**3. Start the agent.**
```bash
claude    # or: copilot
```

The plugin's session start hook installs `dolt`, creates a new ledger for the repo and
asks which mission to work. The first `tsk ledger push` creates the `tsk/ledger` branch on
`origin`.

To hold the ledger in a separate nexus repo instead, for a repo you cannot push a ledger
branch to, follow
[docs/user-guide/new-nexus-on-a-clean-machine.md](docs/user-guide/new-nexus-on-a-clean-machine.md).

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
| [docs/user-guide/getting-started.md](docs/user-guide/getting-started.md) | The ledger, thread and external event commands, the TUI, building, publishing |
