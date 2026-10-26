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
