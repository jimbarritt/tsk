#!/usr/bin/env bash
set -euo pipefail

if [ -z "${GH_AUDIT_TOKEN:-}" ]; then
  read -rsp "Fine-grained token (input hidden): " GH_AUDIT_TOKEN
  echo >&2
fi
GH_TOKEN=$(printf '%s' "$GH_AUDIT_TOKEN" | tr -d '[:space:]')
export GH_TOKEN
unset GH_AUDIT_TOKEN

case "$GH_TOKEN" in
  github_pat_*) ;;
  *)
    printf 'The token does not start with github_pat_ (length %d, starts with "%.4s").\n' "${#GH_TOKEN}" "$GH_TOKEN" >&2
    exit 1
    ;;
esac
printf 'Token length %d, starts with github_pat_\n' "${#GH_TOKEN}" >&2

section() {
  printf '\n== %s ==\n' "$1"
}

list() {
  local out err
  err=$(mktemp)
  if ! out=$(gh api --paginate "$1" --jq "$2" 2>"$err"); then
    printf 'not available: %s\n' "$(grep -m 1 '^gh:' "$err" || head -n 1 "$err")"
    rm -f "$err"
    return 0
  fi
  rm -f "$err"
  if [ -z "$out" ]; then
    printf 'none\n'
  else
    printf '%s\n' "$out"
  fi
}

section "SSH keys (id, title, created)"
list user/keys '.[] | [.id, .title, .created_at] | @tsv'

section "SSH signing keys (id, title, created)"
list user/ssh_signing_keys '.[] | [.id, .title, .created_at] | @tsv'

section "GPG keys (id, key id, created, expires)"
list user/gpg_keys '.[] | [.id, .key_id, .created_at, (.expires_at // "never")] | @tsv'

section "App installations (id, app, repositories, created, updated)"
list user/installations '.installations[] | [.id, .app_slug, .repository_selection, .created_at, .updated_at] | @tsv'

cat <<'EOF'

Not listed by any API. Check these in the browser:
  Settings, Developer settings: personal access tokens (classic and fine-grained)
  Settings, Applications: authorised OAuth apps and installed GitHub Apps
  Settings, Sessions: active sessions

Delete the token you used for this run.
EOF
