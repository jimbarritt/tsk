#!/usr/bin/env bash
set -uo pipefail

# Installs the dolt binary that tsk's default ledger backend runs, at the pinned
# version. Does nothing when that version is already installed. Prints a
# WARNING line on stdout when the install or the identity setup fails, and
# nothing otherwise. Always exits 0, so a caller never stops on it.

DOLT_VERSION="1.88.2"

installed_is_current() {
  command -v dolt >/dev/null 2>&1 &&
    dolt version 2>/dev/null | grep -q "$DOLT_VERSION"
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
  if [ -z "$(dolt config --global --get user.name 2>/dev/null)" ]; then
    dolt config --global --add user.name tsk >/dev/null 2>&1 ||
      warn "setting the dolt user.name failed. Run \`dolt config --global --add user.name tsk\`."
  fi
  if [ -z "$(dolt config --global --get user.email 2>/dev/null)" ]; then
    dolt config --global --add user.email tsk@localhost >/dev/null 2>&1 ||
      warn "setting the dolt user.email failed. Run \`dolt config --global --add user.email tsk@localhost\`."
  fi
fi
exit 0
