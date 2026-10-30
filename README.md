# tsk - work with a clear context

**The objective of tsk is to scale humans.** The less supervision the work needs, the more
work one human delegates to agents, and the more one human produces.

Reducing context bloat is an optimisation. It can serve a separate objective: to reduce
overall token costs.

tsk is about navigating software delivery, for people and agents together.

It keeps track of where you and your agents are: which piece of work each of you is
on, what it is for, what is decided, what is still open, and where a line of work
stopped so the next session picks it up. Work spread across many people and many
sessions stays in one place that all of them read and write.

tsk is open source, under the [MIT licence](LICENSE).

**If you are an agent, read [README-AGENT.md](README-AGENT.md) first.**

```bash
brew install jimbarritt/tap/tsk
tsk install-plugin claude-cli    # in your repo. Or: tsk install-plugin copilot-cli
```

The full instructions are in [Install](#install).

## Contents

- [Four dimensions of delivery](#four-dimensions-of-delivery)
- [How tsk works](#how-tsk-works)
- [Install](#install)
- [Work on tsk itself](#work-on-tsk-itself)
- [Documentation](#documentation)

## Four dimensions of delivery

tsk is built on a [conceptual model](docs/domain/domain-model-overview.md) which
defines four dimensions of software delivery. tsk models each one explicitly. Other tools
model parts of them.

- **Navigation.** Where am I, where am I going, where have I been. The route through the
  work, not only the list of destinations.
- **Delta.** An observable change to the production system: what differs between the
  system yesterday and the system now. A story is temporary; the delta is what persists.
- **Product.** The thing being changed: product maps, metrics, signals from customers.
  Other tools put this outside the engine, in a wiki that is soon out of date.
- **Scale.** One model at every zoom level, from a single commit to a whole programme,
  with no change of type between an epic, a story and a sub-task.

The definitions, and the alternatives rejected, are in
[docs/domain/ubiquitous-language.md](docs/domain/ubiquitous-language.md). The reasoning
is in [docs/vision.md](docs/vision.md).

## How tsk works

A repo gets a **ledger**: a database of records, synced through git alongside the code.
The records are:

- A **mission**: an outcome to achieve, with its purpose, its constraints, what is out
  of scope, and who decides what. A mission states the outcome, not the method, and the
  agent or the person who takes it works out the mechanism. This is loosely based
  on [military command doctrine](docs/domain/mission-model.md): the order states the
  objective, not the route.
- **Tasks**, **decisions** and **open questions**, each a numbered record of its mission.
  A decision records what was chosen and why, in one line. A question stays open until
  someone answers it.
- A **thread**: one line of work, paused and resumed across sessions. When a session
  pauses a thread, it records the mission, the task in progress and where things stand.
  The next session, agent or human, resumes the thread and continues.
- **Documents**: research, reports and designs, as Markdown, linked to the mission they
  serve.
- A **nexus**: a separate repo that contains the ledgers of several repos, for work you
  cannot push a ledger to or that spans repos. See
  [docs/domain/territory-and-nexus.md](docs/domain/territory-and-nexus.md).

Everything is written and read through `tsk` commands, so it is queryable:

```bash
tsk mission list            # every mission, its status and what blocks it
tsk question list           # every open question, across every mission
tsk thread list             # every thread and who is working it
```

A ledger is the point of coordination for multiple people or agents running multiple
sessions, which can be across different worktrees. Each writes locally and publishes at
the points that matter: when it claims work, when it pauses, when the session ends.
Writes from different actors merge by row, and a push that collides on a number
renumbers and prints each change.

## Install

On macOS or Linux with [Homebrew](https://brew.sh):

```bash
brew install jimbarritt/tap/tsk
```

Then, in the git repo you work in. The command stops outside a git repo, because tsk
works per repo.

```bash
tsk doctor                       # check the environment; it prints the fix for each fault
tsk install-plugin claude-cli    # or: tsk install-plugin copilot-cli
claude                           # or: copilot
```

To add the `swe` plugin (Software English checks) in the same step, run
`tsk install-plugin claude-cli --plugin tsk --plugin swe`. It installs at project scope,
for this repo only. `claude plugin install` on its own defaults to user scope, which
applies to every repo on the machine.

A copy of `tsk` from an earlier install, for example `~/.cargo/bin/tsk` after
`cargo install`, shadows a new one when it precedes the new one on `PATH`. A stale copy lacks the
commands of later releases. `tsk doctor` lists every `tsk` on `PATH` with its version and
names the one that runs. `tsk self-update` prints the same note.

That is the whole setup. The plugin installs into Claude Code or GitHub Copilot CLI,
and at the start of every session it fetches the ledger and prints the missions, with a
prompt to pick one or to resume a thread. The first push creates the ledger on `origin`.

Without Homebrew: `cargo install tsk-bin --locked`, with `dolt` and `jq` on `PATH`. See
[docs/user-guide/installation.md](docs/user-guide/installation.md).

**Claude Code cloud sessions.** A cloud session starts in a new container, so each
environment has a setup script that installs `tsk`. The same `tsk install-plugin`
command above also commits a hook that installs the plugin in every cloud session. The
script and the hook are in
[docs/user-guide/new-cloud-environment.md](docs/user-guide/new-cloud-environment.md).

**A repo you cannot push a ledger to.** Keep its ledger in a separate nexus repo:
[docs/user-guide/new-nexus-on-a-clean-machine.md](docs/user-guide/new-nexus-on-a-clean-machine.md).

## Work on tsk itself

```bash
git clone https://github.com/jimbarritt/tsk.git
cd tsk
claude
```

Then say "hi". The repo's session start script builds `tsk`, fetches the ledger and
prints the missions. tsk tracks its own development in its own ledger.

## Documentation

| Doc | Covers |
|---|---|
| [README-AGENT.md](README-AGENT.md) | What an agent needs to work with the ledger |
| [docs/index.md](docs/index.md) | Full documentation index |
| [docs/vision.md](docs/vision.md) | Why tsk exists and what it models |
| [docs/user-guide/introducing-tsk.md](docs/user-guide/introducing-tsk.md) | The story behind why tsk exists |
| [docs/user-guide/missions-threads-and-continuation.md](docs/user-guide/missions-threads-and-continuation.md) | How missions, threads and continuation state fit together |
| [docs/user-guide/getting-started.md](docs/user-guide/getting-started.md) | The commands, building, publishing |
| [docs/user-guide/checking-ledger-documents.md](docs/user-guide/checking-ledger-documents.md) | Run a check, such as a prose lint, on ledger documents, also in a repo you do not control |
| [docs/user-guide/installation.md](docs/user-guide/installation.md) | Prerequisites, installing, upgrading |
| [docs/user-guide/new-cloud-environment.md](docs/user-guide/new-cloud-environment.md) | A Claude cloud environment with tsk in every session |
| [docs/user-guide/new-nexus-on-a-clean-machine.md](docs/user-guide/new-nexus-on-a-clean-machine.md) | A nexus, with a repo's ledger in it |
