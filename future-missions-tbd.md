# Future missions, TBD

Ideas for missions that have not been shaped into briefings yet. Recorded so they are
not lost. Listing an idea here implies no commitment, no ordering and no decision that
it is a good idea.

## What pausing a thread really means

Raised by Jim, 2026-09-17.

Investigate further what pausing a thread really means. A thread should have a state of
paused or running.

There is no such state today. A pause appends a continuation state entry and nothing
records that the thread is paused, so the only evidence is whether the latest entry still
describes where things stand. Work done after a pause leaves that entry stale with
nothing marking it so, and the thread has to be paused again.

## Claude Code Projects redesign: compare against tsk's own model

Raised by Jim, 2026-09-18, from
[claude.com/blog/projects-redesigned](https://claude.com/blog/projects-redesigned).

Anthropic announced a redesigned Projects experience for Claude Code, in beta from
2026-09-17. A project now has a coordinator that receives instructions and routes each
one to a new or existing worker thread, monitors progress, reviews output, and assembles
the result. Worker threads share project memory: decisions and instructions carry across
work spanning several days, for example a release date moving or who must be consulted
before a service changes. Projects also gain a library of user files and Claude-created
artefacts, so a later thread can reuse material an earlier one produced. Each worker
thread is a full Claude Code session in its own right, so parallel work counts against
plan limits accordingly.

Research mission: this sits in the same territory tsk models. A coordinator routing to
threads maps onto tsk's actor and thread concepts; shared project memory maps onto
something like the ledger; the artefact library maps onto tsk's own Artefact term.
Understand where the two agree, where they differ, and whether tsk's model should account
for a product that now does part of this natively.

Open: whether this changes tsk's scope, whether tsk should integrate with or sit
alongside this feature, and whether tsk's terminology (coordinator vs. actor, project
memory vs. ledger) should converge with Anthropic's or stay deliberately distinct.
Sources found during capture, not yet read in full: the blog post above (blocked from
this sandbox's egress proxy), and secondary coverage at
[VentureBeat](https://venturebeat.com/orchestration/anthropic-launches-claude-code-projects-an-always-on-conversation-that-remembers-and-delegates-your-long-running-dev-work),
[Unite.AI](https://www.unite.ai/anthropic-redesigns-claude-code-projects-to-coordinate-agent-threads/),
and [DevOps.com](https://devops.com/anthropic-adds-a-coordinator-to-claude-projects-for-running-ai-work-in-parallel/).

## Report: spawning an agent session to execute a task

Logged 2026-09-16. Session `session_01EepA7GiwbvHJVWxr8JKQ3N`.

Jim planned to add the compact Software English instructions to `CLAUDE.md` (M-BOOT-02
T-12) himself. Instead this session spawned an agent session with `create_session` and
gave it the task.

The agent session started, read the Software English specification from an external
repository, wrote the instructions into `CLAUDE.md`, committed, and pushed to `main`.
It ran to completion with no intervention and no blocking prompts. T-12 is marked done
on this branch at commit `a448461`.

Jim's read: the harness works. Recorded here because it is the first end to end run of
spawn, execute, push without a human in the loop, and because it bears on M-BOOT-03,
whose objective is one unattended run producing a pull request and a run record.

## GitHub PR suggestion banner for metadata branch

Raised by Jim, 2026-09-16.

`tsk/bootstrap` is a real branch now (not a custom ref), which lets agents push to it
from cloud sessions. GitHub's web UI sees a real branch and shows a persistent banner
asking if you want to create a pull request from it, treating it like a development
branch. This is noise.

Candidate solutions: a branch protection rule that hides the branch from PR suggestions,
or a `.github/workflows` check that auto-closes any PR opened against or from
`tsk/bootstrap`, or marking the branch as draft-only in some way, or documenting the
branch in a way that makes GitHub understand it is not for PRs.

Open: whether GitHub supports suppressing PR suggestions per branch, what mechanism
works, whether the solution applies to other metadata branches (if tsk ever has more
than one).

## Design decision recording beyond ADRs

Raised by Jim, 2026-09-17.

ADRs capture architectural decisions that have consequences lasting the lifetime of the
project. Not all design decisions are at that scale. Some are more local: why a
particular component is structured one way, why a feature works this way, constraints
discovered and recorded.

These decisions are worth capturing for future reference, so people rebuilding or
refactoring that code understand what was already decided. They may not warrant an ADR,
but they need to live somewhere that is not a code comment (those rot) and not a
passing conversation.

Research mission: what document types and structures make sense for recording these?
Does tsk need a "design notes" or "design rationale" directory separate from ADRs?
How granular should they be? Where do they live in the repo structure?

Open: scope (what qualifies), form (one per decision or grouped by area), archive
(do old decisions get pruned or kept forever), whether this is just a refined version
of the ADR process applied at finer scale or fundamentally different.

## tsk and Linear / Jira integration

Raised by Jim, 2026-09-16.

Research mission: explore the intersection between tsk's mission and task model and
Linear or Jira. Understand how the two relate, and whether tsk replaces one, complements
it, or interoperates with it.

Open: scope of the research, candidate tools and use cases, and whether this shapes tsk's
design or is purely a downstream question.

## tsk metadata in the nexus, not the repo

Raised by Jim, 2026-09-16.

tsk currently stores its metadata branch in the repo it manages. This works for repos
you own, but breaks for repos you don't: you cannot push a branch without the repo's
permission, and you don't want to ask owners to allowlist your metadata branch.

Solution: store the metadata branch in the nexus repo instead, namespaced by target
repo (e.g. `refs/heads/repos/owner/repo-name/missions`). The nexus is already meant to
index repos; reusing it to hold their tsk metadata is a natural extension.

Open: namespace design, whether a single nexus branch per target repo is enough or
whether finer granularity is needed, whether this changes the mission model or
thread continuity story.

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

## Autonomous conflict resolution on tsk/bootstrap pushes

Raised by Jim, 2026-09-18, alongside the `push-bootstrap-ref.sh` fix for concurrent
pushes (M-BOOT-02).

That fix rebases onto origin's tip and retries when a concurrent push is
non-conflicting, but aborts with a message and stops on a real conflict, for a human to
resolve by hand. An autonomous agent has no human to hand this to.

Jim raised two directions: escalate to Jim for assistance, or have the agent attempt
self-resolution. He connected this to "Agents interrupting each other" above: the two
sessions in conflict could have a discussion with each other, possibly over the same
peer to peer gossip protocol raised there, rather than one session resolving the
conflict blind.

Jim's read: might be part of a later mission. Not decided.

Open: whether self-resolution is safe for ledger content (missions, tasks, thread
state) at all, what a discussion between sessions would need mechanically, and whether
it is the same mechanism as the interrupting-each-other idea or a different one.

## Actor definition

Raised by Jim, 2026-09-17, while resolving what to do with a thread that has no mission.
An administrative mission (idea capture, tidy up, architecture review, distilling
mission reports) is almost an "actor definition", something like a custom agent. It is
more about the actor than about the work: what is your responsibility, what are your
skills.

Jim's read: this might be an additional concept. Come back to it later. An administrative
mission is what the four examples are recorded as for now.

Doctrine has a close match, found in the same session: the mission essential task list,
"a listing of tasks the unit must be able to perform", held per unit and derived from the
unit's anticipated missions rather than from any single one. Standardised METLs are the
official lists of "the fundamental tasks that units are designed to perform in any
operational environment". See `docs/domain/mission-model.md` in the tsk repo, under
mission categories, for the rest of that cross-check.

Open: whether an actor definition is a first-class object in tsk, and how it relates to
Actor, which the ubiquitous language already defines, and to the administrative mission.
