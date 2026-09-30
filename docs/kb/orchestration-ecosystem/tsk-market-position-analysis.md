# tsk market position analysis

Date: 2026-07-01 (beads re-examination), distilled 2026-09-14, widened to cover Claude
Code Projects 2026-09-18, JetBrains Air 2026-09-22, and Cursor Projects 2026-09-29.
LangSmith was added on 2026-09-29 and OpenAI Dots on 2026-09-30, each as a related system,
not a fifth system assessed.

## Status

Accepted as the current strategic position. Provisional: blocked by the token-saving
experiment referenced below, which had not run as of this writing.

## Context

This document holds tsk's strategic position against others building in the same space.
One section per system assessed, then a single verdict over all of them. It is widened
as new systems appear rather than split, so tsk's position is stated in one place and
cannot drift between documents.

Four systems are assessed: beads, an independent tool; Claude Code Projects, a feature
of the platform tsk's own harness runs on; JetBrains Air, an independent tool that
coordinates several vendors' agents at once; and Cursor Projects, a coordinator in
Cursor's editor and cloud.

## Beads

Beads (`gastownhall/beads`, v1.0.4 at the time, roughly 25k GitHub stars) is a graph
issue tracker built for agent work. It matured substantially after tsk's initial design
and independently converged on much of tsk's architecture: Dolt as a version-controlled
substrate, a daemon in single-writer mode over Unix sockets, an `issues.jsonl`
interchange format that is explicitly not the source of truth (the inverse of tsk's
design, where the NDJSON event log is the source of truth and SQLite is a disposable
cache, but the same two-layer instinct), an MCP package, and persistent-memory features
(`bd remember`, `bd prime`). It is positioned as a plan-replacement and a
token/context-economy play, the same positioning tsk holds.

This raised the direct question: is tsk still worth pursuing, and is symbiosis with
beads still possible, or is the space now purely competitive?

### Where beads has converged, and where it hasn't

The overlap is architecture and positioning, not conceptual model.

- **Converged**: Dolt, daemon, single-writer, Unix sockets, the plan-replacement pitch,
  the token/context-economy claim, MCP, persistent memory.
- **Not converged**: beads stays inside the [Navigation](../../domain/ubiquitous-language.md#navigation)
  dimension, with a discrete epic-to-story-to-sub-task hierarchy via dotted IDs, which
  is exactly the artificial-tier boundary tsk's [Scale](../../domain/ubiquitous-language.md#scale)
  dimension exists to dissolve. Beads has no model of
  [Product](../../domain/ubiquitous-language.md#product) (the thing being built), no
  first-class [Delta](../../domain/ubiquitous-language.md#delta), and no continuous,
  fractal Scale.

Yegge's own framing of beads ("forensics, the why of your project, joined against the
what/where/how of your git commits") gestures at a join between intent and change,
loosely Product joined against Delta, but treats it as a join between two systems, not a
modelled domain construct in its own right. It is the closest beads' framing comes to
reaching outside Navigation.

## Claude Code Projects

Announced 2026-09-17, in beta. A project holds a goal, a repo or other context, and
configuration for its cloud environment, connectors, plugins, instructions and model. A
coordinator receives instructions and routes each one to a new or existing worker
thread, monitors progress, reviews output, and assembles the result. Worker threads
share a project memory that carries decisions across days of work, and draw on a library
holding both the files a person adds and the artefacts Claude produces. Each worker
thread is a full Claude Code cloud session on its own branch and copy of the repo, and
can subdivide into subagents, loops and workflows.

This matters more directly than beads does. Beads is an independent tool tsk could adopt
or measure against. Claude Code is the platform tsk's own harness runs inside.

### Where Claude Code Projects has converged, and where it hasn't

- **Converged, on the unit that matters**: a thread that holds its own context, persists
  across days of work, can be checked on and steered mid-flight, and subdivides into
  subagents, loops and workflows. This is close to
  [Thread](../../domain/ubiquitous-language.md#thread) as tsk defines it, and as
  [vision.md](../../vision.md) describes it, "fractal, pausable, resumable, and able to hold
  their own context, the way a stack frame does in a programming model". The announcement
  does not describe an explicit pause and resume state the way tsk's continuation
  mechanism does; what converges is the persistent, steerable unit itself, not a
  confirmed match on that specific mechanic. Two designs reaching a similar unit
  independently is evidence the unit is real.
- **Not converged**: three pairs are fused there that tsk keeps apart.

| Pair | Claude Code Projects | tsk |
|---|---|---|
| Thread and session | one object | bound, not identical, with the reason recorded under [Actor](../../domain/ubiquitous-language.md#actor) |
| Inputs and outputs | one library | [Ledger](../../domain/ubiquitous-language.md#ledger) and [Artefact](../../domain/ubiquitous-language.md#artefact), split by definition |
| Work and its container | a project holds the work | no container; one entity at every zoom |

None of those is tsk missing a part. Each is tsk declining to complect two things the
product ships joined.

**A project is a mission described loosely.** A repo, a goal and configuration is, in
tsk, a territory, an objective and constraints, each with its own definition. tsk also
refused a container above mission once already, on stronger grounds than this:
[domain-model-overview.md](../../domain/domain-model-overview.md) records that no campaign
or major-operation type is added "even though doctrine nests campaign above mission:
Scale, one of tsk's four dimensions, rejects fixed echelons". Doctrine offered a
container type with an argument behind it and tsk declined. Missions are scale free, so
no project type is needed.

**A coordinator is an actor.** The [Actor](../../domain/ubiquitous-language.md#actor) entry
already anticipates one: "a maintenance or coordination actor is the clear case, picking
up whatever needs attention across missions rather than being handed one." tsk has not
built the automation, which is a different statement from the model lacking a place for
it.

**Shared memory is mostly a boundary tsk drew.** Constraints such as who to consult
before a service changes belong in a briefing's Constraints section. Check-in frequency
and update verbosity only make sense because a harness executes the work, so they fall to
ksobr under the existing boundary test. What has no home in tsk is a live cross-mission
fact such as a release date moving. That gap is real, small, and already recorded in the
ledger's future missions as design decision recording beyond ADRs.

**Same dimensional limit as beads.** Claude Code Projects models no Product, no
first-class Delta, and no continuous Scale. It is agent orchestration, which is the axis
[vision.md](../../vision.md) already uses to separate tsk from beads: tsk "is about
collaboration between humans and agents, and between humans and humans, not agent
orchestration alone."

### Locked to a substrate, four of five: tsk's real premise

Claude Code Projects is not the only coordinator-and-threads system, and most of the
ones found so far share a property Claude Code Projects has on its own: none of them is
portable away from the vendor that built it. JetBrains Air, covered in full below, is
the one exception found, and only at the worker layer, not the coordinator layer.

- **Claude Code Projects** (Anthropic): every thread is a Claude Code cloud session.
- **Gas Town** (Steve Yegge, independent of Anthropic): runs 20 to 30 Claude Code
  instances at once. Built by someone with no reason to favour Anthropic's product over
  any other, and it still only runs Claude Code.
- **`/fleet`** (GitHub Copilot CLI): an orchestrator decomposing a task and dispatching
  Copilot's own agents in parallel. The same coordinator-and-workers shape, on GitHub's
  own substrate.
- **Agent HQ** (GitHub): lets a Copilot Business or Pro user pick Claude or Codex as the
  model a task runs on. This looks like portability, but the choice is which vendor's
  model GitHub's own orchestration layer calls, not the coordinator-and-threads
  abstraction itself running on a substrate its own vendor didn't build.
- **JetBrains Air**, the exception, covered in full below: a worker thread can run
  Claude, Codex, Gemini CLI, Junie, or any other agent speaking the Agent Client
  Protocol, an open protocol, not one vendor's own. The coordinator itself is still a
  single vendor's product.

Four of the five bolted the coordinator-and-threads shape to one vendor's execution
layer, and that is not carelessness on any of their parts; a coordinator has to actually
run the threads it creates, and the fastest way to build one is against the runtime
already in front of you. JetBrains Air shows a fifth way was available: decouple the
worker layer from any single vendor by building against an open protocol instead of one
runtime.

This is the premise tsk is built on: the
domain, mission, thread, actor, ledger, is defined without reference to which substrate
executes a thread, and that abstraction is more powerful than any one vendor's
implementation precisely because it is not confined to that vendor. A tsk thread's actor
could be a Claude Code session, a Copilot CLI session, or something else; nothing in the
domain model encodes that choice, the way "every thread is a Claude Code cloud session"
is encoded into Claude Code Projects by construction.

**Not yet demonstrated, and worth being honest about.** tsk's own bootstrap harness
currently runs inside Claude Code specifically; `CLAUDE.md` and ADR 0008 both describe
mechanics tied to "the Claude Code cloud sandbox proxy". That is an implementation fact
about how tsk bootstraps itself today, not evidence the domain model achieves substrate
independence in practice. The real test is running a tsk thread with a Copilot CLI
session, or some other substrate, as its actor, and confirming nothing in the domain
model quietly assumed Claude Code underneath. Untested, and worth tracking as its own
question rather than asserting the premise holds because it was designed to.

JetBrains Air narrows what that test still has to prove. It shows a worker being
vendor-independent is buildable and already shipping, at the protocol layer. What it
does not show is a coordinator, missions, threads, actors, a ledger, defined without
reference to any vendor's product at all: Air's own coordinator is a JetBrains
application, not a portable abstraction. That second half remains tsk's own,
untested claim.

### The context boundary: the sharpest difference

Claude Code Projects makes the work fit the context. tsk makes the work outlive the
context.

The announcement splits work by task or domain boundary, not by context size: its two
worked examples are a thread per endpoint being profiled, and a thread per repo whose
callers need migrating. It does not say whether or how a thread's scope is calibrated
against its own context window, and it does not say what happens if a single thread's
task runs long enough to approach that limit.

This analysis draws a parallel, not a citation, to a separate pattern Anthropic has
published for long-running agents, recorded in
[agent-context-self-regulation-and-unattended-handoff.md](../agent-context-self-regulation-and-unattended-handoff.md):
an initializer writes a `feature_list.json` of 200-plus granular features before any
coding agent runs, so each piece stays small enough to finish inside one context. That
decompose-first pattern is a plausible mechanism behind how Projects keeps its threads
completable. It is not confirmed by the Projects announcement itself, which never
mentions it.

tsk inverts it. A thread holds the mission and runs until it judges the objective met.
The unattended handoff pattern in that same document writes the stop condition as a
disjunction: the thread prints its own context usage each turn, and crossing the ceiling
makes the condition refuse until a handoff file exists, is committed and pushed, and a
successor has been spawned from inside the condition. The document's own summary: "the
goal clears with state on disk and a successor already running." No controller decides
the split. The thread establishes that it cannot continue and hands to itself.

| | Claude Code Projects | tsk |
|---|---|---|
| Who sizes the work | the coordinator, by how it scopes each request | nobody; the thread runs until the objective is met |
| At the context limit | not described | the thread writes state, pushes, and spawns its successor, inside the goal condition |
| What the successor reads | not described | the repository, because a cloud session starts from a fresh clone |
| Where continuity lives | shared project memory | the ledger |

**The recursion tsk already named.** A coordinator is a conversation. It accumulates
memory across days and it fills. The same document records this as operating context 3:
"the orchestrator is itself a session, and it eventually hits the same context limit its
workers do. Continuation has to apply recursively, not only to the leaves." Shared
project memory mitigates that without answering it, because retrieval of what was
decided is not continuation of a running loop.

**The announcement is silent on this, not just brief about it.** Read in full, it
describes shared memory for decisions and preferences, and describes usage limits at the
plan level, aggregated across every thread a project runs at once. It says nothing about
what happens when one worker thread's own context window fills mid-task. That silence
could mean the platform decomposes finely enough in practice that it rarely happens, or
that it is not yet solved, or that it is handled by a mechanism the post simply did not
cover. The test is cheap and settles it either way: give a Projects thread work that
cannot fit in one context and watch what it does.

**This reframes the substrate question favourably.** If a Projects thread does hit a
wall, tsk's continuation mechanism is what it needs. That is tsk sitting inside a thread
rather than against one.

## JetBrains Air

Source: [Introducing JetBrains Air](https://blog.jetbrains.com/blog/2026/09/22/introducing-jetbrains-air/),
2026-09-22, and JetBrains' Air documentation, as of 2026-09-22. A claim with no
JetBrains source is marked as such.

The announcement covers "Air", "an open,
coherent system of products", of three parts, "available today alongside others that
will be introduced as the system develops": **Air in JetBrains IDEs**, "a complete
agentic development experience for directing and orchestrating agents and verifying
their work inside JetBrains IDEs"; **Air Teams**, coordinating software-delivery
workflows across developers and agents; and **Air Governance** (formerly JetBrains
Central), organisational policy, cost, and accountability for agent-driven development.
The rest of this section covers Air in JetBrains IDEs only, launched as a public preview
in March 2026. Air Teams and Air Governance are not assessed.

An agentic development environment, built on the codebase of Fleet, JetBrains' earlier,
abandoned lightweight editor (no JetBrains source; reported by The Register and
DevClass). Launched as a public preview in March 2026, macOS first. Windows and Linux are
available via JetBrains Toolbox as of September 2026. A coordinator dispatches a task to a worker, run in one of four
execution environments: Local Workspace, the default, applies changes directly to the
working copy with no isolation; Git Worktree, Docker, and Cloud each isolate the task
instead, on its own branch, named `air/<task>` in every case. Several tasks run in
parallel without one touching another's files only in the three isolated modes, not in
the default.

A task's work is split into agent roles: Planner turns an incoming task into an
implementation-ready task brief, decomposing it if needed; Implementer changes code on a
branch or pull request against that brief; Reviewer checks the result against scope and
repository rules; QA proves the change works through verification and tests. One agent
can hold all four roles in sequence, or a separate agent can hold each, run in parallel
(no JetBrains page confirmed; `agentization-cookbook.html` returns 404, and the list is
from independent coverage). Agents
coordinate through repository artefacts, files and the pull request, not through shared
chat history.

Unknown: whether a task's brief and role history persist in git beyond ordinary
commits, or only in Air's local state. The documentation states "History combines Git
commits with task-related snapshots", without saying where a snapshot is stored.

Status as of 2026-09-22, from `jetbrains.com/air/`: the IDE plugin is
labelled "Air Alpha", Air Teams offers "early access", and no pricing or tiers are
shown; use is free with a JetBrains AI subscription or an agent provider's own API key.
No JetBrains source confirms the $5 to $10 per user per month tier that third parties
report.

### Why an IDE vendor built this

An agent workflow that writes, reviews, and verifies code
without a human working in an editor reduces what an IDE licence buys. Building an agent
product is a rational hedge for a company whose revenue rests on IDE licences,
independent of how the move is read.

Two predictions follow from that incentive, neither confirmed by JetBrains:

- **Pricing.** Free today, in alpha, with no published tier (recorded above). A
  licence-revenue business has reason to price this once it is the product rather than a
  preview feature.
- **A state backend outside git.** The open question recorded above, whether a task's
  brief and role history persist in git beyond ordinary commits, is also the seam a
  vendor would use to hold a user to its own product: state that does not travel in the
  git history does not travel to a competitor's tool either.

### Where JetBrains Air has converged, and where it hasn't

- **Converged, on the same unit as Claude Code Projects**: a coordinator dispatching to
  a persistent worker, the same [Thread](../../domain/ubiquitous-language.md#thread)-shaped
  unit found there. Isolated when the task runs in Git Worktree, Docker, or Cloud mode;
  the default, Local Workspace, applies changes to the working copy directly, with no
  isolation. A third independent design reaching the Thread-shaped unit strengthens the
  same conclusion drawn from Claude Code Projects: two designs converging could be
  coincidence, three is a pattern.
- **Converged, a new data point**: role specialisation, Planner, Implementer, Reviewer,
  QA, over one task is a concrete instance of splitting a thread's work by function
  rather than by size. Coordination through repository artefacts instead of shared chat
  history is the same instinct behind an idea already in this ledger's future missions,
  "Agents interrupting each other": "whether this is needed at all, given that agents
  can already communicate through git."
- **Not converged, the same dimensional limit as beads and Claude Code Projects**: Air
  models no [Product](../../domain/ubiquitous-language.md#product), no first-class
  [Delta](../../domain/ubiquitous-language.md#delta), and no continuous
  [Scale](../../domain/ubiquitous-language.md#scale). It is agent orchestration inside
  an IDE, the same Navigation-only axis every other system assessed here occupies.

### Not locked to one execution vendor, at the worker layer

Air bundles Claude, Codex, Gemini CLI and Junie as built-in agents, and reaches roughly
twenty more, including GitHub Copilot, OpenCode, Pi and Cline, through the Agent Client
Protocol (ACP). ACP is co-developed by JetBrains and Zed, an open protocol, not one
vendor's private interface: a new agent is added by pointing Air at it through an
`acp.json` file, needing no integration purpose-built for that agent by JetBrains.

This is the exception recorded above, under "Locked to a substrate, four of five." Any
ACP-speaking agent can be a worker, so the worker layer is not locked to one vendor's
runtime the way a Claude Code Projects thread is locked to Claude Code. The coordinator
layer is a different question: Air runs as a JetBrains IDE, a web application
(`air.jetbrains.cloud`), and a CLI, not only a desktop application, but each of those is
still JetBrains-built and JetBrains-run. Adopted as a product, it is
the same kind of dependency Claude Code Projects is for tsk's own harness, just without
the "the platform tsk already runs inside" relationship that makes Claude Code Projects
the sharper case.

## Cursor Projects

Full reference: [cursor-projects.md](cursor-projects.md). Beta, announced 2026-09-10.

A project is one outcome in one repository. A coordinator agent produces a plan and
delegates the work to subagents, cloud or local. It does not write code. Project files sync
across every machine the agents use and hold research, artefacts and learned preferences.
Subscriptions start work from pull requests, Slack or a schedule.

- **Converged**: a coordinator that delegates and never edits code, workers handed only
  what they need, written state that outlives a session, and work started from an event.
- **Not converged**: inputs, outputs and preferences share one set of files where tsk
  separates the ledger from the artefacts. No pause, resume or continuation is
  documented. No approval or escalation model is documented. Product, Delta and Scale
  are not modelled. The same dimensional limit as beads, Claude Code Projects and Air.
- **Substrate**: not added to "Locked to a substrate" above. Whether a project can run
  another vendor's agents is not documented.
- **Availability**: not offered on Enterprise plans or with Privacy Mode (Legacy).

## Three-part viability verdict

1. **tsk as a research programme: yes.** Four independent designs have now converged on
   parts of tsk's model: beads on the architecture and the plan-replacement positioning,
   Claude Code Projects and JetBrains Air both on Thread as the unit that holds context
   and persists, and Cursor Projects on a coordinator that delegates and keeps written
   state. All four validate parts of tsk's underlying claim. None says anything
   about whether adding Product, Delta, and continuous Scale produces further measurable
   value. All four are credible baselines to measure the other three dimensions
   against, and this track continues regardless of the product outcome.
2. **tsk as a head-to-head agent issue tracker or orchestrator: no.** Against beads that
   category has an incumbent with distribution, maturity, an evangelist, and most of
   tsk's architecture. Against Claude Code Projects it is worse: the incumbent is the
   platform tsk runs on, shipping orchestration as a native feature. JetBrains Air adds
   a third incumbent with its own distribution, a JetBrains product line, and reach
   across whichever agent a team already uses. Cursor Projects adds a fourth, shipped
   inside an editor with its own user base. Entering any of these races confines tsk
   to Navigation, the one dimension all four already occupy.
3. **tsk as a product differentiated by the full four-dimension model: open.** This is
   the central bet, and what the (separately scoped, not yet run) token-saving experiment
   exists to test. Whether Product, Delta, and Scale add value an agent or buyer will
   reward is unproven. None of the four systems models them, so all four sharpen the
   experiment rather than settling it. The product decision waits on that experiment
   rather than being made now.

Caveat in tsk's favour: beads' star count likely overstates independent, load-bearing
adoption, since Gas Town/City is largely beads' own primary consumer. "Beads has won"
overstates the case. The category is also visibly unsettled elsewhere (Linear's "issue
tracking is dead" framing, the Block convergence). The product path is not closed, it is
not one to walk through on the Navigation axis.

## Symbiosis vs competition

**Beads: technically symbiotic, not a partner to build strategy on.** The layers are
compatible: tsk could use beads as its Navigation substrate, or as the baseline it
measures against. But beads is an expanding project with momentum and an evangelist, and
expanding incumbents tend to absorb adjacent value rather than leave room for a symbiote.
Treat beads as a swappable substrate and a research baseline, not an ally.

**Claude Code Projects: a dependency, not a symbiosis.** The swappable-substrate escape
that applies to beads does not apply here. tsk's harness runs inside Claude Code, so the
platform is not a layer tsk chooses. What the platform ships natively reduces what tsk
needs to build, and also reduces what tsk can differentiate on within Navigation. Assume
it keeps expanding along that axis.

**JetBrains Air: a dependency only if adopted, unlike Claude Code Projects.** tsk's
harness does not run inside Air the way it runs inside Claude Code, so Air is closer to
beads' position than to Claude Code Projects': a product tsk could measure against, not
one tsk is built on top of. Its open worker layer, ACP, is worth tracking regardless of
whether tsk ever adopts Air itself: if ACP becomes a common way to address an agent
across products, it is a candidate answer to the untested half of tsk's own substrate
question, what a tsk thread's actor looks like when it isn't a Claude Code session.

**Cursor Projects: a dependency only if adopted, like Air.** The coordinator, the cloud
machines and the file sync are Cursor's. tsk's harness does not run inside Cursor, so it
is a product to measure against, not one tsk is built on. Its documentation does not say
whether a project can run another vendor's agents, so it adds no evidence on the
untested half of tsk's substrate question.

tsk's thesis is the unification of all four dimensions, not any single one, so it does
not collapse if any of these systems later absorbs another dimension.

## Net position

Do not stop tsk. Do not ship tsk as a head-to-head tracker or orchestrator. Do not plan
around beads' goodwill, and do not plan around the platform leaving a gap. Run the
token-saving experiment to de-risk the four-dimension claim, keep the research track
moving regardless of the product outcome, and let the experiment's data decide the
product question.

One concrete consequence for work already in flight. M-BOOT-03's T-07 defines the run
loop, and that task splits in two, with only one half at risk.

The plumbing, implement, run tests, push a branch and open a pull request, is close to
what a Projects thread does natively. Check before building it.

The continuation is not provided, and it is M-BOOT-03's own objective line: "a second
unattended run resumes the first run's thread from that state rather than starting the
mission over." Nothing described in Projects does that. The coordinator reaches the same
end by starting a fresh thread on a fresh piece, which is starting over by construction
rather than resuming. That half is tsk's actual contribution to the mission and stands.

A second consequence, not yet actioned anywhere: substrate independence is tsk's stated
premise and an untested one. Worth a mission at some point that runs a tsk thread with a
non-Claude-Code actor, to find out whether the domain model actually holds that
abstraction or only claims to.

## Jev: a related component, not a competing system

Research: [typesafe-jev-classifier.md](../typesafe-jev-classifier.md).

Jev, from TypeSafe AI, is not a coordinator-and-threads system, so it does not belong
in the systems-assessed list above. It intersects tsk on a narrower point: the
evaluator mechanism behind a verification loop.

`docs/kb/agent-context-self-regulation-and-unattended-handoff.md` records `/goal`'s own
evaluator as "a small, separate model (Haiku by default on the Claude API)" that reads
a condition against the transcript and returns not-yet-met, met, or impossible. That is
an LLM-as-judge pattern: a small model generating a verdict as text. Jev-as-a-Judge, also
covered in `typesafe-jev-classifier.md`, answers the same kind of question, whether a
condition holds against a given state, through a typed classifier instead: `Choice`,
`Score` or `Noul`, with a probability and confidence, not generated text. Same job,
different mechanism.

Not a component decision. `/goal`'s evaluator is Claude Code's own mechanism, not one
tsk built or could swap independently. Where this becomes live for tsk is if tsk ever
needs a verification-loop or condition-evaluation mechanism of its own, rather than
depending on `/goal`, at which point a typed classifier is one candidate shape for that
evaluator, worth weighing against a small LLM judge on the axes
`typesafe-jev-classifier.md` reports: consistency, cost, and latency.

## LangSmith: a related layer, not a competing system

Research: [langsmith.md](langsmith.md). Announced 2026-09-24.

LangSmith is LangChain's platform for tracing, evaluating and improving agents. It is not
a coordinator-and-threads system, so it sits beside Jev and not in the systems-assessed
list. It observes agents that run elsewhere. Three items announced together intersect
tsk:

- **Trajectories**: a chronological view of an agent session, with each message once,
  across the main agent and its subagents. It is "a projection over the traces in a
  thread". Evaluators score it, experts annotate it in queues, and a good one is saved to
  a dataset for fine-tuning.
- **Engine v2**: reads production traces, groups them into an issue with a root cause,
  proposes a prompt or code fix, reproduces the failure and tests the fix before a person
  sees it, and opens a pull request on one click. It reopens an issue that recurs.
- **Fine-Tuning**: supervised fine-tuning of an open model on curated trajectories.

Three points of contact:

1. **A trajectory and a thread continuation are different records.** A trajectory is
   derived from traces after the fact, and serves scoring and training. A continuation is
   written at a pause for the next actor, and serves resuming. Neither replaces the other.
2. **Engine covers stages tsk does not model, for one kind of signal.** The
   [positive-friction note](../ai-and-the-loss-of-positive-friction.md) records that tsk has
   no object for signals or insights. An Engine issue is an insight object linked to the
   traces behind it, which is the shape of that note's remedy 1. The signals are an
   agent's production behaviour. Customer signals stay a gap in tsk.
3. **The word "thread" collides.** In LangSmith a thread is linked traces from a
   multi-turn session. In tsk a [Thread](../../domain/ubiquitous-language.md#thread) is the
   execution sequence, with its own identity, pausable and resumable.

Position: no change to the three-part verdict below. LangSmith models no mission, no
coordination, no Product, no Delta and no Scale. It is not a substrate for tsk. Engine
documents support for Deep Agents, LangChain and LangGraph agents only, so it adds no
evidence on the untested half of tsk's substrate question. Whether tsk needs a
per-session record of the kind a trajectory provides is not decided.

## OpenAI Dots: a related system, not a competing one

Research: [openai-dots.md](openai-dots.md). Announced 2026-09-29.

A dot is an always-on agent in ChatGPT with its own cloud computer. The owner hands it a
goal and it runs in the background. The owner can message it in ChatGPT, Slack and Teams,
or call it. It is not a coordinator-and-threads system: it is one persistent agent per
owner, so it is listed beside LangSmith, not among the systems assessed. It is closer to
tsk's collaboration between humans and agents than the orchestrators are. Three items
intersect tsk:

- **Custom Rules and auto-review**: the owner sets which actions a dot takes alone, which
  need approval, and which it must not take. A check runs before an action that could
  touch an account or share information.
- **Specialist dots**: previewed. An employer provisions each with its own identity,
  credentials and tools for a single defined job.
- **Context across channels**: context follows a dot between ChatGPT and Slack or Teams,
  and questions and progress updates go back to the owner.

Three points of contact:

1. **Authority per action class is shipped.** The
   [Post](../../domain/ubiquitous-language.md#post) placeholder holds authority and
   standing instructions, and the
   [energy-constraints reference](underlying-energy-constraints-of-running-a-factory.md)
   lists escalation and a comms medium as conditions for humans to hold posts. Dots
   ships the first of these as owner-set rules, and the third as Slack and Teams.
2. **The authority is attached to the dot.** No source read describes a position that a
   different dot could be appointed to, which is the separation Post makes between a
   position and its holder.
3. **No work model is documented.** No mission, objective, briefing, report or
   continuation record is described. State is the dot's memory and its cloud computer, and
   how either is stored is not described.

Position: no change to the three-part verdict below. Dots models no mission, no
coordination, no Product, no Delta and no Scale, and it runs on one vendor's model and
cloud. It is a data point for Post and escalation, not for Navigation. OpenAI's own pages
were not readable, so this section is based on press coverage.

## Seats (Wheelhouse), and tsk's Actor plus Thread continuation

Source: Yegge's essay
["Seats and Sunsets"](https://yegge.ai/essays/seats-and-sunsets/), on Wheelhouse, his
private harness for Wyvern.

A seat is a role-based position with persistent context, a defined scope of authority, a
history, and accountability. Yegge's framing: a plain session has to derive whether an
action is safe each time it acts; a seat turns that derivation into a lookup. The essay
ties this to Wheelhouse's own cost problem: distrust forces a model into expensive
re-verification, and a seat, by caching that trust, cuts the cost.

Hypothesis: [Actor](../../domain/ubiquitous-language.md#actor) plus
[Thread continuation](../../domain/ubiquitous-language.md#thread-continuation) is the
breakdown of a seat into tsk's terms, one object split into two.

Decision, 2026-09-21: deferred. The bootstrap runs in supervised mode, with one human
holding every position, so the position and the person are not yet separate. The concept
enters the design when agents run autonomously and manage each other. Its tsk name is
settled as [Post](../../domain/ubiquitous-language.md#post), a placeholder in the
ubiquitous language; the doctrine mapping below is the reason for the name.

### What each side holds

tsk's Actor holds identity and cardinality: whoever holds a thread, human or agent
session, with the rule that a human holds many threads while an agent session is bound to
one, and a different session picking up a thread later is a different actor holding it.
Thread continuation holds a snapshot at each pause: the mission briefing link, the task
in progress, a written account of what's next, and a written-by field naming which actor
wrote the entry, forming an audit trail of who touched the thread.

Between them, Actor answers who holds the thread, and Thread continuation answers what
happened on it so far. Neither holds a scope of authority, and neither caches a trust or
permission decision the way a seat does. tsk's model today has no place that stores "this
actor may act on this without re-checking."

### Where the breakdown holds, and where it doesn't

Holds: a seat's history component maps to Thread continuation's append-only store, and
its identity component maps to Actor. Both systems separate who does the work from the
record of what was done, even though tsk splits it into two named parts and Wheelhouse
holds it in one.

Doesn't hold: a seat is tied to a named, standing role, such as the Marshal or the
Seneschal in Wheelhouse's own crew, that persists across many threads and missions. tsk's
Actor is thread-scoped by definition: an agent session is bound to one thread, and picking
up a different thread makes it, by the model's own cardinality rule, a different actor
holding that thread, not the same actor changing seats. tsk has no concept of a role that
outlives a single thread's binding and holds authority across missions. Building that
would mean either loosening Actor's cardinality rule or adding a new part above it, not
just relabelling Thread continuation.

The essay's actual payload, caching a trust decision so it doesn't need re-deriving, has
no counterpart in either tsk object. Bringing that in is a new mechanism, not a rename of
what already exists.

### Where doctrine places it

Doctrine has the concept under another name, and splits it more finely than "seat" does.
Sources and the verification caveat are in
[military-doctrine-sources.md](../military-doctrine-sources.md): the terms the repo already
cites were gathered in earlier sessions, and the ones added here come from the same body of
vocabulary, not from primary text opened inside the sandbox, whose proxy blocks `jcs.mil`.

Already cited in `mission-model.md`, and why each falls short of the seat:

- **Role** and **function** (JP 1, 2013) are organisation-level, "the broad and enduring
  purposes for which the Services and the combatant commands were established". A seat
  is an individual position, one level down. This is why "role" is rejected as the tsk
  name: it collides with doctrine's sense.
- **Standing commitments** and **steady-state** (JDP 0-01) describe enduring work, not the
  position that holds it. This is the standing-mission conversation recorded under mission
  categories in `mission-model.md`: adjacent, not the same thing.
- The **mission essential task list**, held per unit and derived from anticipated
  missions, is the nearest cited term for what a position must be able to do.

Not yet cited in the repo, and the terms that do fit:

- **Post** (UK), or **billet** (US): an authorised position in a unit's structure. It
  exists whether filled or vacant, and a person is appointed to it. This is the seat
  itself.
- **Command** (DoD Dictionary): "the authority that a commander lawfully exercises over
  subordinates by virtue of rank or assignment". Authority is vested by assignment to the
  position, not held by the person. This is the property the essay is built on: the trust
  lookup is a lookup because the position holds the authority.
- **Standing orders** and **standing operating procedures**: instructions attached to a
  position that persist across whoever fills it. Wheelhouse's own phrase for its role
  agents is "role agents with standing orders", so the essay already draws on this
  vocabulary.
- The **table of organisation**: the authorised set of posts, the structure they sit in.
- The **duty log**, or unit journal: the running record kept at a position and handed over
  between incumbents.

Set against that hypothesis, doctrine gives a five-way split:

| Doctrine | Seat component | tsk today |
|---|---|---|
| Post | the standing position | none |
| Authority by assignment | scope of authority | none |
| Standing orders | persistent instructions | partly: a briefing's Constraints, and ksobr |
| Incumbent | who fills it now | Actor |
| Duty log | history, handed over | Thread continuation |

Actor plus Thread continuation is the incumbent plus the duty log. What "seat" bundles in,
and tsk has no term for, is the post with its vested authority and standing orders. In
supervised mode every post has the same incumbent, so nothing yet separates the post from
the person.

Name: **post**, the UK term. Rejected: billet (the US term for the same thing), seat
(Wheelhouse's term), role (the JP 1
collision above).

## Related

- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): the Navigation,
  Delta, Product, and Scale dimensions referenced throughout, and the Actor and Thread
  continuation entries cited in the Seats section above.
- [docs/vision.md](../../vision.md): the four dimensions, and the line separating tsk from
  agent orchestration.
- [typesafe-jev-classifier.md](../typesafe-jev-classifier.md): research on Jev, cited in
  the Jev section above for its intersection with `/goal`'s evaluator mechanism.
- [beads-as-backing-store-analysis.md](beads-as-backing-store-analysis.md): a schema-level
  sharpening of the Beads section above, on whether beads could be tsk's official ledger.
- [cursor-projects.md](cursor-projects.md): the full reference for the Cursor Projects
  section above, with a mechanic-by-mechanic comparison against tsk.
- [langsmith.md](langsmith.md): the full reference for the LangSmith section above, with
  a comparison against tsk.
- [openai-dots.md](openai-dots.md): the full reference for the OpenAI Dots section above,
  with a comparison against tsk.
- The token-saving experiment referenced above has not yet been designed or run as of
  this writing; it is not tracked in the M-BOOT mission tree, which is scoped to
  bootstrapping self-hosting rather than to this product decision.
