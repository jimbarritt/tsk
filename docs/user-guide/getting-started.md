# Getting started

## Usage

Run `tsk` from inside any working tree of a repository whose `origin` holds a ledger
branch, `refs/heads/tsk/ledger`. The `ledger`, `thread` and `events` commands run in the `tsk`
client alone, with no daemon. The default ledger backend is Dolt and needs the `dolt` binary, version 1.88.2 or later (see
[installation.md](installation.md)). The ledger's tables, files and formats are in
[ledger-layout.md](../domain/ledger-layout.md). Threads, bindings and continuation state
are in [session-continuation-design.md](../domain/session-continuation-design.md).

To bring an existing plan into a ledger, run `tsk guide import`. It prints a guide for an
agent: what the ledger holds, how a plan maps onto records, and the steps to write
missions, tasks, decisions and questions through the `tsk` commands. The session start
context names the command while the ledger holds no missions.

The ledger holds records and documents. Missions, tasks, decisions and questions are
records, and `tsk mission list` is the index of them. Documents are free text, such as
research and reports. Agents do not edit either by hand.

```bash
tsk mission list                     # every mission: id, status, blocked-by, parent, name
tsk mission list M-BOOT-04 --json    # one mission in full
tsk task list M-BOOT-04              # its tasks
tsk decision list M-BOOT-04          # its decisions
tsk question list                    # open questions, all missions
```

The commands that write records are `tsk mission`, `tsk task`, `tsk decision` and
`tsk question`. Each changes the local ledger store only. `tsk ledger status` shows what
is not pushed, `tsk ledger push` publishes it, and `tsk ledger discard --yes` drops it
before the push.

**1. Fetch the ledger:**

```bash
WT="$(tsk ledger fetch)"
```

It clones the ledger as a Dolt database, the ledger store, outside the repository
(`.../tsk/repos/<clone-id>/ledger-db`) and prints its path. `tsk ledger path` prints the
same path without fetching. Do not open files in it.

The ledger has a second branch, `tsk/docs`, that holds the documents as plain Markdown
files. `tsk ledger fetch` checks it out beside the database, at
`.../tsk/repos/<clone-id>/docs`. Read and write documents with `tsk ledger doc`:

```bash
tsk ledger doc list missions/                    # list the documents under a prefix
tsk ledger doc get missions/operational/M-BOOT-04/research.md
tsk ledger doc put missions/operational/M-BOOT-04/research.md notes.md   # or pipe the body on stdin
tsk ledger doc rm missions/operational/old.md
tsk ledger doc path missions/operational/M-BOOT-04/research.md   # the file's absolute path, to open it elsewhere
tsk mission link M-BOOT-04 missions/operational/M-BOOT-04/research.md --role intelligence
tsk ledger push "Add the research"               # commit and push the change
```

`doc put` and `doc rm` change the local worktree only. `tsk ledger push` sends the change.
It pushes the documents branch first and the Dolt data second, so a record never names a
document the remote lacks. If the documents push fails, the Dolt data stays unpushed and
the next `tsk ledger push` tries both again. `doc path` exits 1 when the document does not
exist.

A ledger created before version 0.8.0 keeps its documents in a Dolt table. Run
`tsk ledger migrate-docs` once to move them to `tsk/docs`. Until then every command
except `tsk ledger fetch` stops and says so.

**2. Start a thread for a mission:**

```bash
tsk thread start M-BOOT-04
```

The mission has a record (see `tsk mission list`). With no mission ID the thread has no
mission and no tasks, and `tsk thread set mission <id>` attaches one later. The command mints a thread ID, stores the thread
and its mission on the ledger, binds the current code worktree (or cloud session) to
it, pushes, and prints `started:<thread-id>`. It refuses an unknown mission ID. With a
binding already in place it prints `resume-required:<thread-id>` and exits with status 2.
To work without a thread, run `tsk thread skip`.

**3. Pause a thread before the session ends:**

```bash
tsk thread pause <thread-id> T-05 "Thread commands done; next: T-06"
```

The mission comes from the thread. When the mission has a record, the task must be one of
its tasks. The command appends a continuation state entry, pushes, and prints `paused:<thread-id>`. It
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

**6. Change the ledger:**

Write records with the `tsk mission`, `tsk task`, `tsk decision` and `tsk question`
commands, and documents with `tsk ledger doc put`. Do not edit files in `$WT`. Then push:

```bash
tsk ledger status                       # what is not pushed
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

`tsk` with no arguments prints the help text and exits with status 2. The TUI and the
`tskd` daemon were retired (ADR 0012) and removed in 0.16.0.

## Running tests

```bash
cargo test --workspace
```

## Building from source

```bash
cargo build --workspace --release
```

The binary arrives in `target/release/`: `tsk`.

Or install locally with `just`:

```bash
just build-install   # builds and installs to ~/.cargo/bin
just test            # run all tests
just publish         # publish tsk-bin to crates.io
```

## Releasing

A push to `main` releases when the workspace version in `Cargo.toml` has no `v<version>`
tag yet. The workflow `.github/workflows/release.yml`:

1. Creates the tag `v<version>` on the pushed commit and the GitHub release
   `tsk <version>`.
2. Writes the notes: `docs/releases/v<version>.md` when that file exists, then the
   subject of every commit since the previous `v*` tag, then the compare link GitHub
   generates.
3. Calls `.github/workflows/publish-crates.yml`, which publishes `tsk-bin` to crates.io.
4. Calls `.github/workflows/binaries.yml`, which builds `tsk` for macOS (arm64 and
   x86_64) and Linux (x86_64 and arm64), attaches one archive per platform to the
   release, and writes `Formula/tsk.rb` in `jimbarritt/homebrew-tap`.
   Run it from the Actions tab with a tag to build an earlier release.

To release:

1. Bump the workspace version with `just bump <version>` (requires cargo-edit:
   `cargo install cargo-edit`).
2. Optional: write a summary in `docs/releases/v<version>.md`.
3. Commit and push to `main`.

A push that leaves the version unchanged releases nothing.

`publish-crates.yml` also runs when someone publishes a release on GitHub by hand, and
from the Actions tab with a tag. It skips a crate whose version is already on crates.io,
so a rerun after a failure publishes only what is missing.

The release secrets are in the GitHub environment `release`:

| Secret | Read by | Value |
|---|---|---|
| `CARGO_REGISTRY_TOKEN` | the publish job in `publish-crates.yml` | A crates.io API token with the publish-update scope for `tsk-bin`. |
| `HOMEBREW_TAP_TOKEN` | the formula job in `binaries.yml` | A fine-grained personal access token with read and write access to the contents of `jimbarritt/homebrew-tap`. |

Only a job that names the environment can read its secrets. The environment's deployment
rules admit `main`, for the release workflow and a run from the Actions tab, and tags
matching `v*`, for a release published by hand.

From a terminal, `just publish` publishes both crates by hand.

The published crate name for the CLI is `tsk-bin` (it installs the `tsk` binary).
