# Future missions, TBD

Ideas for missions that have not been shaped into briefings yet. Recorded so they are
not lost. Listing an idea here implies no commitment, no ordering and no decision that
it is a good idea.

## Agents interrupting each other

Raised by Jim, 2026-09-15.

Agents should be able to interrupt each other. The mechanism suggested is tsk running a
peer to peer "gossip" protocol that lets agents send each other messages.

Open: whether this is needed at all, given that agents can already communicate through
git. Recorded as an idea, not a direction.

## Generalising the harness's cross-repo needs

Raised by Jim, 2026-09-15, alongside M-BOOT-02 T-06.

tsk's `CLAUDE.md` now carries a tsk-specific instruction: attach `ksobr-transcripts`
with `add_repo` before pushing a transcript at session end. That is a stopgap. Once the
harness is meant to run in more than one repo, a harness needs a general way to declare
"this repo also needs this other repo attached" that does not mean hand-writing the
instruction into every consuming repo's `CLAUDE.md`.

Open: what that declaration looks like, and whether it can be read by a `SessionStart`
hook at all, given `add_repo` is an MCP tool call the agent makes, not something a bash
script can invoke.

## Safeguard main against leaked secrets

Raised by Jim, 2026-09-16, after an agent committed a live session ID to a public repo.

An agent can push something to `main` that shouldn't be there: a credential, a session
ID, anything identifying. Need a way to catch this deterministically rather than relying
on an agent noticing.

Candidate shape, sketched, not decided:

1. An agent pushes a commit to `main`.
2. A GitHub Action fires on the push and reviews it — Copilot, or a spawned Claude
   session, checking the diff for secrets and other things that shouldn't be public.
3. If it finds something, then what? Open questions, none answered yet:
   - A pre-commit hook is not a reliable first line of defence here: `.git/hooks/` is
     never cloned, so it only exists if something installs it into each fresh session's
     checkout, and `--no-verify` skips it anyway. GitHub's own secret scanning push
     protection is server-side and can't be skipped that way, but only catches
     recognised secret patterns, not something project-specific like a session ID.
   - `git revert` does not remove the leaked content from history: the original commit
     and blob stay fully present and fetchable by SHA. Actually removing it needs a
     history rewrite (`git filter-repo`, or BFG) and a force-push, and even then: forks
     keep the old history untouched, existing clones keep the old blob until they
     re-clone, and CI logs that printed the secret are a separate purge, not touched by
     rewriting git history at all. The one action that reliably neutralises a leaked
     credential is rotating it. History rewriting is cleanup, not the fix.
   - Race condition: if the review is asynchronous, another commit can land on `main`
     while it runs. A plain revert may not apply cleanly against a `main` that has moved
     on. A history rewrite is worse: it changes the bad commit's SHA and rebases
     everything after it. Whether `main` should be frozen while a finding is under
     remediation is unresolved.
