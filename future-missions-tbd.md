# Future missions, TBD

Ideas for missions that have not been shaped into briefings yet. Recorded so they are
not lost. Listing an idea here implies no commitment, no ordering and no decision that
it is a good idea.

## Seats: wait and let it come out in the design

Raised by Jim, 2026-09-21, after reading the seats comparison in
`docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md` in the tsk repo.

Jim's read: we haven't reached the point of needing seats yet because we are still
bootstrapping in supervised mode. Until we actually have agents autonomously running and
trying to manage each other we don't need it, or rather Jim is sitting in all the seats.

The word "seat" itself doesn't make intuitive sense to him. He asked for another way to
describe it given tsk's context, as a future note only. Decision: wait and let it come out
in the design.

Connects to an open point already recorded in `docs/domain/mission-model.md` under
mission categories: a mission with no end point "is closer to a description of an actor
than to a unit of work: the responsibility it carries, and the skills it needs", and
whether tsk models that separately is open.

## tsk talking to Wheelhouse, and messaging into the tsk nexus

Raised by Jim, 2026-09-20, from Yegge's tweet about orchestration factories exchanging
ideas with Gas Town and Wheelhouse.

Jim's read: he wants tsk able to talk to Wheelhouse, and wants to interact via some kind
of messaging with his tsk nexus. He asked whether these are the same answer.

Found so far: Gas Town and Wheelhouse have no documented protocol for messaging between
separate, independently-run installations. Gas Town's own routing and mail work within
one town only. What Gas Town, Wheelhouse, and other independent orchestrators share is
Beads, a shared substrate, not a network protocol between them, consistent with the
existing finding in `docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md`
that beads is "technically symbiotic... a swappable substrate and a research baseline,
not an ally."

The tsk nexus (`docs/domain/territory-and-nexus.md`, `jimbarritt/tsk-nexus`) is a
discovery and routing index today, with no messaging layer, and that gap is already
logged as open in that document. Both of Jim's asks land on the same missing piece: no
ready-made inter-instance protocol exists to reuse from either Gas Town or Wheelhouse. A
messaging layer for the tsk nexus would be new design work, not an adoption of something
already built elsewhere.

## Switch tsk's CLAUDE.md to also load as AGENTS.md

Raised by Jim, 2026-09-20, alongside `docs/kb/claude-code-mods.md`.

Jim's read: switch from `CLAUDE.md` to `AGENTS.md` everywhere, for compatibility.

Confirmed while researching mods: Claude Code already supports this today, through the
built-in `agents-md` mod, on this container's version, 2.1.278. Four loading modes
exist; `claude-md-and-agents-md` loads both side by side. Not yet decided: which mode,
whether tsk renames `CLAUDE.md` to `AGENTS.md` outright or adds `AGENTS.md` alongside
it.

"Everywhere" means every one of Jim's repos, done one at a time, not in one pass.

## External access to Claude Code Remote sessions, and deep-linking

Raised by Jim, 2026-09-19.

Research mission: does an external process, not a Claude Code session, get access to
list or manage Claude Code Remote sessions.

Found so far: `create_trigger` and `list_triggers` name three caller types, in-session,
toolbox, and OAuth. `list_triggers` restricts one parameter to OAuth callers and errors
for the other two on that path. An OAuth-authenticated external caller reaches this
backend. Not confirmed for session listing specifically; inferred, since Claude Code
Remote's own web interface is not itself a Claude Code session and needs to list
sessions to render itself.

Jim's follow-up question, not yet researched: can a URL be generated that deep-links
into a specific Claude iOS session.

Connects to the substrate-independence point in
`docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md`: session discovery and
access from outside any one Claude Code session bears on whether a coordinator built
outside Anthropic's own tooling could reach a Claude Code session as an actor.

## The "robot factory" framing, and why Jim is doing this concretely

Raised by Jim, 2026-09-19, while pointing at `docs/kb/yegges-levels.md`.

Yegge's levels is partly why Jim is doing tsk. It also frames the market and the niche:
as more organisations realise they need to build a "robot factory", coordinating enough
concurrent agents that a person can no longer track them by hand, that is where jobs for
engineers will be. Jim is writing a LinkedIn post about this.

Whether tsk itself is adopted is a separate question. One of Jim's reasons for building
it, and writing about it, is to experience concretely what running that factory means,
not only to theorise about it.

The point behind the LinkedIn post, added once the two verbatim tellings of Yegge's
ladder were both in `docs/kb/yegges-levels.md`: if you don't actually climb the ladder,
it's hard to really grok what it means. The two tellings themselves are evidence for
this. Yegge described the same middle rungs differently a month apart, physically the
first time, psychologically the second, because what a climber notices changes as they
climb it. You can't get that from reading about it once.

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

Done, same day. The comparison lives in `docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md` in the tsk
repo, which was widened from the beads assessment to cover the field under one verdict.

In short: Claude Code Projects converges on Thread as the unit that holds context and
persists, and diverges by fusing three pairs tsk keeps apart. A project is a mission
described loosely. It models no Product, no Delta and no Scale, so its dimensional
limit matches beads'.

Two things the comparison left for other work. M-BOOT-03's T-07 defines a run loop the
platform may now provide, so check before building it. And the live cross-mission fact
with no home in tsk, such as a release date moving, is the same gap as design decision
recording beyond ADRs, below.

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

## Auto mode's safety classifier moves server-side

Raised by Jim, 2026-09-20, from the agentic engineering news feed
(`2026-09-19-anthropic-auto-mode-classifier-server-side`).

From Claude Code 2.1.278, Enterprise, Claude API, Bedrock, Vertex and Foundry accounts
have auto mode's safety classifier run server-side by default, at no charge. A
client-side fallback covers the case where the server cannot be reached, and a new
"Auto mode server" row in `/status` shows which mode a session is in.
[code.claude.com/docs/en/auto-mode-classifier-billing](https://code.claude.com/docs/en/auto-mode-classifier-billing).

## Claude Code workflows pause at a usage limit instead of dropping agents

Raised by Jim, 2026-09-20, from the agentic engineering news feed
(`2026-09-14-claude-code-workflows-pause-usage-limit`).

In Claude Code 2.1.271, a dynamic workflow that hits a usage limit now pauses and
resumes automatically once the limit resets, rather than dropping the agents running
inside it. [code.claude.com/docs/en/changelog#2-1-271](https://code.claude.com/docs/en/changelog#2-1-271).

## Claude Code approves network hosts per Bash command in sandboxed auto mode

Raised by Jim, 2026-09-20, from the agentic engineering news feed
(`2026-09-14-claude-code-sandboxing-per-command-hosts`).

In Claude Code 2.1.271, sandboxed auto mode lists the network hosts a Bash command
needs, and the safety classifier reviews the hosts together with the command. Approval
opens those hosts for that one command only.
[code.claude.com/docs/en/sandboxing#per-command-allowed-domains-in-auto-mode](https://code.claude.com/docs/en/sandboxing#per-command-allowed-domains-in-auto-mode).

## Cursor launches Projects, a coordinator agent for large-scale work

Raised by Jim, 2026-09-20, from the agentic engineering news feed
(`2026-09-10-cursor-projects`).

Cursor Projects uses a coordinator agent that plans tasks and delegates to subagents,
running in the cloud so closing a laptop does not stop the work. It can watch Slack
channels, pull requests and schedules to start work on its own, and scales to thousands
of concurrent subagents. [cursor.com/changelog](https://cursor.com/changelog).

A fifth team reaching this coordinator-and-threads shape, after Claude Code Projects,
Gas Town, Copilot's `/fleet`, and Agent HQ. Bears on
`docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md`.

## LangChain scores agent evals with a classifier instead of an LLM judge

Raised by Jim, 2026-09-20, from the agentic engineering news feed
(`2026-09-20-langchain-jev-as-a-judge-agent-evals`).

LangChain's Jev-as-a-Judge answers Choice, Score or Boolean questions about an agent's
behaviour directly as typed output, instead of generating text like an LLM judge.
LangChain says this is 92 to 913 times more consistent than an LLM judge, and cheaper
and faster, so teams can run evals more often.
[langchain.com/blog/jev-agent-evals-langsmith](https://www.langchain.com/blog/jev-agent-evals-langsmith).

Further reference, added by Jim, 2026-09-20: [typesafe.ai](https://typesafe.ai/), Jev's
own site. Jim is signing up to its waitlist.

Jim, 2026-09-20: this one needs research.

Same Jev classifier already noted in `docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md`
from LangChain's harness piece, now applied to evals specifically.

## OpenAI's agentic software factory

Raised by Jim, 2026-09-20, from the agentic engineering news feed
(`2026-09-15-pragmatic-engineer-openai-agentic-software-factory`).

OpenAI runs Codex agents through code generation, testing, review, deployment and
monitoring with little human intervention, using specialised agents for different parts
of the infrastructure. Pull requests have grown roughly tenfold in six months, forcing
OpenAI to rethink CI/CD, code review and how it uses pull requests at all.
[newsletter.pragmaticengineer.com/p/openai-software-factory](https://newsletter.pragmaticengineer.com/p/openai-software-factory)
(this sandbox's egress proxy blocks the domain directly; not read from the page, only
the feed's own summary, confirmed by Jim as matching the article).

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
