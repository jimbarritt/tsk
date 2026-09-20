# Mints the short random-looking tokens tsk uses as identifiers: the per-clone
# key in bootstrap-wt-lib.sh, and thread IDs in thread-lib.sh.
#
# Sourced, not executed directly.
#
# Why a hash of a seed rather than a read of /dev/urandom: the obvious idiom,
# `tr -dc 'a-z0-9' < /dev/urandom | head -c 8`, has an unbounded producer, so
# head must cut tr off mid-write. tr then takes SIGPIPE and prints
# "tr: write error: Broken pipe" on every single call, and the pipeline reports
# a non-zero status, which callers had to suppress with `set +o pipefail`. Both
# were noise from a correct operation, and both had to be told apart from a real
# failure by hand. Hashing a finite seed removes the unbounded producer, so
# there is no signal to absorb and no status to suppress: a non-zero exit or a
# short digest here is a genuine fault, and this function fails on it.

# tsk_mint_token <length> [extra-seed]: print a lowercase hex token of exactly
# <length> characters. Exits non-zero, printing why, if it cannot.
#
# The seed combines a nanosecond timestamp, this shell's PID, bash's own
# $RANDOM, and the caller's optional extra seed (a clone path, say). No single
# element carries uniqueness alone: on a platform whose date has no %N the
# timestamp degrades to whole seconds, and $RANDOM plus the PID still separate
# two calls.
tsk_mint_token() {
  local length="${1:-}" extra="${2:-}" seed digest

  case "$length" in
    '' | *[!0-9]*)
      echo "tsk_mint_token: length must be a positive integer, got '$length'" >&2
      return 1
      ;;
  esac
  if [ "$length" -lt 1 ] || [ "$length" -gt 64 ]; then
    echo "tsk_mint_token: length must be between 1 and 64, got $length" >&2
    return 1
  fi

  seed="$(date -u +%s%N):$$:${RANDOM}:${extra}"

  if ! digest="$(printf '%s' "$seed" | sha256sum)"; then
    echo "tsk_mint_token: sha256sum failed" >&2
    return 1
  fi
  digest="${digest%% *}"

  if [ "${#digest}" -lt "$length" ]; then
    echo "tsk_mint_token: sha256sum returned an unusable digest: '$digest'" >&2
    return 1
  fi

  printf '%s\n' "${digest:0:$length}"
}
