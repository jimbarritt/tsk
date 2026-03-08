# 4. Unified `tsk` Binary for CLI and TUI

Date: 2026-03-08

## Status

Accepted

Supersedes the three-binary structure described in [ADR 0002](0002-client-daemon-cqrs-architecture.md).

## Context

ADR 0002 specified three separate binaries: `tsk` (CLI), `tsk-tui` (TUI), and `tskd` (daemon). During planning for Delta 1, we reconsidered whether two of those binaries — the CLI and the TUI — should be merged into a single `tsk` binary.

The key observation: a user should be able to type `tsk` with no arguments to launch the TUI, and `tsk thread start <slug> ...` to send a command to the daemon. These are two modes of the same tool, not two separate tools.

## Decision

Merge the CLI and TUI into a single `tsk` binary:

- **`tsk` (no arguments)**: launches the TUI (ratatui-based interactive interface).
- **`tsk <command> [args]`**: sends a JSON-RPC command to the daemon, prints the result, and exits.
- **`tskd`**: remains a separate binary (daemon lifecycle is fundamentally different from client usage).

This gives two binaries total (`tsk` and `tskd`), not three.

A separate CLI-only binary without TUI dependencies can be introduced later as an optimisation if binary size or deployment to headless environments becomes a concern. This can be done via Cargo feature flags (`--no-default-features` to exclude TUI).

## Alternatives Considered

### 1. Three separate binaries (previous decision)

`tsk`, `tsk-tui`, `tskd` as described in ADR 0002.

**Pros:**

- Smallest possible CLI binary — no ratatui/crossterm dependencies.
- Clean dependency separation.

**Cons:**

- Two things for the user to remember (`tsk` vs `tsk-tui`).
- Less discoverable — `tsk` alone shows help text instead of a UI.
- Extra packaging and naming complexity.

### 2. Single binary for everything (including daemon)

One `tsk` binary with `tsk daemon` to start the daemon.

**Pros:**

- Simplest possible packaging — one binary.

**Cons:**

- Daemon lifecycle is fundamentally different (long-running, started at login, managed by init system). Conflating it with a user-facing CLI creates confusion.
- Init system integration is cleaner when the daemon is its own binary with its own name.

## Consequences

- The crate produces two binaries: `tsk` and `tskd`.
- `tsk` carries ratatui as a dependency even when used in CLI mode. This is acceptable — ratatui is not heavy, and the binary size difference is negligible.
- If binary size becomes a concern, a feature flag can gate the TUI dependencies.
- Agents interact with the daemon via the socket directly or via `tsk <command>` — they never need TUI code, but carrying it as dead code is harmless.
- The workspace structure uses three crates producing two binaries:

```
core/
  src/lib.rs        # shared: event types, JSON-RPC, socket protocol, serde
cli/
  src/main.rs       # unified CLI + TUI binary (tsk)
  tests/e2e.rs      # end-to-end tests
daemon/
  src/main.rs       # daemon binary (tskd)
```
