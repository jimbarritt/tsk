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

## Getting started (Delta 1)

### Build

```bash
cargo build --workspace --release
```

Binaries land in `target/release/`:
- `tskd` — the daemon
- `tsk` — the unified CLI + TUI

### Usage

**1. Start the daemon** in your project root:

```bash
tskd &
```

The daemon creates a `tsk/` directory in the current folder (committed to source control) and a socket at `/tmp/tsk-{project-hash}.sock`.

**2. Create a thread:**

```bash
tsk thread start fix-login PRIO "Fix the login bug"
```

Priorities: `BG` (background), `PRIO` (priority), `INC` (incident).

Output is JSON — useful for agents and scripting:
```json
{
  "hash": "a3f1b2c4...",
  "short_hash": "a3f1b2c",
  "slug": "fix-login",
  "state": "active",
  "priority": "PRIO",
  "description": "Fix the login bug"
}
```

**3. List threads:**

```bash
tsk thread list
```

**4. Launch the TUI** (no arguments):

```bash
tsk
```

Displays a live table of threads, refreshing every 2 seconds. Press `q` to quit.

### Project storage

Everything under `tsk/` is committed to source control:

```
tsk/
  event-log/
    events.ndjson       # append-only audit trail of all events
  threads/
    index.json          # authoritative thread state
    a3f1b2c-fix-login/  # per-thread context directory
```

### Running tests

```bash
# Unit tests only
cargo test -p tsk-core

# All tests including e2e (builds binaries first)
cargo test --workspace
```

### Publishing to crates.io

This is a Cargo workspace with three crates. They must be published individually in dependency order, since crates.io resolves packages independently (it doesn't understand workspace-local paths).

**Order:** `tsk-core` → `tskd` → `tsk-bin`

**1. Login** (once, stored in `~/.cargo/credentials`):

```bash
cargo login
```

**2. Ensure inter-crate dependencies specify both `path` and `version`.**

Crates.io ignores `path` and uses `version` instead. Both must be present when publishing a crate that depends on another local workspace crate:

```toml
# In cli/Cargo.toml and daemon/Cargo.toml:
tsk-core = { version = "0.1.0", path = "../core" }
```

**3. Publish in order:**

```bash
# Core library first
cargo publish -p tsk-core

# Then the binaries (either order, both depend only on tsk-core)
cargo publish -p tskd
cargo publish -p tsk-bin
```

**4. Bump versions** before each publish. Crates.io rejects re-publishing an existing version. Update `version` in the relevant `Cargo.toml` and update the `version` in any crates that depend on it.

**Notes:**
- `cargo publish --dry-run -p <crate>` to validate packaging without uploading
- New versions appear on crates.io within a few minutes but index propagation can take up to a minute
- The published crate name for the CLI is `tsk-bin` (installs the `tsk` binary)

### How it works

`tskd` is a headless daemon that owns all state. `tsk` is a thin client — in CLI mode it sends a JSON-RPC request over a Unix socket and exits; in TUI mode it polls the daemon to render the thread list. Multiple clients (CLI, TUI, agents) can talk to the daemon concurrently. See `doc/arch/` and `doc/adr/` for the full architecture.
