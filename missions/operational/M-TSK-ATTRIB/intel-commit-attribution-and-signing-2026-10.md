# Commit attribution and signing in cloud sessions

Intelligence gathered on 2026-10-27 in a Claude Code cloud session on this repo.

## What the environment sets

The cloud container sets this git config before any commit:

| Key | Value |
|---|---|
| `user.name` | `Claude` |
| `user.email` | `noreply@anthropic.com` |
| `commit.gpgsign` | `true` |
| `gpg.format` | `ssh` |
| `user.signingkey` | `/home/claude/.ssh/commit_signing_key.pub` |

Git writes `user.name` and `user.email` into the author and committer fields of each
commit. `commit.gpgsign` with `gpg.format = ssh` signs each commit with the private key
beside that public key. GitHub holds the matching public key on the `claude` account, so
the commit list shows "claude committed" with a "Verified" badge.

The session also appends two trailers to each commit message: `Co-Authored-By: Claude
<model> <noreply@anthropic.com>` and `Claude-Session: <session URL>`. The trailers are
text in the message. They are separate from the author field.

## What a signature covers

An SSH or GPG commit signature covers the whole commit object: the tree hash, the parent
hashes, the author line, the committer line and the message. A change to any of these
produces a different object, and the old signature no longer matches it.

GitHub marks a commit "Verified" when the signature matches a key registered on a GitHub
account, and the committer email is one of that account's verified emails. A signature
from one account on a commit whose committer email belongs to another shows "Unverified".

## Consequences for a change of author

- Setting `user.name` and `user.email` to a robot identity changes the author and
  committer fields of new commits. The signing key stays Anthropic's, so the committer
  email and the key's account no longer match. The badge drops or reads "Unverified".
- To keep a badge under a robot identity, register a signing key on the GitHub account
  that owns the robot's email, and give the session that key and the config above.
- GitHub links a commit to an account and shows its avatar only when the author email is
  a verified email of that account. An email under a domain Jim controls can be added to
  a GitHub account and verified there.

## Consequences for a history rewrite

`git filter-repo --mailmap <file>` rewrites the author and committer of every commit that
matches a mailmap line, in one pass, then `git push --force` on each branch publishes it.

- Every rewritten commit, and every descendant, gets a new hash. Each clone, including
  each cloud session, must re-clone or hard reset to the new history.
- The rewrite drops every existing signature, because the signed bytes change. The
  "Verified" badge disappears from rewritten commits unless each is re-signed with a key
  the new committer's account holds. `git filter-repo` does not re-sign.
- `refs/heads/tsk/ledger` and `refs/heads/tsk/docs` are separate histories. A rewrite of
  `main` does not touch them. Continuation state entries record `code.commit` hashes of
  `main`; after a rewrite those hashes name commits that no longer exist on any branch.

## Copyright and licence

A commit's author field and its `Co-Authored-By` trailer are metadata. Neither assigns
copyright or grants a licence. Ownership of output follows the terms of the account that
produced it; Anthropic's consumer and commercial terms both assign output rights to the
customer. The open legal question is human authorship: the US Copyright Office does not
register material with no human author, which affects whether a copyright exists at all,
not who holds it. None of this is legal advice.
