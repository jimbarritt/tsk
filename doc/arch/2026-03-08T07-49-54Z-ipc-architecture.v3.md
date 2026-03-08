2026-03-08T07-49-54Z-ipc-architecture.v3.md
project: tsk

# IPC Architecture

Supersedes: 2026-03-07T21-41-38Z-ipc-architecture.v2.md

## Overview

tsk uses a client-daemon architecture over a Unix domain socket with JSON-RPC 2.0 as the message protocol. The daemon process (`tskd`) is the single authority — it owns the event log, manages state, and serves requests. The CLI and TUI are both clients of the daemon. The pattern follows Docker's architecture: stateless CLI, fat daemon, Unix socket between them.

## Processes

### `tskd` (daemon)

- A headless background process. Can be started on login via systemd/launchd.
- Listens on a Unix domain socket (e.g. `$XDG_RUNTIME_DIR/tsk.sock` or `/tmp/tsk.sock`).
- Accepts connections from multiple clients concurrently.
- Processes commands, applies state transitions, appends to the NDJSON event log.
- Serves queries — derives current state from the event log (with SQLite cache for performance).
- Is the single writer to the event log — no concurrency issues.

### `tsk` (CLI)

- A short-lived process. Runs a command like `tsk thread start <slug> <priority> <description>`.
- Connects to daemon socket, sends a JSON-RPC request, reads the response, exits.
- Blazing fast — the overhead is negligible compared to the work the agent or human will do next.
- On `thread start`, prints the created thread directory path to stdout so the calling agent can capture it.

### `tsk-tui` (TUI)

- A persistent process running in a tmux pane.
- Also a client of the daemon — connects to the same socket.
- Sends commands and queries state via JSON-RPC. Pure CQRS: fire commands, then re-query state to render.
- The TUI is optional. If it's not running, the CLI and agents still work because the daemon is always there.

### Agents (future clients)

- Same protocol as CLI — connect to socket, send JSON-RPC request, read response, disconnect.
- Multiple agents can send commands concurrently. The daemon serialises them into the log.

## Crate Structure

A single Cargo project (`tsk-bin`) producing multiple binaries:

```
src/
  lib.rs          # shared: event types, JSON-RPC message types, socket protocol, serde
  bin/
    tsk.rs        # CLI binary
    tskd.rs       # daemon binary
    tsk-tui.rs    # TUI binary
```

One crate, three binaries. All share the library code. Published to crates.io as `tsk-bin`.

## Protocol: JSON-RPC 2.0

- Transport: Unix domain stream socket (bidirectional).
- Framing: NDJSON (one JSON-RPC message per line, newline delimited).
- Every request has an `id` field; the response echoes it back. This allows pipelining multiple requests on a single connection.
- Standard error format with codes for consistent error handling across CLI, TUI, and agents.

### Example: thread start (command)

```json
// request
{"jsonrpc":"2.0","id":1,"method":"thread.start","params":{"slug":"fix-login","priority":"normal","description":"Fix login bug"}}

// response
{"jsonrpc":"2.0","id":1,"result":{"id":3,"dir":"/home/jim/.tsk/threads/fix-login"}}

// error
{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"slug already exists"}}
```

### Example: list threads (query)

```json
// request
{"jsonrpc":"2.0","id":2,"method":"thread.list","params":{}}

// response
{"jsonrpc":"2.0","id":2,"result":{"threads":[{"id":1,"slug":"fix-login","state":"active","priority":"normal"},{"id":2,"slug":"update-deps","state":"paused","priority":"background"}]}}
```

### Example: switch thread (command)

```json
// request
{"jsonrpc":"2.0","id":3,"method":"thread.switch_to","params":{"id":1}}

// response
{"jsonrpc":"2.0","id":3,"result":{"active_thread":1,"paused_threads":[2,3]}}
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

## Event Log

- Format: NDJSON (append-only, one JSON object per line).
- Location: within the tsk project directory, git-committed.
- Source of truth for all state. The daemon derives current state from replay of this log.
- Only the daemon process writes to the log — single writer guarantee.
- SQLite cache (gitignored) for query performance, rebuilt from log replay.

## Why This Approach

- **Daemon is always available**: CLI and agents work whether or not the TUI is running.
- **Single writer**: the daemon owns the event log. No file locking, no concurrent append issues.
- **CQRS TUI**: the TUI is a pure view layer — sends commands, queries state. Simple to build and reason about.
- **Minimal latency**: Unix socket IPC is kernel-buffered, local only, sub-millisecond.
- **Minimal complexity**: no framework, no message broker. Just serde + UnixStream + JSON-RPC + newline framing.
- **Scales to multiple producers**: Unix sockets handle multiple concurrent client connections natively.
- **Proven pattern**: Docker uses the same architecture (thin CLI → Unix socket → fat daemon).
- **Upgrade path**: JSON-RPC is transport-agnostic. Can move to TCP or WebSocket later without changing the protocol. Existing Rust crates (`jsonrpc-core`, `jsonrpsee`) available if needed.
