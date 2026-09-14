2026-03-08T07-49-54Z-ipc-architecture.v3.md
project: tsk

# IPC Architecture

Supersedes: 2026-03-07T21-41-38Z-ipc-architecture.v2.md

## Overview

tsk uses a client-daemon architecture over a Unix domain socket with JSON-RPC 2.0 as the message protocol. The daemon process (`tskd`) is the single authority — it owns the event log, manages state, and serves requests. The CLI and TUI are both clients of the daemon. The pattern follows Docker's architecture: stateless CLI, fat daemon, Unix socket between them.

## Processes

### `tskd` (daemon)

- A headless background process. Can be started on login via systemd/launchd.
- Listens on a Unix domain socket at `/tmp/tsk-{project-hash}.sock` (hash of project root, supports multiple projects).
- Accepts connections from multiple clients concurrently.
- On startup, creates `{project.root}/tsk/event-log/` and `{project.root}/tsk/threads/` if they don't exist.
- Processes commands, applies state transitions, appends to the NDJSON event log, and writes updated `index.json`.
- Serves queries by reading from `index.json` (authoritative state, committed to git).
- Is the single writer to both the event log and `index.json` — no concurrency issues.

### `tsk` (unified CLI + TUI)

A single binary with two modes (see [ADR 0004](../adr/0004-unified-tsk-binary.md)):

**CLI mode** (`tsk <command>`):
- A short-lived process. Runs a command like `tsk thread start <slug> <priority> <description>`.
- Connects to daemon socket, sends a JSON-RPC request, reads the response, exits.
- Blazing fast — the overhead is negligible compared to the work the agent or human will do next.
- On `thread start`, prints the created thread directory path to stdout so the calling agent can capture it.

**TUI mode** (`tsk` with no arguments):
- Launches a persistent ratatui-based interactive interface.
- Also a client of the daemon — connects to the same socket.
- Sends commands and queries state via JSON-RPC. Pure CQRS: fire commands, then re-query state to render.
- The TUI is optional. If it's not running, the CLI and agents still work because the daemon is always there.

### Agents (future clients)

- Same protocol as CLI — connect to socket, send JSON-RPC request, read response, disconnect.
- Multiple agents can send commands concurrently. The daemon serialises them into the log.

## Crate Structure

A Cargo workspace with three crates producing two binaries:

```
Cargo.toml          # workspace root
core/
  Cargo.toml        # tsk-core library crate
  src/
    lib.rs          # shared: event types, JSON-RPC message types, socket protocol, serde
cli/
  Cargo.toml        # tsk binary (unified CLI + TUI)
  src/
    main.rs         # CLI subcommands + TUI (no args)
  tests/
    e2e.rs          # end-to-end tests
daemon/
  Cargo.toml        # tskd binary
  src/
    main.rs         # daemon entrypoint
```

Three crates, two binaries. Published to crates.io as `tsk-bin`.

## Protocol: JSON-RPC 2.0

- Transport: Unix domain stream socket (bidirectional).
- Framing: NDJSON (one JSON-RPC message per line, newline delimited).
- Every request has an `id` field; the response echoes it back. This allows pipelining multiple requests on a single connection.
- Standard error format with codes for consistent error handling across CLI, TUI, and agents.

### Example: thread start (command)

```json
// request
{"jsonrpc":"2.0","id":1,"method":"thread.start","params":{"slug":"fix-login","priority":"PRIO","description":"Fix login bug"}}

// response
{"jsonrpc":"2.0","id":1,"result":{"hash":"a3f1b2c4e5d6...","short_hash":"a3f1b2c","slug":"fix-login","priority":"PRIO","description":"Fix login bug","state":"active","dir":"tsk/threads/a3f1b2c-fix-login"}}

// error
{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"slug already exists"}}
```

### Example: list threads (query)

```json
// request
{"jsonrpc":"2.0","id":2,"method":"thread.list","params":{}}

// response
{"jsonrpc":"2.0","id":2,"result":{"threads":[{"hash":"a3f1b2c4...","short_hash":"a3f1b2c","slug":"fix-login","state":"active","priority":"PRIO"},{"hash":"e802b0a1...","short_hash":"e802b0a","slug":"update-deps","state":"active","priority":"BG"}]}}
```

### Priority abbreviations

| Abbreviation | Full name   |
|-------------|-------------|
| `BG`        | background  |
| `PRIO`      | priority    |
| `INC`       | incident    |

### Example: switch thread (command — future delta)

```json
// request
{"jsonrpc":"2.0","id":3,"method":"thread.switch_to","params":{"hash":"a3f1b2c"}}

// response
{"jsonrpc":"2.0","id":3,"result":{"active_thread":"a3f1b2c","paused_threads":["e802b0a"]}}
```

## Daemon Async Architecture

The daemon uses a dedicated listener thread for the socket, separate from the event processing loop.

```
// Dedicated thread for socket listening
spawn_thread {
    loop {
        connection = socket.accept()
        bytes = connection.read_until('\n')
        request = json_rpc_parse(bytes)
        internal_channel.send((request, connection))
    }
}

// Main processing loop
loop {
    if (request, connection) = internal_channel.try_recv() {
        response = process_request(request)  // command or query
        connection.write(json_rpc_serialize(response) + '\n')
    }
}
```

## Timeouts

The CLI sets a read timeout on the socket connection (e.g. 5 seconds). If the daemon is unresponsive, the CLI gets a clean timeout error rather than hanging indefinitely.

## Storage

All storage is project-scoped under `{project.root}/tsk/` and committed to source control.

```
{project.root}/tsk/
  event-log/
    events.ndjson               # append-only domain events (audit trail)
  threads/
    index.json                  # authoritative thread state
    a3f1b2c-fix-login/          # per-thread context directory
    e802b0a-update-deps/        # per-thread context directory
```

### `index.json` (authoritative state)

- The current state of all threads. Written by the daemon on every state change.
- Committed to git and shared across the team.
- This is the authoritative source, not a cache.

### Event log (audit trail)

- Format: NDJSON (append-only, one JSON object per line).
- Records all domain events (ThreadStarted, etc.) with timestamps.
- Committed to git alongside `index.json`.
- Serves as history and audit trail, not as the primary source of current state.

### Thread directories

- One directory per thread: `{short-hash}-{slug}/` (7-char SHA-256 prefix of slug + slug).
- Stores context files associated with the thread (notes, agent logs, etc.).
- Full hash stored in `index.json`; short hash used for directory names and display.

## Why This Approach

- **Daemon is always available**: CLI and agents work whether or not the TUI is running.
- **Single writer**: the daemon owns the event log. No file locking, no concurrent append issues.
- **CQRS TUI**: the TUI is a pure view layer — sends commands, queries state. Simple to build and reason about.
- **Minimal latency**: Unix socket IPC is kernel-buffered, local only, sub-millisecond.
- **Minimal complexity**: no framework, no message broker. Just serde + UnixStream + JSON-RPC + newline framing.
- **Scales to multiple producers**: Unix sockets handle multiple concurrent client connections natively.
- **Proven pattern**: Docker uses the same architecture (thin CLI → Unix socket → fat daemon).
- **Upgrade path**: JSON-RPC is transport-agnostic. Can move to TCP or WebSocket later without changing the protocol. Existing Rust crates (`jsonrpc-core`, `jsonrpsee`) available if needed.
