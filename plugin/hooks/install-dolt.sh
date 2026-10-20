#!/usr/bin/env bash
set -uo pipefail

# Installs the dolt binary that tsk's ledger runs. Does nothing
# when dolt DOLT_VERSION or later is installed, from Homebrew for example.
# Otherwise it installs DOLT_VERSION from the GitHub release. Prints a
# WARNING line on stdout when the install or the identity setup fails, and
# nothing otherwise. Always exits 0, so a caller never stops on it.

DOLT_VERSION="2.5.0"

# Starting dolt costs 60 to 300 ms, and a session start would start it three times (the
# version, the user name, the user email), in each of two hooks. So a check that passed is
# recorded in a stamp file, and later runs trust it while the dolt binary is no newer than
# the stamp. Replacing the binary makes it newer, which checks again.
STAMP="${XDG_STATE_HOME:-${HOME:-/nonexistent}/.local/state}/tsk/dolt-version-ok"

installed_is_current() {
  local installed bin
  bin="$(command -v dolt 2>/dev/null)" || return 1
  if [ -f "$STAMP" ] && [ ! "$bin" -nt "$STAMP" ] &&
    [ "$(cat "$STAMP" 2>/dev/null)" = "$DOLT_VERSION $bin" ]; then
    return 0
  fi
  installed="$(dolt version 2>/dev/null | sed -n 's/^dolt version \([0-9][0-9.]*\).*/\1/p' | head -n 1)"
  if [ -n "$installed" ] &&
    [ "$(printf '%s\n%s\n' "$DOLT_VERSION" "$installed" | sort -V | head -n 1)" = "$DOLT_VERSION" ]; then
    mkdir -p "$(dirname "$STAMP")" 2>/dev/null &&
      printf '%s %s\n' "$DOLT_VERSION" "$bin" >"$STAMP" 2>/dev/null
    return 0
  fi
  return 1
}

# True when dolt's global configuration file names a non-empty value for the key $1
# (user.name, user.email). The file is .dolt/config_global.json under DOLT_ROOT_PATH, or
# under HOME when that is unset. Reading it saves a dolt process. A file that is absent
# says no, and dolt answers.
identity_in_file() {
  local file="${DOLT_ROOT_PATH:-${HOME:-/nonexistent}}/.dolt/config_global.json"
  [ -f "$file" ] && grep -Eq "\"$1\"[[:space:]]*:[[:space:]]*\"[^\"[:space:]][^\"]*\"" "$file"
}

warn() {
  printf 'WARNING: %s\n' "$1"
}

install_dolt() {
  local os arch asset tmp bin_dir
  case "$(uname -s)" in
    Linux) os="linux" ;;
    Darwin) os="darwin" ;;
    *) warn "dolt $DOLT_VERSION has no download for $(uname -s). Install it by hand."; return 1 ;;
  esac
  case "$(uname -m)" in
    x86_64 | amd64) arch="amd64" ;;
    aarch64 | arm64) arch="arm64" ;;
    *) warn "dolt $DOLT_VERSION has no download for $(uname -m). Install it by hand."; return 1 ;;
  esac
  asset="dolt-$os-$arch"

  tmp="$(mktemp -d)"
  if ! curl -fsSL "https://github.com/dolthub/dolt/releases/download/v$DOLT_VERSION/$asset.tar.gz" \
    -o "$tmp/$asset.tar.gz" 2>"$tmp/curl.log"; then
    warn "downloading dolt $DOLT_VERSION failed: $(tr '\n' ' ' <"$tmp/curl.log")"
    rm -rf "$tmp"
    return 1
  fi
  if ! tar -xzf "$tmp/$asset.tar.gz" -C "$tmp" 2>"$tmp/tar.log"; then
    warn "unpacking dolt $DOLT_VERSION failed: $(tr '\n' ' ' <"$tmp/tar.log")"
    rm -rf "$tmp"
    return 1
  fi

  if [ -w /usr/local/bin ]; then
    bin_dir="/usr/local/bin"
  else
    bin_dir="$HOME/.local/bin"
    mkdir -p "$bin_dir"
  fi
  if ! install -m 0755 "$tmp/$asset/bin/dolt" "$bin_dir/dolt" 2>"$tmp/install.log"; then
    warn "installing dolt into $bin_dir failed: $(tr '\n' ' ' <"$tmp/install.log")"
    rm -rf "$tmp"
    return 1
  fi
  rm -rf "$tmp"

  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *)
      export PATH="$bin_dir:$PATH"
      if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
        printf 'export PATH="%s:$PATH"\n' "$bin_dir" >>"$CLAUDE_ENV_FILE"
      fi
      ;;
  esac
}

if ! installed_is_current; then
  install_dolt || true
fi

if command -v dolt >/dev/null 2>&1; then
  if ! identity_in_file user.name &&
    [ -z "$(dolt config --global --get user.name 2>/dev/null)" ]; then
    dolt config --global --add user.name tsk >/dev/null 2>&1 ||
      warn "setting the dolt user.name failed. Run \`dolt config --global --add user.name tsk\`."
  fi
  if ! identity_in_file user.email &&
    [ -z "$(dolt config --global --get user.email 2>/dev/null)" ]; then
    dolt config --global --add user.email tsk@localhost >/dev/null 2>&1 ||
      warn "setting the dolt user.email failed. Run \`dolt config --global --add user.email tsk@localhost\`."
  fi
fi
exit 0
