#!/bin/bash
set -euo pipefail

BIN="${BIN:-/usr/local/bin}"
SETUP_DIR="${SETUP_DIR:-/usr/local/share/tsk-setup}"

# Options, passed by the environment's setup script as
#   curl -sSfL <this file's URL> | bash -s -- [--use-robot-attribution <email>] [--nexus <url>]
# --use-robot-attribution <email>: turn on robot commit attribution (M-TSK-ATTRIB) for the
# sessions of this environment, with <email> as the robot's git email. The environment also
# needs the secret TSK_ROBOT_SIGNING_KEY. Without the option, attribution stays off.
# --nexus <url>: attach the nexus at <url> with tsk nexus add, so a repo whose nexus entry
# says "ledger": "nexus" reads and writes its ledger there. It also pre-approves the add_repo
# tool, so the agent attaches the nexus repo to a new session on its first turn. The
# account's GitHub connection needs access to the nexus repo. Without the option, no nexus
# is attached.
# The options go to a file in home, so plugin/hooks/refresh-setup.sh can run this script
# again with them at session start, when this file on main changes.
SETUP_STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/tsk/setup"
mkdir -p "$SETUP_STATE_DIR"
if [ "$#" -gt 0 ]; then printf '%s\n' "$@"; fi > "$SETUP_STATE_DIR/args"

ROBOT_EMAIL=""
NEXUS_URL=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --use-robot-attribution)
      [ -n "${2:-}" ] || { echo "--use-robot-attribution needs an email" >&2; exit 1; }
      ROBOT_EMAIL="$2"
      shift 2
      ;;
    --nexus)
      [ -n "${2:-}" ] || { echo "--nexus needs a URL" >&2; exit 1; }
      NEXUS_URL="$2"
      shift 2
      ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
done

case "$(uname -m)" in
  x86_64) TSK_TARGET=x86_64-unknown-linux-gnu; DOLT_ARCH=amd64
          DOLT_SHA256=639b8042d1ea46ebf6b645a002fc787134dbe1dc02888cc51a51d1d3eb557c93 ;;
  aarch64) TSK_TARGET=aarch64-unknown-linux-gnu; DOLT_ARCH=arm64
          DOLT_SHA256=ede33a0ea8fea7c6bd72246f68bf2df7d1334806c802d52b7822f912e3ca0fcf ;;
  *) echo "unsupported machine: $(uname -m)" >&2; exit 1 ;;
esac

# ssh-keygen: git signs a robot commit with it (M-TSK-ATTRIB). A fresh container has no
# package lists, so apt-get install alone fails: update them first.
command -v ssh-keygen >/dev/null || { apt-get update -qq && apt-get install -y openssh-client; }

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
formula="$(curl -sSfL --retry 3 --retry-delay 2 https://raw.githubusercontent.com/jimbarritt/homebrew-tap/main/Formula/tsk.rb)"
TSK_VERSION="$(printf '%s\n' "$formula" | sed -n 's/^ *version "\(.*\)"$/\1/p' | head -n 1)"
[ -n "$TSK_VERSION" ] || { echo "no version in the Homebrew formula" >&2; exit 1; }
TSK_SHA256="$(printf '%s\n' "$formula" | awk -v t="$TSK_TARGET.tar.gz" 'index($0, t) { found = 1; next } found && /sha256/ { gsub(/[" ]/, "", $2); print $2; exit }')"
[ -n "$TSK_SHA256" ] || { echo "no checksum for $TSK_TARGET in the Homebrew formula" >&2; exit 1; }
curl -sSfL --retry 3 --retry-delay 2 "https://github.com/jimbarritt/tsk/releases/download/v$TSK_VERSION/tsk-$TSK_VERSION-$TSK_TARGET.tar.gz" -o "$tmp/tsk.tar.gz"
check "$tmp/tsk.tar.gz" "$TSK_SHA256" "tsk $TSK_VERSION"
tar -xz --no-same-owner -C "$tmp" -f "$tmp/tsk.tar.gz" tsk
install_bin "$tmp/tsk" tsk

# dolt: the ledger store. 2.5.0 is the version plugin/hooks/install-dolt.sh requires.
# dolt publishes no checksum file, so the SHA-256 of each archive is pinned here, taken from
# the download on 2026-10-25. Work out new values when DOLT_VERSION changes.
DOLT_VERSION=2.5.0
curl -sSfL --retry 3 --retry-delay 2 "https://github.com/dolthub/dolt/releases/download/v$DOLT_VERSION/dolt-linux-$DOLT_ARCH.tar.gz" -o "$tmp/dolt.tar.gz"
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

# The tsk and swe plugins at user scope, so every session of the environment loads them
# from its first process and a repo needs no committed hook. The plugin's own SessionStart
# hook installs dolt and tsk when they are missing, fetches the ledger and prints the
# context. A failure here prints a warning and does not stop the script: the repo's own
# hook or `/tsk:regenerate-cloud-instance` installs the plugins at the next session.
MARKETPLACE_SOURCE=jimbarritt/claude-plugins
MARKETPLACE=jimbarritt-claude-plugins
if command -v claude >/dev/null 2>&1; then
  if claude plugin marketplace add "$MARKETPLACE_SOURCE" >"$tmp/plugin.log" 2>&1; then
    # install does not upgrade an installed plugin, so a run of this script on a saved disk
    # (plugin/hooks/refresh-setup.sh) refreshes the marketplace and updates each plugin too.
    # From plugin 0.3.48 the session start check also updates a plugin older than its
    # release when this script has not changed.
    claude plugin marketplace update "$MARKETPLACE" >"$tmp/plugin.log" 2>&1 ||
      echo "WARNING: refreshing the $MARKETPLACE marketplace failed: $(tr '\n' ' ' <"$tmp/plugin.log")" >&2
    for plugin in tsk swe; do
      claude plugin install "$plugin@$MARKETPLACE" --scope user -y >"$tmp/plugin.log" 2>&1 ||
        echo "WARNING: installing the $plugin plugin failed: $(tr '\n' ' ' <"$tmp/plugin.log")" >&2
      claude plugin update "$plugin@$MARKETPLACE" --scope user >"$tmp/plugin.log" 2>&1 ||
        echo "WARNING: updating the $plugin plugin failed: $(tr '\n' ' ' <"$tmp/plugin.log")" >&2
    done
  else
    echo "WARNING: adding the $MARKETPLACE marketplace failed: $(tr '\n' ' ' <"$tmp/plugin.log")" >&2
  fi
else
  echo "WARNING: claude is not on PATH, so the plugins are not installed" >&2
fi

# Robot attribution, with --use-robot-attribution only. The user config turns it on, and
# tsk thread session-start then writes the signing set-up from TSK_ROBOT_SIGNING_KEY.
# The user-level AGENTS.md tells every session in the environment, in any repo, how to
# treat the platform's Stop hook, which accepts only noreply@anthropic.com as a committer.
# Each option below appends its section to the user-level AGENTS.md, which this script
# writes from empty on every run.
if [ -n "$ROBOT_EMAIL" ] || [ -n "$NEXUS_URL" ]; then
  mkdir -p "$HOME/.claude"
  : > "$HOME/.claude/AGENTS.md"
  grep -qxF '@AGENTS.md' "$HOME/.claude/CLAUDE.md" 2>/dev/null ||
    printf '@AGENTS.md\n' >> "$HOME/.claude/CLAUDE.md"
fi
if [ -n "$ROBOT_EMAIL" ]; then
  "$BIN/tsk" config attribution robot --email "$ROBOT_EMAIL" --user
  cat >> "$HOME/.claude/AGENTS.md" <<'MD'
# Robot commit attribution

This environment signs commits as a robot (tsk, M-TSK-ATTRIB): the committer email is the
robot email, and the robot key signs each commit. GitHub shows these commits as Verified.

The platform's Stop hook, `~/.claude/stop-hook-git-check.sh`, accepts only the committer
email `noreply@anthropic.com`. At the end of a turn it names each unpushed robot commit as
Unverified, and prints commands that reset the author to Claude.

- Ignore those commands. Do not change `user.name` or `user.email`, and do not reset the
  author.
- Push a robot commit in the same turn that makes it. With nothing unpushed, the hook
  prints nothing.
- If the push waits for a person's confirmation, the message repeats at the end of each
  turn until the push. Ignore it.

MD
fi

# The nexus, with --nexus only. tsk nexus add writes the URL to the user config.
# A new cloud session has one repository attached, so its git proxy refuses the nexus and
# session start cannot fetch the ledger. The agent attaches the nexus with the add_repo
# tool on its first turn. Three settings pre-approve that call, so auto mode does not
# refuse it in a session whose first message does not name the nexus:
# - a section of the user-level AGENTS.md, which states the authorisation;
# - a permissions.allow rule for the add_repo tool in the user settings;
# - autoMode entries in the user settings, which name the nexus as a trusted repository.
if [ -n "$NEXUS_URL" ]; then
  "$BIN/tsk" nexus add "$NEXUS_URL"
  cat >> "$HOME/.claude/AGENTS.md" <<MD
# The tsk nexus

This environment keeps tsk ledgers in the nexus repository $NEXUS_URL. The owner of this
environment chose it with the \`--nexus\` option of the setup script, and authorises every
session in this environment to attach it with the \`add_repo\` tool.

On the first turn of a session, before any other work and whatever the first message
asks, run \`tsk ledger status\` and do what its \`Next\` line says, with no confirmation
first. Run it again after each action, until \`Local\` shows a ledger. If \`add_repo\` is
refused, report the refusal and stop. If \`Next\` names \`tsk ledger init\`, run the
\`/tsk:init-ledger\` skill.

Once \`Local\` shows a ledger, run \`tsk thread next\` in the same turn and do what it says:
it asks which robot this session is, then which thread to resume. Do this before you
answer the first message.

MD
  python3 - "$HOME/.claude/settings.json" "$NEXUS_URL" <<'PY'
import json, os, sys
path, url = sys.argv[1], sys.argv[2]
settings = {}
if os.path.exists(path):
    with open(path) as f:
        settings = json.load(f)

def add(items, entry):
    if entry not in items:
        items.append(entry)

add(settings.setdefault("permissions", {}).setdefault("allow", []),
    "mcp__claude-code-remote__add_repo")
auto = settings.setdefault("autoMode", {})
for key, entry in (
    ("environment", "Trusted repo: " + url + " is the tsk nexus of this environment's "
     "owner. It holds the tsk ledgers of the owner's repositories."),
    ("allow", "Attaching the repository " + url + " to the session with the add_repo "
     "tool, cloning it, and fetching from it and pushing to it with tsk commands: the "
     "owner of this environment authorises these in every session."),
):
    items = auto.setdefault(key, ["$defaults"])
    # An entry for another nexus URL is from an earlier run: replace it.
    items[:] = [i for i in items if "tsk nexus" not in i and "add_repo tool" not in i]
    add(items, entry)
tmp = path + ".new"
with open(tmp, "w") as f:
    json.dump(settings, f, indent=2)
    f.write("\n")
os.replace(tmp, path)
PY
fi

mkdir -p "$SETUP_DIR"
date -u +%Y-%m-%dT%H:%M:%SZ > "$SETUP_DIR/last-run"
"$BIN/tsk" --version
"$BIN/dolt" version
"$BIN/just" --version
claude plugin list 2>/dev/null || true
