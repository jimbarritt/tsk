# tsk - work with a clear context

tsk keeps the work of a repo in a ledger: missions, their tasks, decisions and questions,
and the threads that work them. Claude Code and GitHub Copilot CLI sessions read and
write the ledger through `tsk` commands, so a session starts with the state of the work,
and the state it writes is there for the next one. The ledger is a Dolt database, synced through a
git ref on the repo. Research and reports are Markdown documents on a second ref.

**If you are an agent, read [README-AGENT.md](README-AGENT.md) first.**

Set up a repo the way that matches where your sessions run: on a laptop (section 1), or
in Claude Code cloud sessions (section 2).

## 1. Set up on a laptop, in a terminal

You need git with credentials that can push to the repo, and Claude Code or GitHub
Copilot CLI, signed in.

**Install tsk.** On macOS or Linux with [Homebrew](https://brew.sh):

```bash
brew install jimbarritt/tap/tsk
```

Homebrew installs `dolt` and `jq` with it. Without Homebrew, see
[docs/user-guide/installation.md](docs/user-guide/installation.md).

**Install the plugin in your repo, and start your agent:**

```bash
cd your-repo
tsk install-plugin claude-cli    # or: tsk install-plugin copilot-cli
claude                           # or: copilot
```

The plugin's session start hook creates a ledger for the repo and prints the missions
with a prompt to choose one. The first `tsk ledger push` creates the ledger ref on `origin`.

**If the same repo is also used from Claude cloud sessions,** choose branches for the
ledger before the first push, and commit the choice:

```bash
tsk config ledger-refs branches
git add .tsk/config.toml && git commit -m "Keep the tsk ledger on branches" && git push
```

A new ledger uses a custom git ref, which GitHub does not list as a branch. A push of a
custom ref fails at the cloud proxy, so a repo used from the cloud keeps its ledger on
branches instead.

## 2. Set up for Claude Code cloud sessions

A cloud session starts in a new container with nothing installed. tsk works there after
one setup step per environment and one per repo.

**Once per environment:** add the setup script from
[docs/user-guide/new-cloud-environment.md](docs/user-guide/new-cloud-environment.md) to
the environment. It installs `tsk` and `dolt`.

**Once per repo:** from any session or terminal in the repo, run:

```bash
tsk install-plugin claude-cli
tsk config ledger-refs branches
git add .claude/settings.json .tsk/config.toml
git commit -m "Set up tsk for cloud sessions" && git push
```

The first command writes a `SessionStart` hook into `.claude/settings.json`. In every
cloud session the hook installs the plugin, fetches the ledger and prints the missions
with a prompt to choose one. The second command keeps the ledger on branches, which the
cloud proxy accepts.

The guide covers both steps in full, with what to check in the first session.

## Keep the ledger outside the repo

For a repo you cannot push a ledger ref to, keep its ledger in a separate nexus repo.
Follow
[docs/user-guide/new-nexus-on-a-clean-machine.md](docs/user-guide/new-nexus-on-a-clean-machine.md).

## Work on tsk itself

```bash
git clone https://github.com/jimbarritt/tsk.git
cd tsk
claude
```

Then say "hi". The repo's `SessionStart` script installs the plugins and `dolt`, builds
`tsk` from `cli/`, fetches the ledger and prints the missions with a prompt to choose one.

## Documentation

| Doc | Covers |
|---|---|
| [README-AGENT.md](README-AGENT.md) | What an agent needs to know to work with the ledger |
| [docs/index.md](docs/index.md) | Full documentation index |
| [docs/vision.md](docs/vision.md) | Why tsk exists and what it models |
| [docs/user-guide/installation.md](docs/user-guide/installation.md) | Prerequisites, installing, upgrading, CI |
| [docs/user-guide/new-cloud-environment.md](docs/user-guide/new-cloud-environment.md) | Step by step: a Claude cloud environment with tsk ready in every session |
| [docs/user-guide/new-nexus-on-a-clean-machine.md](docs/user-guide/new-nexus-on-a-clean-machine.md) | A new nexus, with a repo's ledger in it |
| [docs/user-guide/getting-started.md](docs/user-guide/getting-started.md) | The ledger, thread and external event commands, building, publishing |
