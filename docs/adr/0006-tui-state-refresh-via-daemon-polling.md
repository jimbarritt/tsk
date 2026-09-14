# 6. TUI State Refresh via Daemon Polling

Date: 2026-03-09

## Status

Accepted. Supersedes [ADR 0005](0005-tui-state-refresh-via-file-watch.md).

Will be superseded by a future ADR when daemon push is implemented.

## Context

ADR 0005 introduced file system watching (kqueue/inotify via the `notify` crate) to give the
TUI instant updates when state changed. In practice, this approach was unreliable: kqueue on
macOS fires directory-level events for structural changes (file creation, deletion, rename) but
not for content modifications to existing files. When `tsk thread switch-to` updates
`index.json` in place, the directory watch does not fire, and the TUI never refreshes.

Watching the file directly (rather than its parent directory) would fix this specific case, but
introduces new fragility: the file may not exist when the TUI starts, `fs::write` may use
atomic rename on some platforms (creating a new inode and breaking the watch), and kqueue
behaviour varies across macOS versions. The file watch adds a dependency (`notify`) and
platform-specific complexity for a problem that has a simpler solution.

## Decision

Replace the file watch with polling the daemon via `thread.list` over the Unix socket every
500ms.

- The `notify` crate dependency is removed.
- The TUI event loop polls `crossterm::event::poll` with a 100ms timeout for key events.
  Every 500ms (5 iterations without a file-change event), it re-fetches threads from the
  daemon via `thread.list`.
- The daemon remains the single source of truth. The TUI never reads `index.json` directly.

## Alternatives reconsidered

### A. Fix the file watch (watch the file, not the directory)

Would fix the immediate bug but keeps the TUI coupled to the daemon's storage layout and
dependent on platform-specific file system event semantics. Fragile across macOS versions,
atomic writes, and daemon restarts.

### B. TUI reads `index.json` directly

Eliminates the socket round-trip but breaks the client-daemon abstraction. If the storage
format or location changes, the TUI breaks. If the daemon moves to a remote host, the TUI
has no path forward.

### C. Daemon push over persistent socket

The correct long-term architecture. The daemon maintains subscriber connections and broadcasts
state changes immediately. Zero latency, no polling, no file system coupling. Requires
protocol changes (`thread.subscribe` method, persistent connections, broadcast on mutation).

This remains the planned direction — see **Future** section below.

## Consequences

- TUI updates within 500ms of any state change — imperceptible for a status display.
- No platform-specific file watching code. Removes the `notify` dependency.
- The daemon is polled ~2 times per second when the TUI is idle. This is a cheap local Unix
  socket call with a tiny JSON payload — negligible overhead.
- The architecture stays clean: TUI speaks only the JSON-RPC protocol, knows nothing about
  storage layout. This is the right posture for a future move to daemon push or remote daemon.

## Future: Daemon Push

The correct long-term solution is event-driven push from the daemon:

- Add a `thread.subscribe` JSON-RPC method. The daemon keeps the connection open and streams
  `thread.changed` notifications as newline-delimited JSON on every state mutation.
- The TUI opens one persistent connection for the subscription and a separate ephemeral
  connection per command.
- The daemon broadcasts to all subscriber connections on every mutation.

This eliminates polling entirely, gives truly instant updates, and is the right foundation
if `tskd` ever runs remotely. When implemented, this ADR should be marked superseded.
