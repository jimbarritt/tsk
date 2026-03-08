# Delta 1: Foundation

The first working slice of tsk. At the end of this delta, a user can start the daemon, create and list threads via the CLI, and see them rendered in a TUI.

## Goal

A human can type:

```
tskd &                                          # start daemon
tsk thread start fix-login PRIO "Fix the login bug"    # create a thread
tsk thread list                                  # list threads (JSON output)
tsk                                              # launch TUI showing threads
```

## Design Decisions

These were resolved during planning and are locked in for Delta 1:

- **Storage is project-scoped** — `{project.root}/tsk/` directory, committed to source control
- **`index.json` is authoritative state** — not a cache. Committed to git. The event log is an audit trail / history
- **Thread ID** — SHA-256 hash of the slug. First 7 chars used as short ID (like git). Full hash stored in thread data
- **Thread dirs** — `{short-hash}-{slug}/` with 7-char hash prefix, e.g. `a3f1b2c-fix-login/`
- **Priority** — enum with abbreviations: `BG` (background), `PRIO` (priority), `INC` (incident). Abbreviations used in CLI commands and TUI display. Expandable to full names where needed
- **Socket path** — `/tmp/tsk-{project-hash}.sock` (runtime only, not committed, supports multiple projects)
- **No tokio** — `std::thread` for Delta 1
- **CLI output is JSON** — humans use the TUI; CLI is for agents and scripting

## Project-Scoped Storage

The daemon creates this structure on startup if it doesn't exist:

```
{project.root}/tsk/
  event-log/
    events.ndjson           # append-only domain events (audit trail)
  threads/
    index.json              # authoritative thread state (committed to git)
    a3f1b2c-fix-login/      # per-thread context directory
    e802b0a-update-deps/    # stores context files for this thread
```

- `event-log/events.ndjson` — every domain event (ThreadStarted, etc.) as NDJSON. Append-only. History and audit trail
- `threads/index.json` — current state of all threads. Written by the daemon on every state change. This is what gets committed and shared via git
- `threads/{hash}-{slug}/` — per-thread directory for context files (notes, agent logs, etc.)

## Project Structure (Cargo Workspace)

```
tsk/                        # repo root
  Cargo.toml                # workspace root
  core/
    Cargo.toml              # tsk-core library crate
    src/
      lib.rs                # shared types and protocol
  cli/
    Cargo.toml              # tsk binary (unified CLI + TUI)
    src/
      main.rs               # entrypoint: subcommands → CLI, no args → TUI
    tests/
      e2e.rs                # end-to-end tests
  daemon/
    Cargo.toml              # tskd binary
    src/
      main.rs               # daemon entrypoint
```

**Dependencies:**
- `serde`, `serde_json` — serialisation
- `sha2` — SHA-256 hashing for thread IDs
- `clap` — CLI argument parsing (with subcommands)
- `ratatui`, `crossterm` — TUI rendering

## Deliverables

### 1. Shared library (`tsk-core`)

The core types and protocol shared between binaries:

- **Thread model**: `Thread { hash: String, short_hash: String, slug: String, state: ThreadState, priority: Priority, description: String }`
- **Priority enum**: `BG`, `PRIO`, `INC` — serialises to/from abbreviations, can expand to full names
- **Thread ID generation**: `fn thread_hash(slug: &str) -> (String, String)` — returns (full SHA-256 hex, first 7 chars)
- **Event types**: `ThreadStarted { hash, slug, priority, description, timestamp }`
- **JSON-RPC types**: `Request`, `Response`, `Error` structs with serde
- **Socket path resolution**: `fn socket_path(project_root: &Path) -> PathBuf` — hashes project root to produce `/tmp/tsk-{hash}.sock`
- **Client helper**: `fn send_request(socket: &Path, method, params) -> Result<Value>` — connect to socket, send JSON-RPC, read response
- **Storage paths**: `fn tsk_dir(project_root: &Path) -> PathBuf` etc.

### 2. Daemon (`tskd`)

Minimal daemon that:

- Takes project root as an argument (or discovers it)
- Creates `tsk/event-log/` and `tsk/threads/` directories if they don't exist
- Creates and listens on a Unix domain socket at `/tmp/tsk-{project-hash}.sock`
- Accepts connections (one `std::thread` per connection)
- Parses JSON-RPC requests
- Handles two methods:
  - `thread.start` — validates params, computes hash from slug, appends event to `events.ndjson`, creates thread dir `{short-hash}-{slug}/`, writes updated `index.json`, returns the thread
  - `thread.list` — reads `index.json`, returns all threads
- Loads state from `index.json` on startup
- Clean shutdown on SIGTERM/SIGINT (removes socket file)

**Not in scope for Delta 1:**
- SQLite cache
- systemd/launchd integration
- `thread.switch_to`
- Authentication or access control

### 3. CLI mode (`tsk <command>`)

Subcommand routing via clap:

- `tsk thread start <slug> <priority> <description>` — sends `thread.start` to daemon, prints JSON result
- `tsk thread list` — sends `thread.list` to daemon, prints JSON result

Error handling:
- If daemon is not running, print a clear error: `"tskd is not running. Start it with: tskd"`
- If command fails, print the JSON-RPC error message

### 4. TUI mode (`tsk` with no args)

Minimal ratatui TUI that:

- Connects to the daemon socket
- Sends `thread.list` query on startup
- Renders a table of threads (short hash, slug, state, priority abbreviation, description)
- Polls/refreshes on a timer (e.g. every 2 seconds)
- Exits on `q` keypress

**Not in scope for Delta 1:**
- Sending commands from TUI
- Complex layouts or multiple panes
- Thread detail view

## Implementation Order

Test-driven, red-green-refactor. Write tests first, see them fail, then implement.

### Step 1: Workspace setup

Set up the Cargo workspace with three crates (`core`, `cli`, `daemon`). Establish the skeleton — `lib.rs`, `main.rs` files that compile but do nothing.

### Step 2: Shared types — unit tests first

Write unit tests in `core/src/lib.rs` for:

- `thread_hash("fix-login")` returns a 64-char hex string and 7-char short hash
- Same slug always produces the same hash (deterministic)
- `Priority` serialises to `"BG"`, `"PRIO"`, `"INC"` and deserialises back
- `Priority` expands to full names (`background`, `priority`, `incident`)
- JSON-RPC `Request` serialisation/deserialisation
- JSON-RPC `Response` (success) serialisation/deserialisation
- JSON-RPC `Response` (error) serialisation/deserialisation
- `ThreadStarted` event serialisation/deserialisation
- `Thread` model serialisation (includes `hash` and `short_hash` fields)
- `socket_path()` returns a path under `/tmp/` containing a hash of the project root

Run tests — they all fail (red). Then implement the types and protocol until tests pass (green).

### Step 3: E2E test skeleton — tests first

Write e2e tests in `cli/tests/e2e.rs` that:

1. Create a temp directory as project root
2. Start `tskd` as a subprocess pointing at the temp project root
3. Wait for socket to appear
4. Send a `thread.start` command via the client helper with slug `"fix-login"`, priority `PRIO`, description `"Fix the login bug"`
5. Assert the response contains a thread with slug `"fix-login"` and a 7-char short hash
6. Assert `tsk/threads/index.json` exists and contains the thread
7. Assert `tsk/threads/{short-hash}-fix-login/` directory was created
8. Assert `tsk/event-log/events.ndjson` contains a `ThreadStarted` event
9. Send a `thread.list` query
10. Assert the response contains the thread created above
11. Kill the daemon subprocess, clean up

Run tests — they fail because the daemon doesn't exist yet (red).

### Step 4: Daemon implementation

Implement `tskd` until the e2e tests pass:

- Directory creation (`tsk/event-log/`, `tsk/threads/`)
- Socket listener
- JSON-RPC request parsing
- `thread.start` handler (validate, compute hash, append event, create thread dir, write `index.json`, respond)
- `thread.list` handler (read `index.json`, respond)
- State load from `index.json` on startup
- Clean shutdown

### Step 5: CLI implementation

Write e2e tests that exercise the full CLI binary:

1. Start `tskd` subprocess with temp project root
2. Run `tsk thread start fix-login PRIO "Fix login"` as a subprocess
3. Assert stdout is valid JSON containing the created thread
4. Run `tsk thread list` as a subprocess
5. Assert stdout is valid JSON containing `fix-login`

Run tests — they fail (red). Then implement the clap subcommands until tests pass (green).

### Step 6: TUI implementation

The TUI is hard to test automatically. Implementation approach:

1. Write a unit test that the "no args" code path calls the TUI launcher (can test routing without rendering)
2. Implement the ratatui rendering: connect to daemon, query `thread.list`, render table
3. Manual verification: start daemon, create threads, launch `tsk`, visually confirm

## Testing Strategy

- **Unit tests** (`core/`) — test-driven for all shared types: hashing, priority enum, JSON-RPC serialisation, event types, thread model, socket path
- **E2e tests** (`cli/tests/e2e.rs`) — test-driven for the full stack: start daemon subprocess, send commands (via client helper and via CLI binary), verify responses and filesystem side-effects
- **Manual testing** for the TUI — visual verification is acceptable for Delta 1

All tests are written before implementation. See them fail first, then make them pass.
