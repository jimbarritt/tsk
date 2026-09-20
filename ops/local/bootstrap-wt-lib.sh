# Resolves where the tsk/bootstrap worktree is checked out, and migrates it
# from the historical location if it is still there.
#
# Sourced by fetch-bootstrap-ref.sh and push-bootstrap-ref.sh so the path is
# computed in exactly one place.
#
# Location: ${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/bootstrap
# See docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md.

bootstrap_state_root() {
  printf '%s/tsk\n' "${XDG_STATE_HOME:-$HOME/.local/state}"
}

bootstrap_git_common_dir() {
  git rev-parse --path-format=absolute --git-common-dir
}

# The historical checkout location, inside .git/. Kept only so an existing
# clone can be migrated off it.
bootstrap_legacy_wt() {
  printf '%s/tsk/bootstrap-ref-wt\n' "$(bootstrap_git_common_dir)"
}

# A stable per-clone key. Minted once and stored in the clone's own git
# metadata, so it survives the clone directory being renamed or moved,
# which a hash of the path would not.
bootstrap_clone_id() {
  local common_dir marker id name suffix
  common_dir="$(bootstrap_git_common_dir)"
  marker="$common_dir/tsk-clone-id"

  if [ -s "$marker" ]; then
    tr -d '[:space:]' < "$marker"
    printf '\n'
    return 0
  fi

  name="$(basename "$(dirname "$common_dir")" | tr -cd 'A-Za-z0-9._-')"
  [ -n "$name" ] || name="repo"
  # tr is killed by SIGPIPE once head has its bytes; expected, not a failure.
  # 2>/dev/null silences the resulting "write error: Broken pipe" noise.
  suffix="$(set +o pipefail; LC_ALL=C tr -dc 'a-z0-9' < /dev/urandom 2>/dev/null | head -c 8)"
  id="$name-$suffix"

  printf '%s\n' "$id" > "$marker"
  printf '%s\n' "$id"
}

bootstrap_wt_path() {
  printf '%s/repos/%s/bootstrap\n' "$(bootstrap_state_root)" "$(bootstrap_clone_id)"
}

# Commits the worktree holds that origin's tip does not, newest first. Empty
# when the worktree is level with origin or behind it.
#
# A commit made without a push leaves a clean worktree, so a status check
# cannot see it, and reset --hard would orphan it. A worker restart produces
# exactly that state by ending a turn between the commit and the push.
bootstrap_wt_pending_commits() {
  local wt="$1" origin_sha="$2"
  git -C "$wt" log --oneline "$origin_sha..HEAD" 2>/dev/null
}

# Move a clone off the historical in-.git/ checkout. Refuses rather than
# destroying anything if that worktree holds uncommitted work.
bootstrap_migrate_legacy_wt() {
  local legacy
  legacy="$(bootstrap_legacy_wt)"
  [ -d "$legacy" ] || return 0

  if [ -n "$(git -C "$legacy" status --porcelain 2>/dev/null)" ]; then
    echo "error: the old bootstrap worktree at $legacy holds uncommitted changes." >&2
    echo "       Push them with ops/local/push-bootstrap-ref.sh first, then re-run." >&2
    return 1
  fi

  git worktree remove --force "$legacy" >/dev/null 2>&1 || rm -rf "$legacy"
  git worktree prune >/dev/null 2>&1 || true
  rmdir "$(dirname "$legacy")" >/dev/null 2>&1 || true
  echo "note: moved the bootstrap worktree out of .git/ (was $legacy)" >&2
}
