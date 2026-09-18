# tsk viability against the field

Date: 2026-07-01 (beads re-examination), distilled 2026-09-14, widened to cover Claude
Code Projects 2026-09-18.

## Status

Accepted as the current strategic position. Provisional: blocked by the token-saving
experiment referenced below, which had not run as of this writing.

## Context

This document holds tsk's strategic position against others building in the same space.
One section per system assessed, then a single verdict over all of them. It is widened
as new systems appear rather than split, so tsk's position is stated in one place and
cannot drift between documents.

Two systems are assessed: beads, an independent tool, and Claude Code Projects, a
feature of the platform tsk's own harness runs on.

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
- **Not converged**: beads stays inside the [Navigation](../domain/ubiquitous-language.md#navigation)
  dimension, with a discrete epic-to-story-to-sub-task hierarchy via dotted IDs, which
  is exactly the artificial-tier boundary tsk's [Scale](../domain/ubiquitous-language.md#scale)
  dimension exists to dissolve. Beads has no model of
  [Product](../domain/ubiquitous-language.md#product) (the thing being built), no
  first-class [Delta](../domain/ubiquitous-language.md#delta), and no continuous,
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

- **Converged, on the unit that matters**: a thread that holds its own context, pauses
  and resumes, sustains work over days, and subdivides. This is
  [Thread](../domain/ubiquitous-language.md#thread) as tsk defines it, and as
  [vision.md](../vision.md) describes it, "fractal, pausable, resumable, and able to hold
  their own context, the way a stack frame does in a programming model". Two designs
  reaching the same unit independently is evidence the unit is real.
- **Not converged**: three pairs are fused there that tsk keeps apart.

| Pair | Claude Code Projects | tsk |
|---|---|---|
| Thread and session | one object | bound, not identical, with the reason recorded under [Actor](../domain/ubiquitous-language.md#actor) |
| Inputs and outputs | one library | [Ledger](../domain/ubiquitous-language.md#ledger) and [Artefact](../domain/ubiquitous-language.md#artefact), split by definition |
| Work and its container | a project holds the work | no container; one entity at every zoom |

None of those is tsk missing a part. Each is tsk declining to complect two things the
product ships joined.

**A project is a mission described loosely.** A repo, a goal and configuration is, in
tsk, a territory, an objective and constraints, each with its own definition. tsk also
refused a container above mission once already, on stronger grounds than this:
[domain-model-overview.md](../domain/domain-model-overview.md) records that no campaign
or major-operation type is added "even though doctrine nests campaign above mission:
Scale, one of tsk's four dimensions, rejects fixed echelons". Doctrine offered a
container type with an argument behind it and tsk declined. Missions are scale free, so
no project type is needed.

**A coordinator is an actor.** The [Actor](../domain/ubiquitous-language.md#actor) entry
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
[vision.md](../vision.md) already uses to separate tsk from beads: tsk "is about
collaboration between humans and agents, and between humans and humans, not agent
orchestration alone."

## Three-part viability verdict

1. **tsk as a research programme: yes.** Two independent designs have now converged on
   parts of tsk's model: beads on the architecture and the plan-replacement positioning,
   Claude Code Projects on Thread as the unit that holds context and persists. Both
   validate parts of tsk's underlying claim. Neither says anything about whether adding
   Product, Delta, and continuous Scale produces further measurable value. Both are
   credible baselines to measure the other three dimensions against, and this track
   continues regardless of the product outcome.
2. **tsk as a head-to-head agent issue tracker or orchestrator: no.** Against beads that
   category has an incumbent with distribution, maturity, an evangelist, and most of
   tsk's architecture. Against Claude Code Projects it is worse: the incumbent is the
   platform tsk runs on, shipping orchestration as a native feature. Entering either
   race confines tsk to Navigation, the one dimension both already occupy.
3. **tsk as a product differentiated by the full four-dimension model: open.** This is
   the central bet, and what the (separately scoped, not yet run) token-saving experiment
   exists to test. Whether Product, Delta, and Scale add value an agent or buyer will
   reward is unproven. Neither system models them, so both sharpen the experiment rather
   than settling it. The product decision waits on that experiment rather than being made
   now.

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

tsk's thesis is the unification of all four dimensions, not any single one, so it does
not collapse if either system later absorbs another dimension.

## Net position

Do not stop tsk. Do not ship tsk as a head-to-head tracker or orchestrator. Do not plan
around beads' goodwill, and do not plan around the platform leaving a gap. Run the
token-saving experiment to de-risk the four-dimension claim, keep the research track
moving regardless of the product outcome, and let the experiment's data decide the
product question.

One concrete consequence for work already in flight: M-BOOT-03's objective is one
unattended run producing a pull request and a run record, and its T-07 defines the run
loop. A Claude Code Projects thread already opens pull requests and runs tests on its own
branch. Check what the platform provides before building that loop.

## Related

- [docs/domain/ubiquitous-language.md](../domain/ubiquitous-language.md): the Navigation,
  Delta, Product, and Scale dimensions referenced throughout.
- [docs/vision.md](../vision.md): the four dimensions, and the line separating tsk from
  agent orchestration.
- The token-saving experiment referenced above has not yet been designed or run as of
  this writing; it is not tracked in the M-BOOT mission tree, which is scoped to
  bootstrapping self-hosting rather than to this product decision.
