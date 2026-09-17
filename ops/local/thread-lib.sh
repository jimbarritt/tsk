# Shared functions for the thread continuation scripts (M-BOOT-02-01).
# Sourced, not executed directly. Callers: thread-*.sh in this directory.
#
# Design reference: docs/domain/session-continuation-design.md on main.

# thread_wt: print the bootstrap worktree path, fetching it into existence
# first if the caller hasn't already (e.g. a plain `claude -p` invocation
# with no SessionStart hook run yet).
thread_wt() {
  if [ -n "${TSK_BOOTSTRAP_WT:-}" ] && [ -d "${TSK_BOOTSTRAP_WT}" ]; then
    printf '%s\n' "$TSK_BOOTSTRAP_WT"
  else
    "$(dirname "${BASH_SOURCE[0]}")/fetch-bootstrap-ref.sh"
  fi
}

# thread_refresh_wt: fetch the latest tsk/bootstrap into the worktree and
# print its path. Use before any read that must not be stale (collision
# checks, reading the latest continuation event) and before any write.
thread_refresh_wt() {
  "$(dirname "${BASH_SOURCE[0]}")/fetch-bootstrap-ref.sh"
}

# thread_git_dir: the current repo's own --git-dir, absolute. This is the
# worktree identity used for the CLI binding marker — never
# --show-toplevel, which is unstable across a worktree rename or move.
thread_git_dir() {
  git rev-parse --path-format=absolute --git-dir
}

# thread_worktree_name: the basename of --git-dir. Not permanently unique
# (recycled if the worktree is removed and a new one reuses the same
# basename) — used only inside the written-by URN, never as a binding key.
thread_worktree_name() {
  basename "$(thread_git_dir)"
}

# thread_mint_id: an 8-character lowercase base36 slug, checked against
# threads/ on the (freshly fetched) bootstrap worktree for collision.
# Prints the id. Caller is responsible for refreshing the worktree first
# if freshness matters.
thread_mint_id() {
  local wt id
  wt="$(thread_wt)"
  while :; do
    id="$(LC_ALL=C tr -dc 'a-z0-9' < /dev/urandom | head -c 8)"
    if [ "${#id}" -eq 8 ] && [ ! -e "$wt/threads/$id" ]; then
      printf '%s\n' "$id"
      return 0
    fi
  done
}

# thread_actor_urn: the URN naming the current binding surface, per the
# written-by field format in the design doc.
thread_actor_urn() {
  if [ -n "${CLAUDE_CODE_REMOTE_SESSION_ID:-}" ]; then
    printf 'urn:tsk:cloudsession:%s\n' "$CLAUDE_CODE_REMOTE_SESSION_ID"
  else
    printf 'urn:tsk:worktree:%s\n' "$(thread_worktree_name)"
  fi
}

# thread_cloud_lookup_path: the lookup file's path inside a given worktree.
thread_cloud_lookup_path() {
  local wt="$1"
  printf '%s/threads/lookup-by-cloud-session.json\n' "$wt"
}

# thread_resolve_binding: resolve the current binding. Checks the cloud
# session ID first; falls back to the worktree marker. Prints
# "cloud:<thread-id>" or "worktree:<thread-id>" and exits 0 on a hit,
# prints nothing and exits 1 on a miss.
thread_resolve_binding() {
  local wt lookup id marker
  wt="$(thread_refresh_wt)"
  if [ -n "${CLAUDE_CODE_REMOTE_SESSION_ID:-}" ]; then
    lookup="$(thread_cloud_lookup_path "$wt")"
    if [ -f "$lookup" ]; then
      id="$(jq -r --arg k "$CLAUDE_CODE_REMOTE_SESSION_ID" '.[$k].thread_id // empty' "$lookup")"
      if [ -n "$id" ]; then
        printf 'cloud:%s\n' "$id"
        return 0
      fi
    fi
    return 1
  fi
  marker="$(thread_git_dir)/tsk-thread-id"
  if [ -f "$marker" ]; then
    id="$(tr -d '[:space:]' < "$marker")"
    if [ -n "$id" ]; then
      printf 'worktree:%s\n' "$id"
      return 0
    fi
  fi
  return 1
}

# thread_bind_cloud <thread-id> <wt>: write/update the cloud lookup entry
# in the given worktree. Does not commit or push — caller pushes once,
# after any other writes in the same operation.
thread_bind_cloud() {
  local thread_id="$1" wt="$2" lookup now tmp
  lookup="$(thread_cloud_lookup_path "$wt")"
  mkdir -p "$(dirname "$lookup")"
  [ -f "$lookup" ] || printf '{}' > "$lookup"
  now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  tmp="$(mktemp)"
  jq --arg k "$CLAUDE_CODE_REMOTE_SESSION_ID" \
     --arg id "$thread_id" \
     --arg at "$now" \
     '.[$k] = {thread_id: $id, registered_at: $at}' \
     "$lookup" > "$tmp"
  mv "$tmp" "$lookup"
}

# thread_bind_worktree <thread-id>: write the local marker file. Local to
# this worktree's own git metadata, never pushed.
thread_bind_worktree() {
  local thread_id="$1"
  printf '%s\n' "$thread_id" > "$(thread_git_dir)/tsk-thread-id"
}

# thread_bind_current <thread-id> <wt>: bind whichever surface is current
# (cloud session, or worktree) to the given thread id.
thread_bind_current() {
  local thread_id="$1" wt="$2"
  if [ -n "${CLAUDE_CODE_REMOTE_SESSION_ID:-}" ]; then
    thread_bind_cloud "$thread_id" "$wt"
  else
    thread_bind_worktree "$thread_id"
  fi
}

# thread_written_by_actors <thread-id> <wt>: distinct written-by URNs from
# the thread's continuation log, one per line. Empty if the thread has no
# continuation events yet.
thread_written_by_actors() {
  local thread_id="$1" wt="$2" log
  log="$wt/threads/$thread_id/continuation-state.jsonl"
  [ -f "$log" ] || return 0
  jq -r '.written_by // empty' "$log" | sort -u
}
