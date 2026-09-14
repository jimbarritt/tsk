# Ubiquitous language

Every term tsk's design uses, with its meaning, the alternatives rejected, and the
reason.

## Territory

A bounding area. It is where isolation is defined. A mission operates within a
territory. Expected territories: work, personal, Ubiqtek, agentic research. Territories
are independent of GitHub organisations. Whether a link may be followed across a
territory boundary is governed by the territory, not by the nexus.

Rejected: realm, workspace, atlas, estate, province, district, theatre, area of
operations. A territory is a bounded sub-part by definition, with borders and
ownership. Atlas and estate both read as the whole rather than a part. Theatre collides
with the compliance-as-theatre usage in Thinking in Code.

## Nexus

The discovery and routing mechanism. Nothing more. A nexus holds the index of projects
in its area, and links to other nexuses. A nexus does not define a boundary, the
territory does that. Each project repo keeps its own event log under its own data ref;
the nexus records where to find it.

Rejected: base camp and Basecamp (an existing product in the same category), mission
control and operations centre (hold an operational-control sense that does not fit a
passive index), helm, confluence, concourse, depot, relay, hub, registry, directory
(tooling collisions). Sonatype Nexus, Google Nexus and Nexus Mods are an accepted
collision.

## Thread

The execution sequence. A thread holds its own context and can be paused, suspended
and resumed. This is what lets a human or an agent continue a set of tasks from one
session to the next, possibly as a different actor.

Distinguished from: step, a movement made, independent of people and mostly of time,
leaving a trail behind similar to a log. Step is not adopted as a unit in tsk.

## Task

The unit of work. Nestable to any depth. Every task has an identity, an objective and a
status from the start, not a line of text in a table.

## Mission

The way an agent or a human receives instructions. A mission has an objective, which also serves as its
definition of done, and breaks down into tasks. Mission is a first-class domain object,
not only a link to a parent task, and can hold its own metadata.

A task becomes a mission at the point it is delegated: handed to a different actor from
the one holding the parent mission. That is when a briefing is needed, because the
receiving actor does not have the holder's context. Assignment alone is not the test:
every task on an agent's own task list has an assignee (the agent itself), and none of
them is a mission. Most tasks never become missions.

Tasks are scale-free: a task at a higher level becomes a mission for a sub-agent. The
common element across levels is the objective.

Rejected: work package, replaced by mission.

## Mission briefing

The document handed to an agent, rendering a mission for a specific actor. The same
mission renders differently for a human and for a cloud agent.

## Objective

An objective is one of two kinds:
- **Attainable**: a state that is checkable and finishable. The default.
- **Maintained**: a condition held true over time and never done. A mission whose
  objective is maintained looks like a campaign from below. A weak attainable objective
  is often a maintained condition written as if it were a state; check before writing.

## Intelligence (or intel for short)

The general term for input context. It covers earlier candidate terms such as research,
knowledge base, and evidence, each of which may still hold its own specific meaning,
not yet defined. A whole mission can be intelligence gathering, for example a spike or a
research assignment.

## Navigation

One of tsk's four core dimensions (Navigation, Delta, Product, Scale), per the README.

Work moves like a route being walked. The destination is not fully known at the start;
it reveals itself as work moves along it. The route is partly planned, partly
discovered. A route can split into parallel routes, and a route can be abandoned. An
abandoned route produces no delta but produces navigational knowledge: "we tried going
that way" holds value and is not discarded.

Note: the source session for this definition used "territory" as a generic metaphor for
the area of unknown work being navigated. That predates, and differs from, the adopted
[Territory](#territory) term above (a bounding area where isolation is defined). Not
resolved; flagged here so the two are not conflated.

## Delta

One of tsk's four core dimensions (Navigation, Delta, Product, Scale), per the README.

A self-contained, meaningful change to the state of the system. "Atomic" describes a
delta at the smallest scale. A delta is fractal: zoom out, and a cluster of atomic
deltas becomes a single composite delta. Zoom levels, smallest to largest:

| Level | Name | Description |
|---|---|---|
| 1 | Transformation | Smallest compilable change. The system remains valid before and after. |
| 2 | Behavioural delta | A transformation, or a sequence of them, that changes observable system behaviour, proven by a test going from failing to passing. |
| 3 | Atomic deployment delta | A single deployed change to a single service. May be composed of multiple commits. The deployment event is the meaningful boundary. |
| 4 | Composite delta | A logically coherent change spanning multiple services or atomic deployments, where neither is meaningful in isolation. |
| 5+ | Higher-order groupings | Clusters of composite deltas forming releases, milestones, and so on. |

A delta is valid only if it transitions the system from one fully functional state to
another. The system stays fully functional at every point, even mid-composite-delta,
never broken or partial in production, even temporarily. A composite delta in progress
leaves the system in a known intermediate state: incomplete, not broken.

A **structural change** (refactoring) rearranges without changing observable behaviour:
a delta with no external effect. A **behavioural change** moves the system to a new
state, proven by tests.

The plan format also used Delta as the name for a top-level planning group. That usage
has felt incongruous, and it is not adopted in the mission model. Whether it is the same
concept as the dimension above is a possible collision, not resolved.

## Path

The story of how a delta came to exist: commits, PRs, decisions, abandoned routes. The
path structure mirrors git's own model, with intent and narrative attached. A path
nests: the commits inside a delta form its micro-path, and a PR is a small-scale path.
The nesting is consistent across scales.

## Waypoint

A point at which the system is fully functional and could be stopped at. Not just
"code compiles" or "tests pass," but a shippable, working system, even one doing less
than eventually intended.

