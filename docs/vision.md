# Vision: why tsk exists

## Thesis

Software delivery could be more efficient with a unified model of work. Today, many
tools build ad hoc abstractions on top of a ticket, adding human process over the
top. The common wisdom is that a generic model is the only option, and each
organisation must adapt it. tsk's thesis is the opposite: a specific, rigorous model
of work is possible, and building one would let a substantial amount of ad hoc
process collapse.

Two research questions follow from that thesis, both worth industrial-PhD-level
study tied to real project data rather than controlled experiments (which are not
possible in software delivery, since you cannot rerun the same project two ways):

1. Can such a model even be built?
2. If built, would it be useful?

## The four dimensions

tsk's domain model rests on four dimensions, each a facet of any software delivery
effort. Existing tools such as Jira or Linear model parts of these dimensions, but
not all four, and end up too abstract in the wrong direction to give a coherent
picture. Full definitions and rejected alternatives are in
[docs/domain/ubiquitous-language.md](domain/ubiquitous-language.md); this section is
the motivating narrative.

**Navigation.** Where am I, where am I going, where have I been. How tasks, tickets
and stories break down. A mental model for the route through delivery, not just a
list of destinations.

**Delta.** An observable change to the production system. A basis in reality: after a
period of work, what differs between the system yesterday and the system now. Deltas
are fractal, the same concept at the scale of a single commit or a whole release, and
planning work is, at bottom, planning deltas. Stories and tickets are
temporal and ephemeral; the delta is what persists as a change to the product
underneath them.

**Product.** The dimension other tools leave floating: product maps, A/B tests,
metrics, OKRs, all disconnected from the execution side. tsk's claim is that these
belong in the same queryable, well-defined structure as Navigation and Delta, linked
and referenced between views rather than stitched together ad hoc.

**Scale.** Fractal across the other three dimensions, in the way a map zooms from a
whole country down to street level using one consistent model, not a change of type
at each level. No epic-versus-story-versus-sub-task boundary; one entity, viewed at
different zoom levels.

The hypothesis: a conceptual model connecting all four dimensions makes the whole of
software delivery navigable, and unifying them produces benefits beyond what any one
dimension delivers alone.

## Threads

Work is organised into threads: fractal, pausable, resumable, and able to hold their
own context, the way a stack frame does in a programming model. A thread holds tasks
and deltas; a delta may itself span multiple threads. Navigation happens in time (the
history of a project) and in space (the current state of everything a person or an
agent holds in mind at once, the cognitive estate).

## tsk as the implementation

tsk is a reference implementation of this model; and an extension of a simpler
practice, keeping a plan file, into something fully externalised from any one
conversation. Rather than asking an agent "what's the plan?" and waiting for a skill
to reconstruct the answer, a person or an agent talks to tsk directly. tsk can have a
CLI, a TUI, or a web view; any of them can add, move, or complete work
deterministically, without needing a language model's power just to render a list.

## Precedent and differentiation

Beads (by Steve Yegge) is the closest existing tool, but it is scoped to
orchestrating agent work within the Navigation dimension. tsk is broader: it is about
collaboration between humans and agents, and between humans and humans, not agent
orchestration alone. See
[docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md](kb/orchestration-ecosystem/tsk-market-position-analysis.md)
for the detailed comparison and the current strategic position, against beads and
against Claude Code Projects.

Existing tools such as Incident.io, Freshdesk, Port, Notion, and Linear each model a
piece of this space and get combined ad hoc. tsk's bet is that an actual underlying
structure exists to discover and externalise, rather than stitching separate tools
together per project. This would have value even without agents; with agents, the
value compounds, since an agent that models the structure can work through it
directly rather than through a human-shaped interface built for one purpose at a
time.

## Status

This is the research and product thesis behind tsk, not a settled specification.
[docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md](kb/orchestration-ecosystem/tsk-market-position-analysis.md)
records
the current position on whether and how to pursue tsk as a product versus a research
programme.
