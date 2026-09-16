# Future missions, TBD

Ideas for missions that have not been shaped into briefings yet. Recorded so they are
not lost. Listing an idea here implies no commitment, no ordering and no decision that
it is a good idea.

## Validation: Agent spawning and autonomous execution

Logged 2026-09-16.

Spawned agent session `session_01EepA7GiwbvHJVWxr8JKQ3N` to add software English prompt to
tsk's CLAUDE.md (M-BOOT-02 T-12). The harness worked as designed: agent came up, read the
specification from an external repo, extracted and integrated the changes, committed and
pushed to main — all without intervention. No blockers, no prompts. The mechanism is sound.

## tsk and Linear / Jira integration

Raised by Jim, 2026-09-16.

Research mission: explore the intersection between tsk's mission and task model and
Linear or Jira, understanding how the two systems might relate, whether tsk replaces
one, complements it, or needs to interoperate with it.

Open: scope of research, candidate tools and use cases, whether this shapes tsk's
design or is purely a downstream question.

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

## Software English review, and document types, in the harness

Raised by Jim, 2026-09-16, after `docs/kb/session-creation-and-environments.md` came out
a mess: written as a first-person log of one session's inspection, with live identifiers
in it, when what was wanted was a reference document.

Two connected needs:

1. Software English review as part of the harness, so prose an agent writes into the
   repository gets checked rather than trusted. Spec:
   https://github.com/jimbarritt/software-english. M-BOOT-02 T-12 covers putting the
   compact instructions into `CLAUDE.md`, which is the instruction half. This is the
   review half.
2. The harness defines specific document types with specific rules, so that "create a
   ref document" has one meaning and produces the right shape. A reference document, a
   mission briefing, an intel file, an ADR and a KB entry are not the same kind of
   writing, and today only the mission briefing has a template.

Jim's read: this is probably a set of skills rather than more prose in `CLAUDE.md`.

Open: which document types are worth defining, whether a skill per type is the right
granularity, and whether review runs as a skill the agent invokes, a hook, or a check in
CI alongside the secrets scanning in the mission above.

## Long lived workers, and where continuity belongs

Raised by Jim, 2026-09-16, from the question of whether cloud sessions can be reused as a
pool of workers rather than created fresh per mission.

Reusing a session is mechanically supported: `create_trigger` fires into a named
`persistent_session_id`, and `claude -p --cloud <session-id>` queues a message into an
existing session. See `docs/kb/session-creation-and-environments.md` in the tsk repo for
the mechanics and their limits.

The domain question it raises: a worker that takes mission after mission accumulates
continuity of its own, independent of any single mission. The ubiquitous language today
defines a thread as the execution sequence, resumable "possibly as a different actor",
which puts continuity in the work and lets actors come and go. A worker pool inverts
that: continuity sits in the actor, and missions pass through it. Both are coherent. They
disagree about where continuity is anchored.

This is the same tension already flagged against M-BOOT-02 T-04, that a thread may be
more about the actors than about overall work status. Settle them together.

Open: whether tsk models the worker as a first-class thing at all, or whether a worker is
simply an actor that happens to persist, with threads still carrying the continuity.
