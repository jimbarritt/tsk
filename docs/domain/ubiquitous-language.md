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

The discovery and routing mechanism. Nothing more. A nexus holds the index of the repos
in its area, and links to other nexuses. A nexus does not define a boundary, the
territory does that. Each repo keeps its own [ledger](#ledger); the nexus records where
to find it.

Rejected: base camp and Basecamp (an existing product in the same category), mission
control and operations centre (hold an operational-control sense that does not fit a
passive index), helm, confluence, concourse, depot, relay, hub, registry, directory
(tooling collisions). Sonatype Nexus, Google Nexus and Nexus Mods are an accepted
collision.

## Actor

Whoever or whatever holds a thread and executes a mission: a human, or an agent
session. Not the same as a session: a session is a platform-level execution instance,
ephemeral and defined by the harness (see `docs/kb/session-creation-and-environments.md`
in the tsk repo), while an actor is the tsk-domain party using one.

Cardinality is the distinguishing fact, and it settles whether a thread is about the
actor or about the work: a human actor holds many threads and switches between them,
which is what `thread switch-to` already assumes. An agent session, by contrast, is
bound to one thread. A different agent session picking up that thread later is a
different actor holding it, not the same actor switching.

This is also why thread identity cannot be borrowed from a session's own platform
identifier. Confirmed empirically (M-BOOT-02, 2026-09-16): a Claude Code cloud
session's session ID survives `/clear`, but the same command on the CLI produces a new
one. An identifier that is stable on one surface and not another cannot serve as a
thread's identity across both. Thread identity is tsk's own, minted once when the
thread starts, and a session or a worktree binds to it, not the other way round.

Related: operating context (`docs/kb/agent-context-self-regulation-and-unattended-handoff.md`
in the tsk repo) describes how a session started and who or what is supervising it —
supervised interactive, unsupervised autonomous, orchestrator spawning workers,
event-triggered. Actor and operating context overlap but answer different questions:
operating context is a property of the session, actor is who holds the thread running
in it.

## Thread

The execution sequence. A thread holds its own context and can be paused, suspended
and resumed. This is what lets a human or an agent continue a set of tasks from one
session to the next, possibly as a different actor. See Actor for what "different
actor" means, and why thread identity is tsk's own rather than borrowed from a
session's platform identifier.

Distinguished from: step, a movement made, independent of people and mostly of time,
leaving a trail behind similar to a log. Step is not adopted as a unit in tsk.

Relationship to Mission: loose, not fixed. Usually one thread carries the tasks of one
mission, but a thread is scoped to an actor's continuity, not to a mission's, so the two
are not required to line up. An actor's thread can carry tasks across several missions
over its life — a maintenance or coordination actor is the clear case, picking up
whatever needs attention across missions rather than being handed one. Most threads
will still be one-to-one with a mission in practice; the model just doesn't force it.

## Thread continuation

A record of a thread's state at the point it paused, one entry in an append only store
that keeps its history. Distinguished from Thread itself: the thread is the identity,
held across every pause and resume; a continuation is one snapshot within it, added each
time a pause happens.

Each continuation carries a fixed schema: the mission briefing it points at, the task in
progress, the commit `tsk/bootstrap` and `main` were each at when the thread paused, and
a short written account of what to do next. The commit fields and the record's own
timestamp come from a script, since the script already handles the git side of a pause.
The what's-next account is the one field a human or an agent writes by judgement.

Resuming a thread reads the latest continuation by default. Earlier ones stay in the
store and can be read directly, for example to notice a task stalling across several
pauses.

Named "thread continuation" rather than "continuation" alone because the record already
lives inside the thread's own directory, so the qualifier keeps the term consistent with
its location rather than adding one only some readers would need.

## Post

Placeholder. Named on 2026-09-21, not yet modelled: the design has not called for it,
and the term is recorded now so it is not renamed later.

A standing position that exists whether or not anyone fills it, holding a scope of
authority and standing instructions that persist across whoever is appointed to it. An
[actor](#actor) is appointed to a post; the post is not the actor. The UK doctrine term,
chosen over the US "billet".

What it is for: a post caches a trust decision. An actor in a post acts within the post's
authority without re-deriving, each time, whether the action is allowed. Today tsk has no
place that stores that: [Actor](#actor) answers who holds a thread, and
[Thread continuation](#thread-continuation) answers what happened on it, and neither holds
a scope of authority. The bootstrap runs in supervised mode with Jim appointed to every
post, so the post and the person have not yet had a reason to be separated. The concept
comes out in the design when agents run autonomously and manage each other.

Source: Wheelhouse's "seat", from Steve Yegge's essay
["Seats and Sunsets"](https://yegge.ai/essays/seats-and-sunsets/). A seat there is a
role-based position with persistent context, a defined scope of authority, a history, and
accountability. Doctrine splits that bundle five ways, post, authority by assignment,
standing orders, incumbent, and duty log, of which tsk already has the last two as Actor
and Thread continuation. The full mapping, and the doctrine sources, are in
[docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md](../kb/orchestration-ecosystem/tsk-market-position-analysis.md)
under Seats.

Rejected: seat (Wheelhouse's term; does not read intuitively in tsk's context), billet
(the US term for the same position), role (JP 1's sense is organisation-level, "the broad
and enduring purposes for which the Services and the combatant commands were
established", one level above an individual position).

## Ledger

The store of a repo's own mission and task data: missions and their briefings, threads,
thread continuations, and the reports missions leave behind. One ledger per repo. It is
data rather than a line of development, which is why it is kept apart from the
[artefacts](#artefact) instead of sitting beside them.

Today the ledger is the `tsk/bootstrap` branch, checked out at a fixed path outside the
repository (`docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md`,
`docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md`). That is the mechanism
in use, not the definition. M-BOOT-04 and M-BOOT-05 move it, and the term survives the
move.

Distinguished from the artefacts: the artefacts are what a mission builds, the ledger is
the account of the building. A change to one leaves the other untouched, and neither
appears in the other's diff.

Supersedes "data ref", which named a mechanism rather than the thing, and which stopped
being accurate the moment ADR 0008 moved the store off a custom git ref and onto a
branch. Renamed everywhere it appeared, including mission M-BOOT-04's title, 2026-09-17.

Rejected: account (the mission briefing template already uses "your account" for the
actor's own report), record (M-BOOT-02's run record holds that word), chronicle (says
nothing ledger does not), data branch and bootstrap branch (name the mechanism, so they
rot when the mechanism changes, exactly as "data ref" did).

## Task

The unit of work. Nestable to any depth. Every task has an identity, an objective and a
status from the start, not a line of text in a table.

## Task scope

How much a task carries, in four discrete steps rather than a continuum. Not fixed when
the task is created: working a task can discover that it needs more than the step it
started at.

1. **Ad hoc.** Asked for directly, in conversation, and done. Never recorded as a task.
2. **Simple.** A short description, nothing more.
3. **Structured.** A full objective: the checkable state the Task entry already
   requires of every task. This step is where that requirement starts to matter in
   practice.
4. **Mission.** Beyond structured, the task is delegated rather than carried further as
   a task. See Mission for the test.

Discovered, not assigned. A task can start at one step and move to a higher one as work
on it reveals more than was visible when it was scoped. Case in point: M-BOOT-02's T-14
was raised as a short bugfix, then found during its own investigation to need the scope
of a delegated mission, M-BOOT-02-02.

Distinct from [Scale](#scale) despite the similar name: Scale is the continuous zoom
dimension across Navigation, Delta, Product and Scale itself; this is four distinct
steps, not a point on a continuum. Rejected: task tier (tier reads as a ranking of
quality, not of scope).

Open point, not yet resolved: Mission's own test for graduating a task is delegation to
a different actor. This scale's fourth step describes it by duration instead, a task
expected to run more than a few turns. Whether these are one test seen two ways, or
duration is a second, independent trigger for mission-hood, is undecided.

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

A mission is one of two categories, set by which subdirectory of the ledger's
`missions/` it sits in:

- **Operational**: a mission that builds or changes the artefacts.
- **Administrative**: a mission that keeps the work itself in order, such as capturing
  ideas, tidying, or reviewing missions for drift.

The category says where a mission is filed. It does not change a mission's structure or
how it is executed.

Rejected: work package, replaced by mission. Rejected: classifying a mission or its
objective as standing, attainable or maintained.

## Mission briefing

The document handed to an agent, rendering a mission for a specific actor. The same
mission renders differently for a human and for a cloud agent.

## Mission report

The account a mission leaves behind: what was done, what the briefing failed to give the
actor, and what the actor found wrong, including in its own work.

A report carries feedback and nothing else. Feedback is a statement of something found,
written so a reader learns from it without having to act on it. An inconsistency the
mission hit, a defect in its inputs, a judgement the actor had to make to proceed: each
is reported as what happened.

A report never carries an open question. Nothing in it waits on an answer, asks a reader
to decide, or stays live after the mission closes. Where a finding needs a decision or
further work, that work is tracked where work is tracked, as a mission or a task, and the
report states the finding rather than holding the request.

The test: reading a report tells you what happened. It never leaves you owing it a reply.

A report is written once or repeatedly over the mission's life, whichever suits it, and
takes addenda: a later finding is appended rather than replacing what is there.

## Objective

What a mission is for, and how its progress is judged. Every mission has one.

An objective is a fixed end point, or a measure that moves over time. "The three commands
exist and are wired to the hook" is the first. "Reduce our error rate" and "reduce token
consumption" are the second. Both are objectives. Neither is a separate kind of thing,
and a mission does not declare which it carries.

An objective may also carry a duration, and that is independent of its shape: a measure
over time can still run for a fixed period.

## Plan

The product of planning: a provisional sequence of tasks proposed for meeting a
mission's objective. A plan belongs to a mission and has no existence apart from one.
The mission carries the what and the why. The plan carries the how. A plan can be
discarded and rewritten without the mission changing, because the objective is
unchanged.

A plan is provisional by definition. US Army doctrine (ADP 5-0) treats planning as
continuous, which makes a plan an interim product, based on the understanding held at
one moment and subject to revision. Moltke the Elder states the same point earlier: no
plan of operations extends with any certainty beyond the first encounter with the
enemy's main force.

A plan has more than one writer, by design. The actor executing the mission owns it, and
other actors can edit the same document. Conflicts stay rare by convention rather than
by locking: once a mission is in action, the actor that spawned it does not edit it.

A plan belongs to the actor executing the mission, not to the one who wrote the
briefing. A briefing may arrive carrying specified tasks, including an essential one,
but those seed the plan rather than fix it: the actor decides the plan, adds the tasks
it finds implied, and rewrites as it goes. The handover between the two is expected to
be refined with use.

A plan is a section of a mission briefing, not a document of its own. It is the task
table the briefing already carries. See `mission-briefing-template.md`. A plan that
outgrows that table is a signal to delegate: break the work out as sub-missions, each
with its own briefing and its own plan. Plans nest as missions nest.

Distinguished from: the mission, which holds the objective and therefore the definition
of done. Reaching the end of a plan does not mean the objective is met, and meeting the
objective does not require the plan to have been followed.

Not adopted as the name of a file or of a top-level artefact. The prior plan format
(`jimbarritt/dotfiles`, `home/claude/skills/plan-format/PLAN-FORMAT.md`) used "plan" for
the whole tracking document, holding state for every mission at once. That usage is
superseded. Mission status is asked for at whatever level of scale is meant, per
[Scale](#scale), rather than read from one document.

## Planning

The activity that produces a plan. ADP 5-0 defines it as "the art and science of
understanding a situation, envisioning a desired future, and laying out effective ways
of bringing that future about", and holds it alongside preparing, executing and
assessing as activities that run continuously rather than as sequential phases.

Planning starts when a mission is received and continues through execution, because the
situation keeps changing. Eisenhower's "plans are worthless, but planning is everything"
places the value in the understanding the activity produces rather than in the document
it leaves behind.

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

## Product

One of tsk's four core dimensions (Navigation, Delta, Product, Scale), per the README.
Describes the thing being built: not a plan of work, but what the product does or
should do for its users, and the state it is in.

## Artefact

A made thing that a mission builds: a library, a package, a binary, a schema, a
document. Code and documentation are both artefacts, and the term prefers neither.

Held deliberately general, and not expressed in the code. An artefact has no identity,
no status field and no lifecycle of its own in tsk today. It is the word for what sits
in the repo, used when that needs distinguishing from the [ledger](#ledger).

Distinguished from Product, which is a dimension: Product describes what the thing does
for its users and the state it is in, while artefacts are the material that implements
it. Naming the files "the product" would complect the two, which the Story card and
Product capability entries already warn against.

Distinguished from Delta, which is a change to the artefacts, not the artefacts
themselves.

## Story card

An ephemeral planning token, in the Navigation dimension. A placeholder for a
conversation, not a specification. Its job is to name and scope a piece of work small
enough to deliver in a short cycle, and to trigger the right conversations. Once those
conversation happens, the card has done its job. It belongs to the path, not to the
product model, and its being closed is an opinion, not a fact about the product.

## Product capability (or feature)

A persistent description of what the product does, in the Product dimension. It either
exists in the product or it doesn't, though it may evolve. It has acceptance criteria
that define what "healthy" means for it, and a health state. It persists in the
product model for as long as it is relevant, and its history is never deleted, only
superseded.

A story card and a product capability overlap in content, they describe the same
thing, but are not the same kind of thing: one is a planning instrument with a
lifecycle measured in days, the other a product record with a lifecycle measured in
the life of the product. Do not complect the two, nor either with a delta; each lives
in a different dimension.

## `Delta Gate`

The rule that a product capability transitions state only via a delta; no other
mechanism does it. A capability moves from not delivered to delivered when, and only
when, its delta deploys to production and the system is healthy against its
acceptance criteria. A card closing is downstream of that fact, not the cause of it:
"done" is not a status set by hand, it is a consequence of a verified production
state.

## System health

Every capability, subsystem, and the system overall is, at any moment, in one of two
states: **healthy** (functioning as intended, acceptance criteria met in production)
or **unhealthy** (degraded, broken, or not yet delivered). The state is fractal:
overall system health is a composite of component health states, at any zoom level.
Health can regress, healthy back to unhealthy, via a subsequent delta, without any
card being reopened; it is a current, queryable fact, not a historical record of card
closures.

## Scale

One of tsk's four core dimensions (Navigation, Delta, Product, Scale), per the README.
Continuous, not tiered: there are no artificial boundaries such as epic, story, or
sub-task. One entity type nests at whatever level is meaningful, and the zoom level
determines the view rather than the type. A roadmap and a backlog are both views over
the same underlying entities, not separate artefacts.

