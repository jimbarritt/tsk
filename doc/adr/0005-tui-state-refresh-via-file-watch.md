# 5. TUI State Refresh via File Watch (with planned move to Daemon Push)

Date: 2026-03-08

## Status

Accepted. Superseded by ADR 0006 when daemon push is implemented.

## Context

The TUI needs to reflect state changes made via the CLI (e.g. `tsk thread start`, `tsk thread switch-to`) immediately, without the user having to take any action.

The initial implementation polled the daemon via Unix socket every 2 seconds. This means a CLI command causes a visible lag before the TUI updates — unacceptable UX for an interactive tool.

Three options were considered:

1. **Shorter polling interval** — reduce the socket poll from 2s to e.g. 100ms. Simple, but still not instant, wastes resources when nothing is changing.

2. **Watch `index.json` for changes** — use OS-native file system events (kqueue on macOS, inotify on Linux) to trigger a re-fetch the moment the daemon writes state. Instant, zero polling overhead, no protocol changes.

3. **Daemon push over a persistent socket** — the daemon maintains subscriber connections and broadcasts state changes to all listeners the moment they occur. Truly event-driven, no file I/O in the hot path, works over the network if needed in future. Requires protocol changes.

## Decision

Implement option 2 (file watch on `index.json`) using the `notify` crate.

- The TUI watches the `tsk/` directory (non-recursively) using `recommended_watcher`, which resolves to `kqueue` on macOS and `inotify` on Linux.
- On any file system event, the TUI drains the channel and re-fetches threads from the daemon via the existing `thread.list` socket call.
- A 100ms key-event poll replaces the 2s poll, keeping the UI responsive to keypresses.
- The `notify` watcher watches the parent directory rather than `index.json` directly, so it works even before the file is first created.

## Consequences

- Updates appear immediately after any CLI command — no perceptible lag.
- Adds the `notify` crate as a dependency (kqueue/inotify — no daemon threads, low overhead).
- The daemon socket is still used for reads (`thread.list`) — no direct file parsing in the TUI.
- State changes made by other processes (or direct file edits) will also trigger a refresh.

## Future: Move to Daemon Push (Option 3)

This decision is a pragmatic stepping stone. The correct long-term architecture is **daemon push**:

- The daemon maintains a set of persistent subscriber connections (one per TUI instance).
- On any state mutation, the daemon immediately broadcasts the new state (or a change event) to all subscribers.
- The TUI blocks on the subscriber socket rather than polling a file or a channel.

This eliminates the file system as an intermediary, works correctly if `index.json` is ever replaced with a different storage backend, and is the right foundation if `tskd` ever runs remotely (e.g. over SSH or a network socket).

When ADR 0006 is written to implement daemon push, this ADR should be marked superseded.

**Sketch of the protocol change required:**

- Add a `thread.subscribe` JSON-RPC method. The daemon keeps the connection open and streams `thread.changed` events as newline-delimited JSON.
- The TUI opens one persistent connection for the subscription and a separate connection per command.
- The daemon broadcasts to all open subscriber connections on every state mutation.
