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
install --scope user`, and updates each with `claude plugin update --scope user`, because
`install` does not upgrade an installed plugin. A user-scope plugin loads in the first process of every session of
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

A new session starts from a disk that the platform saved after the setup script ran, and
a change to `setup-environment.sh` on `main` does not cause a new build. So both
`SessionStart` scripts run `plugin/hooks/refresh-setup.sh` first. Where `last-run` exists,
it fetches `setup-environment.sh` from `main` and compares its SHA-256 with
`~/.local/state/tsk/setup/script.sha256`. When they differ, it runs the fetched script with
the options in `~/.local/state/tsk/setup/args`, which `setup-environment.sh` writes, then
stores the new SHA-256. It prints one line in the session context, and the text of
`~/.claude/AGENTS.md` when that file exists, because Claude Code can read
`~/.claude/CLAUDE.md` before the hook writes it. A disk saved before the `args` file existed
takes `--use-robot-attribution <email>` from the user tsk config when it sets robot
attribution. When the script has not changed, the check compares the
user-scope version of the `tsk` and `swe` plugins in `~/.claude/plugins/installed_plugins.json`
with the version in each plugin's manifest on `main`, and updates an older one, at project
scope too when the repo has a project-scope install. A plugin release then reaches a new
session with no rebuild. The update loads in the next Claude Code process, and the context
line says so. The check fetches each file by the commit SHA of `main`, which `git
ls-remote` reads from `github.com`, because the CDN keeps a file under `main` for up to 5
minutes. `TSK_SETUP_REFRESH=off` switches the check off. The first session after a build
runs the script once more, because the build stores no SHA-256. A disk saved with a plugin older than 0.3.46 has
no check in the plugin hook, so in a repo other than this one the check starts only after
one rebuild.

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
| A file or setting that `setup-environment.sh` writes is missing, after a change to that file on `main` | Each new session, a regenerated one included, starts from a disk saved by an earlier setup run. Compare `/usr/local/share/tsk-setup/last-run` with the commit time of the change. | Session start runs the script again when it changed: read the set-up line in the session context. A `WARNING:` line there names the failure. Run the environment's setup line by hand in the session to see the full output. |
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

On 2026-11-08, in a session made by `/tsk:regenerate-cloud-instance`:

- Regeneration starts a new machine from the environment's saved disk. The machine booted
  at 21:07:13Z. `/usr/local/share/tsk-setup/last-run` read 12:43:07Z, and
  `~/.config/tsk/config.toml` had the same time. The setup script did not run for the
  session.
- The saved disk holds the repo clone too. `.git/tsk-clone-id` was from 12:43:08Z. Only
  `.git/HEAD` changed at session start (21:07:22Z), when the platform updated the checkout
  to `main`.
- A change to `ops/cloud/setup-environment.sh` on `main` does not reach a session started
  from a disk saved before it. Commit 905e16e (21:06Z) adds `~/.claude/AGENTS.md` and
  `~/.claude/CLAUDE.md`. The new session had neither file. The fetched script on `main`
  held the step, so the URL and its cache were not the cause.
- Running the environment's setup line by hand in the session wrote both files and exited
  0. Claude Code reads `~/.claude/CLAUDE.md` when its process starts, so a file written by
  hand loads in the next process, not the current one.

## Not verified

- Whether the Trusted level allows the GitHub release download hosts. The run on
  2026-10-13 was in a session with a wider network policy.
- How long the setup result is kept, and whether editing the script invalidates it.
