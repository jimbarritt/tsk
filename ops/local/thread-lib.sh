# Shared functions for the thread continuation scripts (M-BOOT-02-01).
# Sourced, not executed directly. Callers: thread-*.sh in this directory.
#
# Design reference: docs/domain/session-continuation-design.md on main.

source "$(dirname "${BASH_SOURCE[0]}")/mint-token-lib.sh"

# thread_wt: print the bootstrap worktree path. Resolves the path only; it
# never refreshes, so it cannot discard uncommitted work in the worktree.
# Falls back to fetching only when the worktree does not exist at all.
#
# Always resolves through bootstrap-wt-path.sh, the same canonical path
# push-bootstrap-ref.sh and fetch-bootstrap-ref.sh use, rather than trusting
# $TSK_BOOTSTRAP_WT on its own say-so. An earlier version returned
# $TSK_BOOTSTRAP_WT directly whenever it was set and the directory existed,
# on the assumption that it always names the current clone's own worktree.
# That holds for the SessionStart hook's own clone, but not for a script run
# from a different clone or worktree while the variable is still exported
# from an earlier session: the two resolve to different clone IDs, so a
# thread script can scaffold a thread into one worktree while
# push-bootstrap-ref.sh, which never reads the variable, pushes against a
# different, unrelated (possibly nonexistent) one. Found live, M-BOOT-02
# T-20, while testing against an isolated clone with the main session's
# $TSK_BOOTSTRAP_WT still in its environment.
thread_wt() {
  local wt
  wt="$("$(dirname "${BASH_SOURCE[0]}")/bootstrap-wt-path.sh")"
  if [ -d "$wt" ]; then
    printf '%s\n' "$wt"
  else
    "$(dirname "${BASH_SOURCE[0]}")/fetch-bootstrap-ref.sh"
  fi
}

# thread_refresh_wt: fetch the latest tsk/bootstrap into the worktree and
# print its path. Use before any read that must not be stale (collision
# checks, reading the latest continuation state entry) and before any write.
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

# thread_mint_id: an 8-character lowercase hex slug, checked against threads/
# on the (freshly fetched) bootstrap worktree for collision. Prints the id.
# Caller is responsible for refreshing the worktree first if freshness matters.
#
# Hex, not the base36 the original design named: the slug comes from
# tsk_mint_token, which slices a sha256 digest, so the alphabet is whatever the
# digest is written in. See mint-token-lib.sh for why that replaced reading
# /dev/urandom. The space narrows from 36^8 to 16^8, about 4.3e9, which the
# collision check below makes immaterial: it regenerates on a hit, so a
# duplicate slug is never issued.
thread_mint_id() {
  local wt id attempt
  wt="$(thread_wt)"
  # Bounded, so a fault that makes every id collide surfaces rather than
  # spinning forever, as the original unbounded loop would have.
  for attempt in $(seq 1 100); do
    if ! id="$(tsk_mint_token 8)"; then
      echo "thread_mint_id: minting a thread id failed" >&2
      return 1
    fi
    if [ ! -e "$wt/threads/$id" ]; then
      printf '%s\n' "$id"
      return 0
    fi
  done
  echo "thread_mint_id: no free id after 100 attempts; is $wt/threads intact?" >&2
  return 1
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

# thread_resolve_binding_at <wt>: binding lookup body, given a worktree
# path the caller has already resolved. Prints "cloud:<thread-id>" or
# "worktree:<thread-id>" and exits 0 on a hit, prints nothing and exits 1
# on a miss. Shared by thread_resolve_binding (fetches first) and
# thread_resolve_binding_local (does not), so the lookup logic itself
# has exactly one copy.
thread_resolve_binding_at() {
  local wt="$1" lookup id marker
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

# thread_resolve_binding [<wt>]: resolve the current binding. With no
# argument, fetches a fresh worktree first (the safe default; use this
# before any write that must not act on stale state — start, resume).
# Prints "cloud:<thread-id>" or "worktree:<thread-id>" and exits 0 on a
# hit, prints nothing and exits 1 on a miss.
#
# Pass <wt> only when the caller fetched it in this same invocation, to
# skip a redundant second fetch. An ambient value such as $TSK_BOOTSTRAP_WT
# lives for a whole session and is not proof of freshness; do not pass
# that here on its own say-so.
thread_resolve_binding() {
  local wt="${1:-}"
  if [ -z "$wt" ]; then
    wt="$(thread_refresh_wt)"
  fi
  thread_resolve_binding_at "$wt"
}

# thread_resolve_binding_local: same lookup, without fetching. For a check
# that runs on every turn (the Stop hook binding guard, M-BOOT-02-02) a
# network fetch on every turn is slow and fragile, and unnecessary: a
# binding this session itself wrote is already reflected in its own
# worktree checkout without re-fetching it. Exits 1, same as a miss, if the
# worktree does not exist yet at all.
thread_resolve_binding_local() {
  local wt
  wt="$("$(dirname "${BASH_SOURCE[0]}")/bootstrap-wt-path.sh")"
  [ -d "$wt" ] || return 1
  thread_resolve_binding_at "$wt"
}

# thread_unbound_prompt_text: the instruction shown to the agent when no
# binding is found. Shared by the SessionStart hook (fires once) and the
# Stop hook binding guard (repeats every turn until bound, M-BOOT-02-02),
# so the two call sites cannot drift the way the push sequence once did
# (see CLAUDE.md).
thread_unbound_prompt_text() {
  printf '%s' "No thread binding was found for this session or worktree. Use the AskUserQuestion tool to ask which mission to work: offer your best-inferred candidate (from index.md, the mission tree, or anything already said this session) as one selectable option, one or two other unblocked missions as alternatives, and leave free text open for anything else. Once answered, run /start-thread for it, or /resume-thread <thread-id> if an existing thread is named instead."
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
  # -p keeps the temp file on the same filesystem as $lookup, so the mv below
  # is an atomic rename rather than a copy-plus-unlink across filesystems
  # (mktemp with no -p defaults to $TMPDIR, which is not guaranteed to share
  # a filesystem with $WT).
  tmp="$(mktemp -p "$(dirname "$lookup")")"
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
# the thread's continuation state, one per line. Empty if the thread has no
# continuation state entries yet.
thread_written_by_actors() {
  local thread_id="$1" wt="$2" store
  store="$wt/threads/$thread_id/continuation-state.jsonl"
  [ -f "$store" ] || return 0
  jq -r '.written_by // empty' "$store" | sort -u
}
