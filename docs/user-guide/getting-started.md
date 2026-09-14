# Getting started

## Usage

**1. Start the daemon** once per user session:

```bash
tskd &
```

The daemon stores all state in `~/.tsk/` and listens on `~/.tsk/tskd.sock`. You only need
one daemon runs, and it serves all your projects.

**2. Create a thread:**

```bash
tsk thread create fix-login PRIO "Fix the login bug"
```

Priorities: `BG` (background), `PRIO` (priority), `INC` (incident).

Output is JSON, useful for agents and scripting:
```json
{
  "id": 1,
  "slug": "fix-login",
  "state": "paused",
  "priority": "PRIO",
  "description": "Fix the login bug",
  "dir": "/home/user/.tsk/threads/0001-fix-login"
}
```

New threads start paused. Use `switch-to` to activate one.

**3. Switch to a thread:**

```bash
tsk thread switch-to 1         # by id
tsk thread switch-to fix-login  # by slug
```

**4. Update a thread:**

```bash
tsk thread update fix-login --description "New description"
tsk thread update fix-login --slug new-slug
tsk thread update fix-login --priority BG
tsk thread update fix-login --path /abs/path/to/project   # bind to a project directory
tsk thread update fix-login --path ""                      # clear the binding
```

All flags are optional: only the fields you pass are changed. If you change the slug, the thread directory is renamed automatically.

**5. List threads:**

```bash
tsk thread list
```

**6. Find the thread bound to the current directory:**

```bash
tsk where
```

**7. Launch the TUI** (no arguments):

```bash
tsk
```

Displays threads grouped by section (Active / Priority & Incidents / Background). Updates live when the CLI makes changes. Use `j`/`k` to scroll, `ctrl-d`/`ctrl-u` to page, `gg`/`G` to jump to top/bottom, `?` for keybindings, `q` to quit.

## Global storage

All tsk state is stored in `~/.tsk/`, not inside your projects:

```
~/.tsk/
  tskd.sock              # Unix socket (present while daemon is running)
  event-log/
    events.ndjson        # append-only audit trail of all events
  threads/
    index.json           # authoritative thread state
    0001-fix-login/      # per-thread context directory
```

## Binding threads to projects

A thread can be bound to a project directory with `--path`:

```bash
tsk thread create fix-login PRIO "Fix the login bug" --path /abs/path/to/project
```

From inside that directory, `tsk where` shows the bound thread. If the project has a
`doc/tsk/` directory, `tsk` auto-zooms to the bound thread when run from that project.
You can commit project-local context files in `doc/tsk/` alongside the global thread
context in `~/.tsk/threads/{id}-{slug}/`.

## Running tests

```bash
# Unit tests only
cargo test -p tsk-core

# All tests including e2e (requires cargo build --workspace first)
cargo test --workspace
```

## Building from source

```bash
cargo build --workspace --release
```

Binaries arrive in `target/release/`: `tsk` and `tskd`.

Or install locally with `just`:

```bash
just build-install   # builds and installs to ~/.cargo/bin
just test            # run all tests
just publish         # publish all crates to crates.io
```

## Publishing to crates.io

Bump the version with `just bump`, commit, then publish:

```bash
just bump 0.1.7   # requires cargo-edit: cargo install cargo-edit
git add -p && git commit -m "Bumping version to 0.1.7"
just publish
```

This publishes `tsk-core` first, waits 30 seconds for crates.io to index it, then publishes `tsk-bin` and `tskd`. The published crate name for the CLI is `tsk-bin` (it installs the `tsk` binary).
