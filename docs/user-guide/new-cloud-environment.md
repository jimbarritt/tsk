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

Paste this into the environment's setup script field. The same text is in
[`ops/cloud/setup-environment.sh`](../../ops/cloud/setup-environment.sh), and a test fails
when the two differ:

```bash
#!/bin/bash
set -euo pipefail

BIN="${BIN:-/usr/local/bin}"
SETUP_DIR="${SETUP_DIR:-/usr/local/share/tsk-setup}"

case "$(uname -m)" in
  x86_64) TSK_TARGET=x86_64-unknown-linux-gnu; DOLT_ARCH=amd64
          DOLT_SHA256=639b8042d1ea46ebf6b645a002fc787134dbe1dc02888cc51a51d1d3eb557c93 ;;
  aarch64) TSK_TARGET=aarch64-unknown-linux-gnu; DOLT_ARCH=arm64
          DOLT_SHA256=ede33a0ea8fea7c6bd72246f68bf2df7d1334806c802d52b7822f912e3ca0fcf ;;
  *) echo "unsupported machine: $(uname -m)" >&2; exit 1 ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# check FILE EXPECTED NAME: stop when the SHA-256 of FILE is not EXPECTED.
check() {
  local actual
  actual="$(sha256sum "$1" | cut -d ' ' -f 1)"
  if [ "$actual" != "$2" ]; then
    echo "checksum mismatch for $3: expected $2, got $actual" >&2
    exit 1
  fi
}

# install_bin FILE NAME: write NAME into $BIN through a temporary file and a rename, so
# a running copy is not overwritten in place.
install_bin() {
  install -m 0755 "$1" "$BIN/$2.new"
  mv -f "$BIN/$2.new" "$BIN/$2"
}

# tsk: the prebuilt binary of the version in the Homebrew formula. The release publishes no
# checksum file. The Homebrew formula in jimbarritt/homebrew-tap holds the version and the
# SHA-256 of each archive, written by the release workflow a few minutes after it publishes
# a release, so the formula names the newest release that has a checksum. A session start
# installs a newer tsk when the plugin needs one.
formula="$(curl -sSfL https://raw.githubusercontent.com/jimbarritt/homebrew-tap/main/Formula/tsk.rb)"
TSK_VERSION="$(printf '%s\n' "$formula" | sed -n 's/^ *version "\(.*\)"$/\1/p' | head -n 1)"
[ -n "$TSK_VERSION" ] || { echo "no version in the Homebrew formula" >&2; exit 1; }
TSK_SHA256="$(printf '%s\n' "$formula" | awk -v t="$TSK_TARGET.tar.gz" 'index($0, t) { found = 1; next } found && /sha256/ { gsub(/[" ]/, "", $2); print $2; exit }')"
[ -n "$TSK_SHA256" ] || { echo "no checksum for $TSK_TARGET in the Homebrew formula" >&2; exit 1; }
curl -sSfL "https://github.com/jimbarritt/tsk/releases/download/v$TSK_VERSION/tsk-$TSK_VERSION-$TSK_TARGET.tar.gz" -o "$tmp/tsk.tar.gz"
check "$tmp/tsk.tar.gz" "$TSK_SHA256" "tsk $TSK_VERSION"
tar -xz --no-same-owner -C "$tmp" -f "$tmp/tsk.tar.gz" tsk
install_bin "$tmp/tsk" tsk

# dolt: the ledger store. 2.5.0 is the version plugin/hooks/install-dolt.sh requires.
# dolt publishes no checksum file, so the SHA-256 of each archive is pinned here, taken from
# the download on 2026-10-25. Work out new values when DOLT_VERSION changes.
DOLT_VERSION=2.5.0
curl -sSfL "https://github.com/dolthub/dolt/releases/download/v$DOLT_VERSION/dolt-linux-$DOLT_ARCH.tar.gz" -o "$tmp/dolt.tar.gz"
check "$tmp/dolt.tar.gz" "$DOLT_SHA256" "dolt $DOLT_VERSION"
tar -xz --no-same-owner -C "$tmp" -f "$tmp/dolt.tar.gz"
install_bin "$tmp/dolt-linux-$DOLT_ARCH/bin/dolt" dolt
"$BIN/dolt" config --global --get user.name >/dev/null 2>&1 || "$BIN/dolt" config --global --add user.name tsk
"$BIN/dolt" config --global --get user.email >/dev/null 2>&1 || "$BIN/dolt" config --global --add user.email tsk@localhost

# just: the command runner that the repo's justfile needs. The PyPI package rust-just is a
# prebuilt wheel that holds the just binary. pip checks the wheel against the SHA-256
# pinned here, for x86_64 and aarch64, taken from PyPI on 2026-10-25. Work out new values
# when JUST_VERSION changes: pip download rust-just==VERSION --no-deps --only-binary=:all:
# with the platform for each architecture, then sha256sum the wheel.
JUST_VERSION=1.59.0
JUST_SHA256_X86_64=3ee47f26e3aff917107602d57a08ede20df447f78dc4f9ceb08e57a883c56eb9
JUST_SHA256_AARCH64=a305aa91fc425c44e3bcc7d5faae7b8c1503091d8e79aae1f6801dbdca478e20
printf 'rust-just==%s \\\n    --hash=sha256:%s \\\n    --hash=sha256:%s\n' \
  "$JUST_VERSION" "$JUST_SHA256_X86_64" "$JUST_SHA256_AARCH64" > "$tmp/just-req.txt"
pip download --require-hashes --no-deps --only-binary=:all: -d "$tmp/just-wheel" -r "$tmp/just-req.txt" --quiet
python3 - "$tmp/just-wheel" "$tmp/just" <<'PY'
import glob, sys, zipfile
wheel = glob.glob(sys.argv[1] + "/rust_just-*.whl")[0]
archive = zipfile.ZipFile(wheel)
name = [n for n in archive.namelist() if n.endswith(".data/scripts/just")][0]
open(sys.argv[2], "wb").write(archive.read(name))
PY
install_bin "$tmp/just" just

mkdir -p "$SETUP_DIR"
date -u +%Y-%m-%dT%H:%M:%SZ > "$SETUP_DIR/last-run"
"$BIN/tsk" --version
"$BIN/dolt" version
"$BIN/just" --version
```

The script reads the version in the Homebrew formula and downloads the prebuilt `tsk` of
that version for the machine. It installs `tsk` into `/usr/local/bin`, which is on `PATH`, and installs `dolt`
2.5.0 and `just` 1.59.0 beside it. It sets a Dolt identity when none is set. It needs no
Rust toolchain. Each file is written to a temporary name and renamed, so a running copy is
not overwritten.

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

The plugin's own `SessionStart` hook also installs `dolt`, but a plugin installed during
startup has no hooks in that session (step 4), so the setup script installs it.

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
| A session starts with no tsk context, and `/tsk:` commands are missing | The repo has no `SessionStart` hook yet. | Add it once, see [step 4](#add-the-hook-once-per-repo). |
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
