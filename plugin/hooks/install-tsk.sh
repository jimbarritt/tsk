#!/usr/bin/env bash
set -uo pipefail

# Installs the tsk binary from the latest GitHub release, with no compile. Does nothing
# when the tsk first on the path is at or above the version in plugin/tsk-version, so a
# development build is never replaced. Otherwise:
#
# - A tsk that is older runs `tsk self-update`. When that cannot run, or leaves the
#   version too low, the release archive replaces that same file, so a stale copy cannot
#   shadow the new one. A tsk under the cargo bin directory skips self-update, which
#   would compile there, and takes the archive. A Homebrew tsk is left to self-update
#   (`brew upgrade`) and is not overwritten.
# - With no tsk, the archive installs to TSK_INSTALL_DIR when set, else /usr/local/bin
#   when writable, else $HOME/.local/bin, which is added to the PATH of the session.
#
# The archive name and the targets follow cli/src/self_update.rs (target_for and
# archive_url), and TSK_RELEASE_REPO_URL and TSK_RELEASE_DOWNLOAD_URL override the
# repository and the download base as they do for the binary. Prints a WARNING line on
# stdout when the install fails, and nothing otherwise. Always exits 0, so a caller
# never stops on it.

PLUGIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUIRED="$(tr -d '[:space:]' <"$PLUGIN_DIR/tsk-version" 2>/dev/null)"
REPO_URL="${TSK_RELEASE_REPO_URL:-https://github.com/jimbarritt/tsk}"
DOWNLOAD_BASE="${TSK_RELEASE_DOWNLOAD_URL:-https://github.com/jimbarritt/tsk/releases/download}"

warn() {
  printf 'WARNING: %s\n' "$1"
}

installed_version() {
  command -v tsk >/dev/null 2>&1 && tsk --version 2>/dev/null | awk '{print $NF}'
}

# True when the tsk first on the path is at or above the required version.
installed_is_current() {
  local installed
  installed="$(installed_version)"
  [ -n "$installed" ] &&
    [ "$(printf '%s\n%s\n' "$REQUIRED" "$installed" | sort -V | head -n 1)" = "$REQUIRED" ]
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
# Prints the required version when the lookup fails.
release_version() {
  local latest
  latest="$(GIT_TERMINAL_PROMPT=0 git ls-remote --tags --refs "$REPO_URL" 2>/dev/null |
    sed -n 's|.*refs/tags/v\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)$|\1|p' |
    sort -V | tail -n 1)"
  echo "${latest:-$REQUIRED}"
}

# Downloads the release archive and installs its binary as the file $1.
install_archive_as() {
  local dest="$1" target version url tmp staged
  if ! target="$(target_for)"; then
    warn "tsk has no release binary for $(uname -s) on $(uname -m). Install it by hand."
    return 1
  fi
  version="$(release_version)"
  url="${DOWNLOAD_BASE%/}/v$version/tsk-$version-$target.tar.gz"
  tmp="$(mktemp -d)"
  if ! curl -fsSL "$url" -o "$tmp/archive.tar.gz" 2>"$tmp/curl.log"; then
    warn "downloading tsk $version failed: $(tr '\n' ' ' <"$tmp/curl.log")"
    rm -rf "$tmp"
    return 1
  fi
  if ! tar -xzf "$tmp/archive.tar.gz" -C "$tmp" 2>"$tmp/tar.log" || [ ! -f "$tmp/tsk" ]; then
    warn "unpacking tsk $version failed: $(tr '\n' ' ' <"$tmp/tar.log")"
    rm -rf "$tmp"
    return 1
  fi
  # The rename is atomic on one filesystem, and a running binary keeps its old inode.
  staged="$(dirname "$dest")/.tsk-new-$$"
  if ! { install -m 0755 "$tmp/tsk" "$staged" && mv -f "$staged" "$dest"; } 2>"$tmp/install.log"; then
    rm -f "$staged"
    warn "installing tsk into $(dirname "$dest") failed: $(tr '\n' ' ' <"$tmp/install.log")"
    rm -rf "$tmp"
    return 1
  fi
  rm -rf "$tmp"
}

resolve() {
  readlink -f "$1" 2>/dev/null || echo "$1"
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

is_homebrew_install() {
  case "$1" in */Cellar/* | */homebrew/* | */Homebrew/*) return 0 ;; esac
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

install_new() {
  local bin_dir
  if [ -n "${TSK_INSTALL_DIR:-}" ]; then
    bin_dir="$TSK_INSTALL_DIR"
  elif [ -w /usr/local/bin ]; then
    bin_dir="/usr/local/bin"
  else
    bin_dir="${HOME:-/nonexistent}/.local/bin"
  fi
  mkdir -p "$bin_dir" 2>/dev/null
  install_archive_as "$bin_dir/tsk" || return 1
  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *)
      export PATH="$bin_dir:$PATH"
      if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
        printf 'export PATH="%s:$PATH"\n' "$bin_dir" >>"$CLAUDE_ENV_FILE"
      fi
      ;;
  esac
  installed_is_current ||
    warn "tsk was installed into $bin_dir, but \`tsk --version\` does not report $REQUIRED or later. Check that $bin_dir comes first on the PATH."
}

if [ -z "$REQUIRED" ]; then
  warn "plugin/tsk-version is missing or empty, so the tsk version to install is unknown."
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
