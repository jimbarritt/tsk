# tsk - work with a clear context
tsk is about tasks. Big and small. From working out your next sequence of moves in the codebase to planning 
large scale cross team initiatives.

It is also designed from the ground up to work in the world of agentic engineering. With a native MCP you can have tsk 
running alongside your agent and keep track of where you both are, or where youre *teams* of agents are for that matter. Or even where you, your human colleagues and their agents are!

It's your "sat nav" for work. Keep your human cognitive context and that of your agents clear and keep track of all the work threads you are context switching to.

## The foundations

At the core of the domain of tsk are four dimensions which are facets of any software engineering delivery. Where tsk 
is different is that it models all four of these dimensions explicitly. Other tools you might be used to like linear or jira 
only model some parts of these dimensions, and end up being a little too abstract (in the wrong direction) to really give
 a holistic abstraction.

tsk is very opinionated but within a very specific abstraction. It has a lot of flexibility but in the right dimensions.

The four dimensions are:

- Navigation
- Deltas
- Product
- Scale

## Prerequisites

tsk is written in Rust. You need the Rust toolchain installed before building or installing.

**rustup** (recommended — official installer, works everywhere):
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

Same command — `cargo install` replaces the existing binaries:

```bash
cargo install tsk-bin tskd
```

This installs two binaries: `tsk` (CLI + TUI) and `tskd` (daemon).

## Getting started

### Usage

**1. Start the daemon** in your project root:

```bash
tskd &
```

The daemon creates a `tsk/` directory in the current folder (committed to source control) and a socket at `/tmp/tsk-{project-hash}.sock`.

**2. Create a thread:**

```bash
tsk thread create fix-login PRIO "Fix the login bug"
```

Priorities: `BG` (background), `PRIO` (priority), `INC` (incident).

Output is JSON — useful for agents and scripting:
```json
{
  "id": 1,
  "slug": "fix-login",
  "state": "paused",
  "priority": "PRIO",
  "description": "Fix the login bug",
  "dir": "/your/project/tsk/threads/0001-fix-login"
}
```

New threads start paused. Use `switch-to` to activate one.

**3. Switch to a thread:**

```bash
tsk thread switch-to 1        # by id
tsk thread switch-to fix-login # by slug
```

**4. Update a thread:**

```bash
tsk thread update fix-login --description "New description"
tsk thread update fix-login --slug new-slug
tsk thread update fix-login --priority BG
```

All flags are optional — only the fields you pass are changed. If you change the slug, the thread directory is renamed automatically.

**5. List threads:**

```bash
tsk thread list
```

**6. Launch the TUI** (no arguments):

```bash
tsk
```

Displays threads grouped by section (Active / Priority & Incidents / Background). Updates live when the CLI makes changes. Use `j`/`k` to scroll, `ctrl-d`/`ctrl-u` to page, `gg`/`G` to jump to top/bottom, `?` for keybindings, `q` to quit.

### Project storage

Everything under `tsk/` is committed to source control:

```
tsk/
  event-log/
    events.ndjson        # append-only audit trail of all events
  threads/
    index.json           # authoritative thread state
    0001-fix-login/      # per-thread context directory
```

### Running tests

```bash
# Unit tests only
cargo test -p tsk-core

# All tests including e2e (requires cargo build --workspace first)
cargo test --workspace
```

### Building from source

```bash
cargo build --workspace --release
```

Binaries land in `target/release/`: `tsk` and `tskd`.

Or install locally with `just`:

```bash
just build-install   # builds and installs to ~/.cargo/bin
just test            # run all tests
just publish         # publish all crates to crates.io (bump versions first)
```

### Publishing to crates.io

Bump the `version` field in each `Cargo.toml` that changed, then:

```bash
just publish
```

This publishes `tsk-core` first, waits 30 seconds for crates.io to index it, then publishes `tsk-bin` and `tskd`. The published crate name for the CLI is `tsk-bin` (it installs the `tsk` binary).

### How it works

`tskd` is a headless daemon that owns all state. `tsk` is a thin client — in CLI mode it sends a JSON-RPC request over a Unix socket and exits; in TUI mode it watches `tsk/threads/index.json` for changes and re-renders instantly. Multiple clients (CLI, TUI, agents) can talk to the daemon concurrently. See `doc/arch/` and `doc/adr/` for the full architecture.
