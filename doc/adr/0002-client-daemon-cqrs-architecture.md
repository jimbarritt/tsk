# 2. Client-Daemon Architecture with CQRS

Date: 2026-03-08

## Status

Accepted

## Context

tsk needs a runtime architecture that allows multiple processes (CLI, TUI, agents) to read and write delivery state. Early designs had the TUI process as the server — it owned the socket, the event log, and the display. This created a problem: if the TUI wasn't running, nothing worked. The CLI and agents depended on the TUI being alive to process commands.

This is unacceptable for a tool that agents will use autonomously. The agent shouldn't need to know or care whether a human has a TUI open.

Additionally, the TUI was becoming complex — it was simultaneously a socket server, an event processor, a log writer, and a rendering engine. These are separate concerns.

## Decision

Split into three processes with a dedicated daemon:

- **`tskd` (daemon)**: headless background process. Owns the Unix socket, the event log, and all state. Processes commands, serves queries. Single writer to the event log. Can be started on login via systemd/launchd.
- **`tsk` (CLI)**: short-lived client. Sends a command, reads a response, exits.
- **`tsk-tui` (TUI)**: persistent client. Sends commands and queries state for rendering. Pure view layer.

All three binaries are built from a single Cargo crate (`tsk-bin`), sharing library code for event types, protocol, and serialisation.

The TUI follows a CQRS (Command Query Responsibility Segregation) pattern:

- **Commands** (e.g. `thread.start`, `thread.switch_to`) are sent to the daemon and modify state.
- **Queries** (e.g. `thread.list`) are sent to the daemon and return current state for rendering.
- The TUI never holds authoritative state. It renders whatever the daemon tells it.

## Alternatives Considered

### 1. TUI as server

The TUI process owns the socket and the event log. CLI and agents connect to the TUI.

**Pros:**

- Simpler — one fewer process. No daemon to manage.
- TUI has direct access to state for rendering.

**Cons:**

- If the TUI isn't running, nothing works. Agents and CLI depend on a human having the TUI open.
- The TUI becomes a monolith: socket server + event processor + log writer + renderer.
- Harder to test — you can't exercise the command/query logic without a terminal.

### 2. CLI writes directly to the event log

No server at all. Each CLI invocation appends directly to the NDJSON log file. The TUI watches the file for changes.

**Pros:**

- Simplest possible architecture. No socket, no daemon, no IPC.
- Each process is fully independent.

**Cons:**

- Loses single-writer guarantee. Concurrent appends from multiple agents risk corruption or interleaving of partial JSON lines.
- No validation at write time — malformed events enter the log.
- No synchronous response. The CLI can't receive a thread ID or directory path back.
- File watching (inotify) introduces latency and platform-specific behaviour.

### 3. Daemon with shared mutable state (no CQRS)

The TUI holds its own copy of state and mutates it directly when it sends commands, rather than re-querying the daemon.

**Pros:**

- Fewer round-trips. The TUI can optimistically update its display.

**Cons:**

- State divergence. The TUI's local state can drift from the daemon's authoritative state if another client (agent, CLI) modifies state concurrently.
- Two sources of truth — exactly the problem tsk is designed to solve for delivery work.

## Consequences

- The daemon must be running for tsk to function. This is managed by starting it on login (systemd user unit or launchd plist). The CLI should give a clear error if the daemon isn't reachable.
- The TUI is optional and disposable. It can be started, stopped, and restarted without affecting state. It re-queries on startup and renders current state.
- The TUI is simpler to build because it has no write-side logic — it just sends commands and renders query results.
- Testing is cleaner: the daemon can be tested without a terminal, the TUI can be tested against a mock daemon.
- The three-binary structure fits in a single Cargo crate using `src/bin/` with shared library code in `src/lib.rs`.
- The daemon is the only process that needs to be robust and correct. CLI and TUI are thin clients.
