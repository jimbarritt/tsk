# Getting started

## Usage

Run `tsk` from inside any working tree of a repository whose `origin` holds a ledger
branch, `refs/heads/tsk/ledger`. The `ledger`, `thread` and `events` commands run in the `tsk`
client alone, with no daemon. The default ledger backend is Dolt and needs the `dolt` binary, version 1.88.2 (see
[installation.md](installation.md)). The ledger's tables, files and formats are in
[ledger-layout.md](../domain/ledger-layout.md). Threads, bindings and continuation state
are in [session-continuation-design.md](../domain/session-continuation-design.md).

**1. Fetch the ledger:**

```bash
WT="$(tsk ledger fetch)"
```

It clones the ledger as a Dolt database, the ledger store, outside the repository
(`.../tsk/repos/<clone-id>/ledger-db`) and prints its path. `tsk ledger path` prints the
same path without fetching. Do not open files in it. Read and write documents with
`tsk ledger doc`:

```bash
tsk ledger doc get index.md                      # print the index
tsk ledger doc list missions/                    # list the briefings
tsk ledger doc get missions/operational/M-BOOT-04-official-ledger.md
tsk ledger doc put index.md updated-index.md     # or pipe the body on stdin
tsk ledger doc rm missions/operational/old.md
tsk ledger push "Update the index"               # commit and push the change
```

`doc put` and `doc rm` change the local store only. `tsk ledger push` sends the change.

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

**8. Attach a nexus and list it:**

```bash
tsk nexus add <nexus url>   # the same as tsk config attach-nexus <nexus url>
tsk nexus list              # config path, nexus URL, territories and repos
tsk nexus list --json       # the same as one JSON object
tsk nexus register-repo     # add this repo's entry to nexus.json, commit and push it
```

A repo whose `nexus.json` entry has `"ledger": "nexus"` holds its ledger in the nexus. See
[new-nexus-on-a-clean-machine.md](new-nexus-on-a-clean-machine.md). With no nexus attached,
`tsk nexus list` prints `nexus: none attached` and exits 0. When the nexus cannot be
fetched, it lists the copy held from an earlier fetch, with a note on stderr, or exits 1
when no copy is held.

`tsk nexus register-repo` runs in the managed repo. It takes `--id`, `--territory`,
`--territory-name`, `--ledger nexus|repo` and `--local`, and prints `already registered`
when the entry exists. Step 6 of the clean machine guide describes each option.

**9. Launch the TUI** (no arguments):

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

## Releasing

A push to `main` releases when the workspace version in `Cargo.toml` has no `v<version>`
tag yet. The workflow `.github/workflows/release.yml`:

1. Creates the tag `v<version>` on the pushed commit and the GitHub release
   `tsk <version>`.
2. Writes the notes: `docs/releases/v<version>.md` when that file exists, then the
   subject of every commit since the previous `v*` tag, then the compare link GitHub
   generates.
3. Calls `.github/workflows/publish-crates.yml`, which publishes `tsk-core`, then
   `tsk-bin`, to crates.io.

To release:

1. Bump the workspace version with `just bump <version>` (requires cargo-edit:
   `cargo install cargo-edit`).
2. Optional: write a summary in `docs/releases/v<version>.md`.
3. Commit and push to `main`.

A push that leaves the version unchanged releases nothing.

`publish-crates.yml` also runs when someone publishes a release on GitHub by hand, and
from the Actions tab with a tag. It skips a crate whose version is already on crates.io,
so a rerun publishes only what is missing.

The publish needs the secret `CARGO_REGISTRY_TOKEN`, a crates.io API token with the
publish-update scope, in the GitHub environment `crates-io`. Only the publish job reads
it. The environment's deployment rules admit `main`, for the release workflow and a run
from the Actions tab, and tags matching `v*`, for a release published by hand.

From a terminal, `just publish` publishes both crates by hand.

The published crate name for the CLI is `tsk-bin` (it installs the `tsk` binary).
