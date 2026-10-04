# Getting started

## Usage

Run `tsk` from inside any working tree of a repository whose `origin` holds a ledger
branch, `refs/heads/tsk/ledger`. The `ledger`, `thread` and `events` commands run in the `tsk`
client alone, with no daemon. The ledger's files and formats are in
[ledger-layout.md](../domain/ledger-layout.md). Threads, bindings and continuation state
are in [session-continuation-design.md](../domain/session-continuation-design.md).

**1. Fetch the ledger:**

```bash
WT="$(tsk ledger fetch)"
```

It checks out the ledger as a detached ledger worktree outside the repository and prints its
path. `tsk ledger path` prints the same path without fetching.

**2. Start a thread for a mission:**

```bash
tsk thread start M-BOOT-04 missions/operational/M-BOOT-04-official-ledger.md
```

The briefing path is relative to the ledger root. The command mints a thread ID, writes
`threads/<thread-id>/` on the ledger, binds the current code worktree (or cloud session) to
it, pushes, and prints `started:<thread-id>`. With a binding already in place it prints
`resume-required:<thread-id>` and exits with status 2.

**3. Pause a thread before the session ends:**

```bash
tsk thread pause <thread-id> missions/operational/M-BOOT-04-official-ledger.md T-05 "Thread commands done; next: T-06"
```

It appends a continuation state entry, pushes, and prints `paused:<thread-id>`. It
refuses when the repository's `HEAD` is not on origin's default branch (for example
`main`, `master` or `trunk`).

**4. Resume a thread:**

```bash
tsk thread resume <thread-id>
```

It binds the current code worktree or cloud session to the thread and prints
`{"thread_id":"...","latest":{...},"warning":"..."}`. `warning` names the other actors
when someone else wrote to the thread.

**5. Detach, stop and list:**

```bash
tsk thread binding            # cloud:<thread-id> or worktree:<thread-id>; exit 1 with no binding
tsk thread detach             # remove this binding, keep the thread
tsk thread stop [<thread-id>] # delete the thread and every cloud binding to it
tsk thread list               # one JSON object per thread, most recently paused first
```

**6. Change a ledger file:**

Edit the file in `$WT`, then push:

```bash
tsk ledger push "Describe the change"
```

**7. Queue and consume external events:**

```bash
tsk events append github dependabot_alert created owner/repo payload.json   # prints queued
tsk events read-new                                                          # events past the watermark
tsk events advance-watermark <total_count>                                   # prints watermark:<count>
```

`append` wraps the first JSON value in the payload file in an event envelope, appends it
to `external-events/queue.ndjson` and pushes. `read-new` prints
`{"new_count":N,"total_count":M,"events":[...]}` and writes nothing. Advance the watermark
to `total_count` only after every event up to it is processed.

**8. Launch the TUI** (no arguments):

```bash
tskd &
tsk
```

The TUI reads threads from the `tskd` daemon, which keeps its own thread model in
`~/.tsk/`, separate from the ledger. It displays threads grouped by section (Active /
Priority & Incidents / Background). Use `j`/`k` to scroll, `ctrl-d`/`ctrl-u` to page,
`gg`/`G` to jump to top/bottom, `?` for keybindings, `q` to quit.

## Running tests

```bash
# Unit tests only
cargo test -p tsk-core

# All tests, including the tskd e2e tests in daemon/tests/e2e.rs
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
