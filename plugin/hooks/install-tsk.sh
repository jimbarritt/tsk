#!/usr/bin/env bash
set -uo pipefail

# Installs the tsk binary from the latest GitHub release, with no compile. Does nothing
# when the tsk first on the path is at or above the version in plugin/tsk-version, so a
# development build is never replaced. Otherwise:
#
# - A tsk that is older runs `tsk self-update`. When that cannot run, or leaves the
#   version too low, the release archive replaces that same file, so a stale copy cannot
#   shadow the new one. A symbolic link is followed: the file it names is replaced and
#   the link stays. A tsk under the cargo bin directory skips self-update, which would
#   compile there, and takes the archive. A Homebrew tsk (a file under a Cellar
#   directory) is left to self-update (`brew upgrade`) and is not overwritten.
# - With no tsk, the archive installs to the first of these that applies:
#   1. TSK_INSTALL_DIR, when set.
#   2. /opt/homebrew/bin, on macOS on Apple silicon (uname -s Darwin, uname -m arm64),
#      when the directory exists and is writable. TSK_HOMEBREW_BIN_DIR overrides the
#      directory.
#   3. /usr/local/bin, when writable.
#   4. $HOME/.local/bin.
#   A directory that is not on the PATH is added to the PATH of the session.
#
# The binary is written to a temporary file in the target directory and renamed over
# the target, so a failed or interrupted install leaves the old file whole, and two
# copies of the script running at once cannot corrupt it.
#
# The archive name, the targets and the tag lookup follow cli/src/self_update.rs
# (target_for, archive_url and highest_version). A test in cli/tests/hooks.rs fails when
# the two disagree. TSK_RELEASE_REPO_URL and TSK_RELEASE_DOWNLOAD_URL override the
# repository and the download base as they do for the binary. Runs under bash 3.2. Prints
# a WARNING line on stdout when the install fails, and nothing otherwise. Always exits 0,
# so a caller never stops on it.

PLUGIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUIRED="$(tr -d '[:space:]' <"$PLUGIN_DIR/tsk-version" 2>/dev/null)"
REPO_URL="${TSK_RELEASE_REPO_URL:-https://github.com/jimbarritt/tsk}"
DOWNLOAD_BASE="${TSK_RELEASE_DOWNLOAD_URL:-https://github.com/jimbarritt/tsk/releases/download}"

WORK=""
STAGED=""
cleanup() {
  [ -z "$STAGED" ] || rm -f "$STAGED"
  [ -z "$WORK" ] || rm -rf "$WORK"
}
trap cleanup EXIT

warn() {
  printf 'WARNING: %s\n' "$1"
}

# The X.Y.Z of the first tsk on the path: the first three-part number of `tsk --version`,
# whatever text surrounds it (the binary prints `tsk X.Y.Z`). Prints nothing when there is
# no tsk or it prints no version.
installed_version() {
  command -v tsk >/dev/null 2>&1 &&
    tsk --version 2>/dev/null | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1
}

# True when the X.Y.Z version $1 is at or above $2. The parts compare as numbers, so 0.9.0
# is below 0.15.0 and 0.26.1 is below 0.26.10.
version_ge() {
  local a1 a2 a3 b1 b2 b3
  IFS=. read -r a1 a2 a3 <<<"$1"
  IFS=. read -r b1 b2 b3 <<<"$2"
  [ "$a1" -ne "$b1" ] && { [ "$a1" -gt "$b1" ]; return; }
  [ "$a2" -ne "$b2" ] && { [ "$a2" -gt "$b2" ]; return; }
  [ "$a3" -ge "$b3" ]
}

# True when the tsk first on the path is at or above the required version.
installed_is_current() {
  local installed
  installed="$(installed_version)"
  [ -n "$installed" ] && version_ge "$installed" "$REQUIRED"
}

# The release target for this platform, as target_for in cli/src/self_update.rs names it.
target_for() {
  case "$(uname -s)/$(uname -m)" in
    Darwin/arm64 | Darwin/aarch64) echo "aarch64-apple-darwin" ;;
    Darwin/x86_64) echo "x86_64-apple-darwin" ;;
    Linux/x86_64 | Linux/amd64) echo "x86_64-unknown-linux-gnu" ;;
    Linux/aarch64 | Linux/arm64) echo "aarch64-unknown-linux-gnu" ;;
    *) return 1 ;;
  esac
}

# The highest vX.Y.Z tag of the repository, as highest_version in self_update.rs finds it.
# The numbers compare as numbers. Prints the required version when the lookup fails.
release_version() {
  local latest
  latest="$(GIT_TERMINAL_PROMPT=0 git -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=30 \
    ls-remote --tags --refs "$REPO_URL" 2>/dev/null |
    sed -n 's|.*refs/tags/v\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)$|\1|p' |
    sort -t. -k1,1n -k2,2n -k3,3n | tail -n 1)"
  echo "${latest:-$REQUIRED}"
}

# Downloads the release archive and installs its binary as the file $1.
install_archive_as() {
  local dest="$1" dest_dir target version url
  dest_dir="$(dirname "$dest")"
  if ! target="$(target_for)"; then
    warn "tsk has no release binary for $(uname -s) on $(uname -m). Install it by hand."
    return 1
  fi
  version="$(release_version)"
  url="${DOWNLOAD_BASE%/}/v$version/tsk-$version-$target.tar.gz"
  if ! WORK="$(mktemp -d "${TMPDIR:-/tmp}/tsk-install.XXXXXX")"; then
    WORK=""
    warn "installing tsk failed: no temporary directory."
    return 1
  fi
  if ! curl -fsSL --connect-timeout 20 --max-time 300 --retry 2 "$url" -o "$WORK/archive.tar.gz" 2>"$WORK/curl.log"; then
    warn "downloading tsk $version failed: $(tr '\n' ' ' <"$WORK/curl.log")"
    return 1
  fi
  if ! tar -xzf "$WORK/archive.tar.gz" -C "$WORK" 2>"$WORK/tar.log" || [ ! -f "$WORK/tsk" ]; then
    warn "unpacking tsk $version failed: $(tr '\n' ' ' <"$WORK/tar.log")"
    return 1
  fi
  # The binary goes to a private file in the target directory, then a rename replaces the
  # target. The rename is atomic on one filesystem, so a reader sees the old file or the
  # new one, never a part, and a running binary keeps its old inode.
  if ! STAGED="$(mktemp "$dest_dir/.tsk-new.XXXXXX" 2>"$WORK/install.log")" ||
    ! { install -m 0755 "$WORK/tsk" "$STAGED" && mv -f "$STAGED" "$dest"; } 2>>"$WORK/install.log"; then
    warn "installing tsk into $dest_dir failed: $(tr '\n' ' ' <"$WORK/install.log")"
    return 1
  fi
  STAGED=""
}

# The file a path names after its symbolic links, with an absolute directory. Written
# without readlink -f, which macOS before 12.3 lacks.
resolve() {
  local path="$1" link dir hops=0
  while [ -L "$path" ] && [ "$hops" -lt 40 ]; do
    link="$(readlink "$path")"
    case "$link" in
      /*) path="$link" ;;
      *) path="$(dirname "$path")/$link" ;;
    esac
    hops=$((hops + 1))
  done
  if dir="$(cd "$(dirname "$path")" 2>/dev/null && pwd -P)"; then
    path="$dir/$(basename "$path")"
  fi
  printf '%s\n' "$path"
}

# True when the file $1 is under the cargo bin directory, where self-update would compile.
is_cargo_install() {
  local file="$1" cargo_bin
  for cargo_bin in "${CARGO_HOME:+$CARGO_HOME/bin}" "${HOME:-/nonexistent}/.cargo/bin"; do
    [ -n "$cargo_bin" ] || continue
    case "$file" in "$cargo_bin"/* | "$(resolve "$cargo_bin")"/*) return 0 ;; esac
  done
  return 1
}

# True when the resolved file $1 lives in a Homebrew Cellar. A plain file in
# /opt/homebrew/bin, such as one this script installed, is not a Homebrew install.
is_homebrew_install() {
  case "$1" in */Cellar/*) return 0 ;; esac
  return 1
}

update_existing() {
  local link file
  link="$(command -v tsk)"
  file="$(resolve "$link")"
  if ! is_cargo_install "$link" && ! is_cargo_install "$file"; then
    tsk self-update >/dev/null 2>&1 || true
    installed_is_current && return 0
  fi
  if is_homebrew_install "$file"; then
    warn "tsk $(installed_version) is older than $REQUIRED and \`tsk self-update\` did not upgrade it. Run \`brew upgrade jimbarritt/tap/tsk\`."
    return 1
  fi
  install_archive_as "$file" || return 1
  installed_is_current ||
    { warn "tsk $(installed_version) is older than $REQUIRED after the install. Run \`tsk self-update\`."; return 1; }
}

# The directory for a tsk that is not installed yet.
new_install_dir() {
  local homebrew_bin="${TSK_HOMEBREW_BIN_DIR:-/opt/homebrew/bin}"
  if [ -n "${TSK_INSTALL_DIR:-}" ]; then
    echo "$TSK_INSTALL_DIR"
  elif [ "$(uname -s)" = "Darwin" ] && [ "$(uname -m)" = "arm64" ] &&
    [ -d "$homebrew_bin" ] && [ -w "$homebrew_bin" ]; then
    echo "$homebrew_bin"
  elif [ -w /usr/local/bin ]; then
    echo "/usr/local/bin"
  else
    echo "${HOME:-/nonexistent}/.local/bin"
  fi
}

install_new() {
  local bin_dir
  bin_dir="$(new_install_dir)"
  mkdir -p "$bin_dir" 2>/dev/null
  install_archive_as "$bin_dir/tsk" || return 1
  case ":${PATH:-}:" in
    *":$bin_dir:"*) ;;
    *)
      export PATH="$bin_dir:${PATH:-}"
      if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
        printf 'export PATH="%s:$PATH"\n' "$bin_dir" >>"$CLAUDE_ENV_FILE"
      fi
      ;;
  esac
  installed_is_current ||
    warn "tsk was installed into $bin_dir, but \`tsk --version\` does not report $REQUIRED or later. Check that $bin_dir comes first on the PATH."
}

if ! printf '%s\n' "$REQUIRED" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  warn "plugin/tsk-version is missing or does not hold an X.Y.Z version, so the tsk version to install is unknown."
  exit 0
fi

if ! installed_is_current; then
  if command -v tsk >/dev/null 2>&1; then
    update_existing || true
  else
    install_new || true
  fi
fi
exit 0
