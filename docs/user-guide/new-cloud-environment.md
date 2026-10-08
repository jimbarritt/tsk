# A new Claude cloud environment with tsk installed

These steps create a Claude Code cloud environment on claude.ai/code in which every new
session finds the `tsk` binary on `PATH`, the `tsk` plugin loaded, and the ledger
fetched.

An environment is created by a person, through the web or the Desktop app. No session or
API call creates one. See
[session-creation-and-environments.md](../kb/session-creation-and-environments.md#constraints).

## What a session needs

| Item | Provided by |
|---|---|
| The repo's code | The cloud session clones it from GitHub. |
| `tsk` on `PATH` | The environment's setup script (step 3). |
| Access to crates.io for the build | The environment's network access setting (step 2). |
| The `tsk` plugin | The repo's `SessionStart` hook (step 4). |
| `$TSK_LEDGER_WT` and the ledger worktree | `tsk thread session-start`, run by the same hook. |

A cloud session does not load plugins installed on your machine, or plugins that the
repo's `.claude/settings.json` enables. The `SessionStart` hook installs the plugin from
the command line, which works in a cloud session. See
[claude-code-plugin-packaging.md](../kb/claude-code-plugin-packaging.md#where-plugins-load).

## 1. Create the environment

1. Open claude.ai/code and start a new session. Do not type a prompt yet.
2. Select the small cloud icon next to the prompt box. It opens environment selection.
3. Create a new environment from there. No existing environment is needed first.
4. Name it, for example `tsk`.

Once created, the environment is selectable in every later session.

## 2. Set network access

The setup script downloads crates from crates.io and, when Rust is missing, the Rust
toolchain from rust-lang.org. Set the environment's network access to a level that
allows these hosts:

- `crates.io`
- `index.crates.io`
- `static.crates.io`
- `static.rust-lang.org`
- `github.com`

The default restricted level allows the package registries in common use. Check that
the crates.io hosts are in its list. If they are not, use full network access or add the
hosts as custom entries.

## 3. Add the setup script

Paste this into the environment's setup script field:

```bash
#!/bin/bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  export RUSTUP_HOME=/usr/local/rustup
  export CARGO_HOME=/usr/local/cargo
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path
  chmod -R a+rX "$RUSTUP_HOME" "$CARGO_HOME"
  for tool in cargo rustc rustup; do
    printf '#!/bin/sh\nRUSTUP_HOME=%s exec %s/bin/%s "$@"\n' "$RUSTUP_HOME" "$CARGO_HOME" "$tool" > "/usr/local/bin/$tool"
    chmod 755 "/usr/local/bin/$tool"
  done
fi

cargo install tsk-bin --locked --root /usr/local

mkdir -p /usr/local/share/tsk-setup
{
  date -u +%Y-%m-%dT%H:%M:%SZ
  echo "user=$(whoami) home=$HOME"
} > /usr/local/share/tsk-setup/last-run
tsk --version
```

When `cargo` is missing, the script installs the Rust toolchain system-wide under
`/usr/local` and writes wrappers for `cargo`, `rustc` and `rustup` into
`/usr/local/bin`. The wrappers set only `RUSTUP_HOME`, so the toolchain is shared and
read-only, and each user's `CARGO_HOME` stays at its own `~/.cargo`. When `cargo` is
already present, that block is skipped.

`--root /usr/local` writes the binary to `/usr/local/bin/tsk`, which is on `PATH` for
every user in the container, whichever user the setup script and the session run as.

The last lines write the run time and the user to
`/usr/local/share/tsk-setup/last-run`. They support the cache test below and can be
removed afterwards.

To pin a version, add `--version <version>` to the `cargo install` line. Without it,
`cargo install` takes the latest published `tsk-bin`.

The build compiles from source. The first session in a new environment waits for it.

## 4. Make the plugin install on session start

For the tsk repo, nothing more is needed. `.claude/settings.json` runs
`ops/local/claude-session-start.sh` on `SessionStart`. The script:

1. Unshallows the clone.
2. Adds the `jimbarritt/claude-plugins` marketplace, installs and updates the `swe` and
   `tsk` plugins at project scope.
3. Builds `tsk` from `cli/` if the installed binary is not at the workspace version.
4. Runs `tsk thread session-start`, which fetches the ledger, exports `$TSK_LEDGER_WT`
   and prints the thread binding prompt.

For another repo, add a `SessionStart` hook that runs the same plugin steps and the same
session start command.

`.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$CLAUDE_PROJECT_DIR/.claude/hooks/session-start.sh"
          }
        ]
      }
    ]
  }
}
```

`.claude/hooks/session-start.sh`, marked executable:

```bash
#!/usr/bin/env bash
set -uo pipefail

claude plugin marketplace add jimbarritt/claude-plugins >&2 || true
claude plugin install tsk@jimbarritt-claude-plugins --scope project -y >&2 || true
claude plugin marketplace update jimbarritt-claude-plugins >&2 || true
claude plugin update tsk@jimbarritt-claude-plugins --scope project >&2 || true

INPUT="$(cat)"
OUTPUT="$(printf '%s' "$INPUT" | tsk thread session-start)"
printf '%s' "$OUTPUT"
```

The script runs the session start command itself because Claude Code reads plugin hooks
once, when its process starts. A plugin installed during startup has no hooks in that
process. The binary claims each event by session ID and source, so when the plugin's own
hook also runs, the second run exits with no output.

A repo that keeps its ledger in a nexus needs the nexus attached in the environment.
Set the nexus URL in `~/.config/tsk/config.toml` from the setup script or the hook, with
`tsk config attach-nexus <nexus url>`. See
[new-nexus-on-a-clean-machine.md](new-nexus-on-a-clean-machine.md).

## 5. Check the environment

Start a session in the new environment, selecting your repo. In the session, run:

```bash
tsk --version
echo "$TSK_LEDGER_WT"
claude plugin list
```

Expected:

- `tsk --version` prints the installed version.
- `$TSK_LEDGER_WT` names a directory that exists.
- `claude plugin list` shows `tsk@jimbarritt-claude-plugins` enabled.

The session's first context message names the ledger worktree path and the thread
binding, if one exists.

## Test whether the setup result is cached

`/clear` does not test this. It starts a new conversation in the same Claude Code
process and the same container, so the setup script does not run again either way.
Compare two separate sessions:

1. Save the environment with the stamp lines from step 3.
2. Start session A in the environment. Run `cat /usr/local/share/tsk-setup/last-run`,
   `date -u` and `whoami`. Note the output and how long the session took to become
   usable.
3. End session A. Start session B a few minutes later in the same environment and run
   the same commands.
4. The same `last-run` time in A and B means the setup result was reused. A later time
   in B means the script ran again.
5. Repeat after an hour or more, and after a day, to find whether the result expires.
6. Edit the script, for example by adding a comment line, and start session C. A
   `last-run` time later than B's confirms that an edit invalidates the result.

The stamp shows reuse only when the cache keeps the container's filesystem. The time the
session takes to start is a second signal: a full `cargo install` takes minutes.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `tsk: command not found` | The setup script failed before the `cargo install` line. | Run the setup script's commands in the session and read the error. |
| `cargo install` fails to resolve or download | crates.io hosts are blocked by the network access level. | Add the hosts in step 2, or raise the level. |
| The `SessionStart` context reports `WARNING: adding the ... marketplace failed` | `github.com` is blocked, or the marketplace repo is not readable. | Allow `github.com`. Confirm `jimbarritt/claude-plugins` is readable from the session. |
| `$TSK_LEDGER_WT` is empty | The hook did not run, or `tsk thread session-start` failed. | Run `WT="${TSK_LEDGER_WT:-$(tsk ledger fetch)}"` and read any error. |
| `tsk ledger push` is rejected | The cloud sandbox proxy refuses writes outside `refs/heads/*`. | The ledger is the branch `tsk/ledger`, not a custom ref. See [ADR 0008](../adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md). |
| `git merge-base` reports unrelated branches | The clone is shallow. | `git fetch --unshallow origin`. The hook does this at session start. |
| A new `tsk` release is not picked up | The setup script result is cached for the environment. | Edit the script, for example by pinning `--version`, so the environment rebuilds. |

## Not verified

- The exact labels of the network access levels, and whether the default level includes
  the crates.io hosts. Check the environment's settings screen.
- Whether the setup script result is cached across sessions, and for how long.
- That `cargo` is present in the default cloud image. The script installs it when missing.
