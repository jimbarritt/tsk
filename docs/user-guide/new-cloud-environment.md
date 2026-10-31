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
| `tsk` and `dolt` on `PATH` | The environment's setup script (step 3). |
| Access to the GitHub release downloads | The environment's network access setting (step 2). |
| The `tsk` and `swe` plugins | The setup script, at user scope (step 3). |
| `$TSK_LEDGER_WT` and the ledger worktree | `tsk thread session-start`, run by the same hook. |

A cloud session does not load plugins installed on your machine, or plugins that the
repo's `.claude/settings.json` enables. A plugin installed from the command line works in a
cloud session. The setup script installs it at user scope (step 3), and a repo's own
`SessionStart` hook can install it at project scope. See
[claude-code-plugin-packaging.md](../kb/claude-code-plugin-packaging.md#where-plugins-load).

## 1. Create the environment

1. Open claude.ai/code and start a new session. Do not type a prompt yet.
2. Select the small cloud icon next to the prompt box. It opens environment selection.
3. Create a new environment from there. No existing environment is needed first.
4. Name it, for example `tsk`.

Once created, the environment is selectable in every later session.

## 2. Set network access

The environment's network access setting has four levels:

| Level | Meaning |
|---|---|
| None | Blocks internet access for maximum security. |
| Trusted (Recommended) | Downloads packages from verified sources. |
| Full | Unrestricted internet access for maximum flexibility. |
| Custom | A list of allowed domains. |

Select Trusted.

The script and the session hooks use these hosts:

- `github.com`, for the release downloads, the plugin marketplace and the ledger.
- `release-assets.githubusercontent.com` and `objects.githubusercontent.com`, which
  serve the release downloads after a redirect from `github.com`.
- `raw.githubusercontent.com`, which serves the Homebrew formula that the script reads for the
  `tsk` checksum.
- `pypi.org` and `files.pythonhosted.org`, which serve the `just` wheel. Whether the Trusted
  level allows them was not checked.

If a host is blocked, use Custom and add it, or use Full.

## 3. Add the setup script

Paste this into the environment's setup script field. It fetches
[`ops/cloud/setup-environment.sh`](../../ops/cloud/setup-environment.sh) from the `main`
branch and runs it, so a change to that file reaches the next environment build with no
edit to the field. The script is not pinned: the downloads inside it are checked against
pinned SHA-256 values, and the script itself is the text on `main` at build time.

```bash
#!/bin/bash
set -euo pipefail
curl -sSfL https://raw.githubusercontent.com/jimbarritt/tsk/main/ops/cloud/setup-environment.sh | bash
```

The fetched script reads the version in the Homebrew formula and downloads the prebuilt `tsk` of
that version for the machine. It installs `tsk` into `/usr/local/bin`, which is on `PATH`, and installs `dolt`
2.5.0 and `just` 1.59.0 beside it. It sets a Dolt identity when none is set. It needs no
Rust toolchain. Each file is written to a temporary name and renamed, so a running copy is
not overwritten.

The script then installs the `tsk` and `swe` plugins at user scope, with `claude plugin
install --scope user`. A user-scope plugin loads in the first process of every session of
the environment, in any repo, so a repo needs no committed `SessionStart` hook and no
regeneration. The plugin's own hook installs `dolt` and `tsk` when they are missing, fetches
the ledger and prints the context. A failed plugin step prints a `WARNING:` line and does
not stop the script. A repo that also installs the plugin at project scope, as this repo
does, has two install records of one plugin. `claude plugin list` shows both, and the
plugin loads once. When the two versions differ, the Claude Code docs do not say which
loads: keep them equal.

Every download is checked against a SHA-256 before it is installed:

- **`tsk`:** the release publishes no checksum file. The Homebrew formula in
  `jimbarritt/homebrew-tap` has the SHA-256 of each archive, written by the release
  workflow, and the script reads the version and the value from there. The formula is one
  release behind for a few minutes after a release, so the script installs that version.
  A session start installs a newer `tsk` when the plugin needs one.
- **`dolt`:** it publishes no checksum file. The SHA-256 of the 2.5.0 archive for each
  architecture is pinned in the script.
- **`just`:** the PyPI package `rust-just` is a prebuilt wheel that contains the `just`
  binary. `pip download --require-hashes` checks the wheel against the pinned SHA-256 of
  each architecture. This takes under a second. Building `just` with `cargo install` takes
  about two minutes.

The pinned values come from the downloads of 2026-10-25. They protect against a corrupt
download and a later change to a file. They do not protect against a bad first download.
Work out new values when you change `DOLT_VERSION` or `JUST_VERSION`.

Run on 2026-10-25 in a cloud container with `BIN` and `SETUP_DIR` pointed at a temporary
directory, the script took about 6 seconds and printed `tsk 0.32.2`, `dolt version 2.5.0`
and `just 1.59.0`. Only the x86_64 path was run.

The plugin's own `SessionStart` hook also installs `dolt`. The setup script installs it too,
so `dolt` is present before the first session.

Setup scripts and sessions both run as root, observed on 2026-10-08, so the binaries
are readable in the session.

The `last-run` lines write the run time to `/usr/local/share/tsk-setup/last-run`. They
support the cache test below and can be removed afterwards.

The stamp shows reuse only when the cache keeps the container's filesystem.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `tsk: command not found` or `dolt is not installed` | The setup script failed. | Run the setup script's commands in the session and read the error. |
| A download in the setup script fails | A GitHub download host is blocked by the network access level. | Add the hosts in step 2 under Custom, or use Full. |
| The script stops with `checksum mismatch for <name>` | A download is corrupt or incomplete, or a pinned value is out of date. | Run the script again. If it repeats, check that the version in the script is the one whose checksum is pinned. |
| The script stops at the `just` step with a pip error | `pip` or `python3` is missing, or `pypi.org` and `files.pythonhosted.org` are blocked. | Allow those hosts in step 2, or install `just` another way. |
| The `SessionStart` context reports `WARNING: adding the ... marketplace failed` | `github.com` is blocked, or the marketplace repo is not readable. | Allow `github.com`. Confirm `jimbarritt/claude-plugins` is readable from the session. |
| A session starts with no tsk context, and `/tsk:` commands are missing | The setup script did not install the plugins, or the environment still holds an older script. | Run `claude plugin list` in the session. Run `tsk install-plugin claude-cli` in the repo to add a project-scope install and the repo hook. |
| `$TSK_LEDGER_WT` is empty | The hook did not run, or `tsk thread session-start` failed. | Run `WT="${TSK_LEDGER_WT:-$(tsk ledger fetch)}"` and read any error. |
| `tsk ledger push` stops with `origin refused the push to refs/tsk/...` | The cloud sandbox proxy refuses writes outside `refs/heads/*`, and the ledger is on a custom ref. | Run `tsk config ledger-refs branches` and push again. See [ADR 0008](../adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md) and [ADR 0016](../adr/0016-a-custom-ref-is-the-default-for-a-new-ledger.md). |
| `git merge-base` reports unrelated branches | The clone is shallow. | `git fetch --unshallow origin`. The hook does this at session start. |
| A new `tsk` release is not picked up | The setup script result is cached for the environment. | Change the script, for example a comment line, so the environment rebuilds. |
| The context warns that `tsk` could not be upgraded | `github.com` is blocked, or the release asset is missing for the platform. | Allow `github.com`, then run `tsk self-update` in the session and read the error. |

## Observed

On 2026-10-08, with the earlier script that built `tsk` with `cargo install` (the hooks now install the release binary with `plugin/hooks/install-tsk.sh`):

- The setup script and the session both run as root, with `HOME=/root`.
- A new session reuses the setup result. A session whose container booted at
  05:53:48Z read a `last-run` stamp of 05:48:41Z, written before the boot. The script did
  not run at session start.
- Two sessions read stamps two seconds apart, 05:48:39Z and 05:48:41Z. The cause is not
  established.
- `/clear` keeps the same container and does not test the cache.

## Not verified

- Whether the Trusted level allows the GitHub release download hosts. The run on
  2026-10-13 was in a session with a wider network policy.
- How long the setup result is kept, and whether editing the script invalidates it.
