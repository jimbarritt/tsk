# Ubiquitous language

Every term tsk's design uses, with its meaning, the alternatives rejected, and the
reason.

The terms form a graph: a mission has tasks, decisions, questions, documents and threads;
a thread has a mission, an actor and continuations; a harness instance registers as an
actor and binds to a thread. The aim is to navigate that graph in tsk from different perspectives (Jim,
2026-10-16). Two terms divide it. A mission is definition of work: it changes over time,
but it represents state, not execution. A thread is temporal: it represents the
execution of work (Jim, 2026-10-16).

## Territory

A grouping over repos or missions, recorded in a [nexus](#nexus). It is local to the
nexus: the same repo or mission can be in several territories, one per nexus that indexes
it. A territory lists its missions. A mission does not record its territory, and a
briefing records none. Expected territories: work, personal, Ubiqtek, agentic research.
Territories are independent of GitHub organisations. Revised 2026-10-16; the analysis is
in `docs/domain/personal-and-team-territories.md`.

Open: whether a territory lists repos, missions, or both, and whether a territory
governs isolation. Access follows two boundaries that exist already: access to a repo,
and access to a nexus repo. Earlier text made the territory the place where isolation is
defined and said a link crosses a territory boundary only if the territory allows it.
That has no mechanics.

Rejected: realm, workspace, atlas, estate, province, district, theatre, area of
operations. A territory is a bounded sub-part by definition, with borders and
ownership. Atlas and estate both read as the whole rather than a part. Theatre collides
with the compliance-as-theatre usage in Thinking in Code.

## Nexus

The discovery and routing mechanism, and the holder of territories. A group of actors
shares a nexus. It is optional: tsk works in a repo with no nexus, and nobody needs a
personal nexus to take part. A nexus contains the index of the repos in its area, its
[territories](#territory), and a related-nexus list (designed, not built). A repo keeps
its own [ledger](#ledger), and the nexus records where to find it. The nexus repo may
have a ledger of its own, for missions that belong to no single repo or cut across
several (decided, not built). Which nexuses an install attaches, and which an interface
shows, is local config. Revised 2026-10-16.

Rejected: base camp and Basecamp (an existing product in the same category), mission
control and operations centre (have an operational-control sense that does not fit a
passive index), helm, confluence, concourse, depot, relay, hub, registry, directory
(tooling collisions). Sonatype Nexus, Google Nexus and Nexus Mods are an accepted
collision.

## Actor

Whoever or whatever has a thread and executes a mission: a human, or a robot. Not the
same as a session: a session is a platform-level execution instance,
ephemeral and defined by the harness (see `docs/kb/session-creation-and-environments.md`
in the tsk repo), while an actor is the tsk-domain party using one.

Cardinality is the distinguishing fact, and it settles whether a thread is about the
actor or about the work. A human actor attends to one line of work at a time and
switches between several. A robot's harness instance is bound to one thread and runs
beside other instances. A different harness instance picking up that thread later is a
different instance that has it; it is the same actor only if it registers under the same
name.

The human case and the robot case differ in who does the scheduling (Jim, 2026-10-16).
In the human case there is one attention: "work on thread 1" pauses the others, and one
thread is active. Jim's earlier use of "thread" named these threads of his attention.
In the robot case there are many actors at once and no single active one, which is why
the ledger has no current mission. The threads of a human's attention are still in the
model, as the open missions and the latest thread continuation on each; the active one
is whichever session the human uses. Where one session switches between
threads, with `/tsk:switch-thread`, the session binding is the attention.

This is also why thread identity cannot be borrowed from a session's own platform
identifier. Confirmed empirically (M-BOOT-02, 2026-09-16): a Claude Code cloud
session's session ID survives `/clear`, but the same command on the CLI produces a new
one. An identifier that is stable on one platform and not another cannot serve as a
thread's identity across both. Thread identity is tsk's own, minted once when the
thread starts, and a session or a [code worktree](#code-worktree) binds to it, not the
other way round.

An actor is a human or a [robot](#robot). An actor has a name of its own choosing and
a kind, and later a role, skills or a focus. An actor is a record of its own (the
`actors` table, from layout 9), so a robot's identity persists outside any
[harness instance](#harness-instance). A harness instance records which actor it is with
`tsk actor register <name> --kind robot|human`; the same command in a new container with
an existing name is that actor returning. Construction is the creation of a new robot: the
`tsk:construct-robot` skill chooses its name and registers it. Construction is the
robot's birth (Jim, 2026-10-24). A human actor's name is the person's name. A
robot gives itself one, in the style of the Culture ships of Iain M. Banks (M-NAMES; the
style is defined in the ledger document
`missions/operational/M-NAMES/in-the-style-of-culture-ships.md`). The name is not a
thread name and not a mission name: an interactive session runs more than one mission
over its life, so the identity sits with the actor (Jim, 2026-10-24). The first registered
robot was the cloud session that built the field, Still Here After The Compaction.

Open: whether two harness instances may be one actor at the same time (the model allows
it, nothing checks it), and the clash rule from M-NAMES Q-02.

Related: operating context (`docs/kb/agent-context-self-regulation-and-unattended-handoff.md`
in the tsk repo) describes how a session started and who or what supervises it:
supervised interactive, unsupervised autonomous, orchestrator spawning workers,
event-triggered. Actor and operating context overlap but answer different questions:
operating context is a property of the session, actor is who has the thread running
in it.

## Robot

The non-human kind of [actor](#actor). Chosen over "agent" (Jim, 2026-10-24) because
Claude Code uses "agent" for the product as a whole and for a subagent spawned by its
Agent tool, which is one session with its own context: smaller than a harness instance,
not larger. A Claude Code subagent is a session, not a robot in tsk's sense, unless it
registers as one. "Robot" has a physical sense in general use, which is accepted. Older
prose in this repo uses "agent" where the term defined here is robot.

## Harness instance

A running harness that executes one or more sessions. Its identifier is the one the
platform gives it: the cloud session ID (`cse_...`), which survives `/clear` and
compaction, or `<clone-id>/<worktree-name>` locally, where the CLI mints a new session ID
at every `/clear`. A harness instance [registers](#actor) as an actor and
[binds](#binding) to a thread; the two are separate records. The `bindings` table's
`actor` column contains this identifier from layout 8; it is a harness instance key, and the
column keeps its name. Chosen over "instance" alone (too general) and "session" (the
platforms do not use it consistently). Jim, 2026-10-24.

## Thread

The execution sequence. A thread is temporal: it represents the execution of work,
where a [mission](#mission) represents the work's state (Jim, 2026-10-16). A thread
has its own context and can be paused, suspended and resumed. This is what lets a human or an agent continue a set of tasks from one
session to the next, possibly as a different actor. See Actor for what "different
actor" means, and why thread identity is tsk's own rather than borrowed from a
session's platform identifier.

Distinguished from: step, a movement made, independent of people and mostly of time,
leaving a trail behind similar to a log. Step is not adopted as a unit in tsk.

Relationship to Mission: loose, not fixed. A thread has at most one mission at a time,
and may have none (decided 2026-10-16, built by M-TSK-SCHEMA T-10): `tsk thread start`
with no argument mints a thread with no mission, and `tsk thread set mission` attaches
or changes one later. Each continuation records the mission current when it was
written, so a thread can cover work across several missions over its life. A mission
can have several threads, at once or over time, and a thread can have several
actors. Tasks never belong to a thread: they belong to a mission, so a thread that needs
ad hoc tasks takes an administrative mission such as M-ADMIN-02. Most threads are one
to one with a mission; the model does not force it.

The four cases: `tsk thread skip` (no thread, nothing persists); a thread without a
mission (continuity, no tasks); a thread on an administrative mission (continuity and
ad hoc tasks); a thread on an operational mission (the full model).

## Thread continuation

A record of a thread's state at the point it paused, one entry in an append only store
that keeps its history. Distinguished from Thread itself: the thread is the identity,
kept across every pause and resume; a continuation is one snapshot within it, added each
time a pause happens.

Each continuation has a fixed schema. It records the mission the thread was on and the
task in progress (checked against the mission's tasks when the mission has a record). It
records the commits the ledger and the code were each at when the thread paused
(`git.ledger.commit`, `git.code.ref` and `git.code.commit`). It ends with a short written
account of what to do next. The commit fields and the record's own
timestamp are set by the `tsk` binary, since it already handles the git side of a pause.
The what's-next account is the one field a human or an agent writes by judgement.

Resuming a thread reads the latest continuation by default. Earlier ones stay in the
store and can be read directly, for example to notice a task stalling across several
pauses.

Named "thread continuation" rather than "continuation" alone because, on the file
ledger, the record lived inside the thread's own directory. Since 2026-10-12 it is a row
of `continuation_entries` in the Dolt ledger. The command that writes one is
`tsk thread pause`, and the M-TSK-SCHEMA design doc calls the same record a pause entry.
One term should win; not decided.

## Post

Placeholder. Named on 2026-09-21, not yet modelled: the design has not called for it,
and the term is recorded now so it is not renamed later.

A standing position that exists whether or not anyone fills it, with a scope of
authority and standing instructions that persist across whoever is appointed to it. An
[actor](#actor) is appointed to a post; the post is not the actor. The UK doctrine term,
chosen over the US "billet".

What it is for: a post caches a trust decision. An actor in a post acts within the post's
authority without re-deriving, each time, whether the action is allowed. Today tsk has no
place that stores that: [Actor](#actor) records who has a thread, and
[Thread continuation](#thread-continuation) records what happened on it, and neither has
a scope of authority. The bootstrap runs in supervised mode with one human appointed to
every post, so the post and the person are not yet separate. The concept is added to the
design when agents run autonomously and manage each other.

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

The store of a repo's own mission and task data: the [records](#record) (missions,
tasks, decisions, questions, threads, thread continuations, bindings) and the
[documents](#document) (intelligence, reports, designs, and the briefings of the file
era). One ledger per repo. It is
data rather than a line of development, which is why it is kept apart from the
[artefacts](#artefact) instead of sitting beside them.

Today the ledger is a Dolt database synced over the branch `refs/heads/tsk/ledger`, with
its documents as Markdown files on a second branch, `refs/heads/tsk/docs` (M-DOLT,
2026-10-12; M-TSK-SCHEMA T-01, 2026-10-15). Before that it was files on the one branch;
that history is on `archive/tsk-ledger-files`. A separate branch, not a directory
committed on `main`, is itself a decision
(`docs/adr/0010-ledger-stays-a-branch-not-a-directory-on-main.md`); a branch rather than
a custom ref is `docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md`,
and a custom ref by default, with branches as the override for a repo used from Claude
Code cloud sessions, is decided and not built (M-TSK-SCHEMA T-07). That is the mechanism
in use, not the definition. `tsk/bootstrap` is retired.

Distinguished from the artefacts: the artefacts are what a mission builds, the ledger is
the account of the building. A change to one leaves the other untouched, and neither
appears in the other's diff.

Supersedes "data ref", which named a mechanism rather than the thing, and which stopped
being accurate the moment ADR 0008 moved the store off a custom git ref and onto a
branch. Renamed everywhere it appeared, including mission M-BOOT-04's title, 2026-09-17.

Rejected: account (the mission briefing template already uses "your account" for the
actor's own report), record (M-BOOT-02's run record uses that word), chronicle (says
nothing ledger does not), data branch and bootstrap branch (name the mechanism, so they
rot when the mechanism changes, exactly as "data ref" did).

## Record

A typed row in the ledger's Dolt database: a mission, task, decision, question, thread,
thread continuation or binding. Written only through `tsk` commands, never by editing a
file. The records are the source of truth for the state of the work.

Core principle, agreed 2026-10-15: structured data is a record, in a typed table that
rejects a wrong value; unstructured text is a [document](#document). Derived facts, such
as blocked, are computed from records and never stored.

## Document

A plain Markdown file on the `tsk/docs` branch, referenced from records by path:
intelligence, a report, a design, or a briefing from the file era. Written with
`tsk ledger doc put`. Free text that nobody needs to query. A mission links to a
document with a role: `intelligence`, `report` or `design`.

## Ledger store

The local Dolt database that contains a clone's copy of the [ledger](#ledger), at
`${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger-db`, with the
documents checked out beside it from `tsk/docs`. One per clone. Every session in that
clone shares it. `tsk ledger fetch` creates and refreshes it, and `tsk ledger push`
commits and pushes from it, documents first. It is a database, not a directory of files:
nothing opens files in it, and an agent touches it only through `tsk` commands.

Supersedes "ledger worktree", the file-era term for the detached linked worktree that
contained the `tsk/ledger` branch at `.../<clone-id>/ledger`
(`docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md` for why it is outside
`.git/`). The `git` backend still uses one. Write "ledger store" in prose, never
"worktree" alone.

Distinguished from a [code worktree](#code-worktree): the ledger store contains the ledger,
a code worktree contains the [artefacts](#artefact).

## Code worktree

A worktree of the managed repo's code, the main one or a linked one. Typically one per
session, so sessions work in parallel without a shared tree.

A session binds to a [thread](#thread) through a row of the ledger's `bindings` table,
keyed on `<clone-id>/<worktree-name>`: `main` for the main worktree, the worktree's name
for a linked one. See [Binding](#binding).

Distinguished from the [ledger store](#ledger-store), which there is one of per clone
and which contains the ledger, not the code.

Usage: "worktree" never appears unqualified in tsk prose. Write "ledger worktree" or
"code worktree". "Worktree" alone is acceptable only where it names the git mechanism
itself, for example `git worktree add` or "a linked worktree" in a description of git,
and in code identifiers. "Clone" means the managed repo's clone, as in `<clone-id>`, and
never the ledger store.

## Binding

The link from a [harness instance](#harness-instance) to the thread it works. A cloud
session's binding is a row of the ledger's `bindings` table, keyed on the session ID that Claude
Code gives it. A code worktree's binding is a row of the same table, keyed on the clone ID
from `.git/tsk-clone-id`, a slash, and the worktree name. A harness instance has one
binding. Several instances can bind to one thread, `tsk thread list` shows them in
`actors` with their registered actor names, and `tsk thread resume` warns when another
instance wrote the latest continuation. A binding is not a registration: `thread stop`
deletes the binding; the registration stays.

Decided 2026-10-16, built by M-TSK-SCHEMA T-11: every binding is a row of one table. The
`tsk-thread-id` marker file that a code worktree used to contain becomes a row the first time
a command resolves it, and nothing writes the file again.

`tsk thread skip` records, locally and never in the ledger, that a session or code
worktree works without a thread. `start` and `resume` clear it.

## Task

The unit of work. A task belongs to a [mission](#mission) and to nothing else: its ID,
`T-nn`, is minted per mission by a counter that only grows, and a thread never owns one
(decided 2026-10-16). Every task has an identity, a description and a status (`todo`,
`in-progress`, `done`) from the start, not a line of text in a table. A task can be
blocked by other tasks of the same mission; blocked is derived from the blocking tasks'
status and never stored. Since 0.10.0 a task is a [record](#record), written only through
`tsk task` commands. Nesting is by mission: a task that outgrows a task becomes a
sub-mission (see Task scope).

## Task scope

How much a task covers, in four discrete steps rather than a continuum. Not fixed when
the task is created: working a task can discover that it needs more than the step it
started at.

1. **Ad hoc.** Asked for directly, in conversation, and done. Never recorded as a task.
2. **Simple.** A short description, nothing more.
3. **Structured.** A full objective: the checkable state the Task entry already
   requires of every task. This step is where that requirement starts to matter in
   practice.
4. **Mission.** Beyond structured, the task is delegated rather than carried further as
   a task. See Mission for the test.

Discovered, not assigned. A task can start at one step and advance to a higher one as work
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

The way an agent or a human receives instructions, and the holder of the work's state.
A mission is definition of work: it changes over time, but it represents state, not
execution; execution is the [thread](#thread) (Jim, 2026-10-16). A mission has an
objective, which also serves as its definition of done, and breaks down into tasks.
Mission is a first-class domain object, not only a link to a parent task, and can have
its own metadata.

Since 0.9.0 a mission is a [record](#record): ID, name, category, status (`todo`,
`in-progress`, `done`; an administrative mission stays `in-progress`), parent, purpose,
decision authority, constraints, out of scope, objective outcomes, blocks on other
missions, and links to documents. A mission is done when its status is set to `done`;
tsk does not check its objectives. There is no current mission: a session's mission is
the one its thread is bound to (Jim, 2026-10-15).

A task becomes a mission at the point it is delegated: handed to a different actor from
the one that has the parent mission. That is when a briefing is needed, because the
receiving actor does not have the owner's context. Assignment alone is not the test:
every task on an agent's own task list has an assignee (the agent itself), and none of
them is a mission. Most tasks never become missions.

Tasks are scale-free: a task at a higher level becomes a mission for a sub-agent. The
common element across levels is the objective.

A mission is one of two categories, a column on the record (the file era filed briefings
under `missions/<category>/`, and documents keep that layout):

- **Operational**: a mission that builds or changes the artefacts.
- **Administrative**: a mission that keeps the work itself in order, such as capturing
  ideas, tidying, or reviewing missions for drift.

The category does not change a mission's structure or how it is executed. An
administrative mission has no end point, which is why its status stays `in-progress`,
and why it is the place for ad hoc tasks (M-ADMIN-02).

Rejected: work package, replaced by mission. Rejected: classifying a mission or its
objective as standing, attainable or maintained.

## Mission briefing

The document handed to an agent, rendering a mission for a specific actor. The same
mission renders differently for a human and for a cloud agent.

In the file era the briefing was the mission. Since M-TSK-SCHEMA the mission is a
record, and a briefing is a rendering of it, a projection, which is not built yet. What
stays a [document](#document) is free text: intelligence, reports and designs, linked
from the mission.

## Mission report

The account that remains after a mission: what was done, what the briefing failed to give the
actor, and what the actor found wrong, including in its own work.

A report contains feedback and nothing else. Feedback is a statement of something found,
written so a reader learns from it without having to act on it. An inconsistency found during the
mission, a defect in its inputs, a judgement the actor had to make to proceed: each
is reported as what happened.

A report never contains an open question. Nothing in it waits on an answer, asks a reader
to decide, or stays live after the mission closes. Where a finding needs a decision or
further work, that work is tracked where work is tracked, as a mission or a task, and the
report states the finding rather than containing the request.

The test: a report states what happened. A reader never owes it a reply.

A report is written once or repeatedly over the mission's life, whichever suits it, and
takes addenda: a later finding is appended rather than replacing what is there.

## Objective

What a mission is for, and how its progress is judged. Every mission has one.

An objective is a fixed end point, or a measure that changes over time. "The three commands
exist and are wired to the hook" is the first. "Reduce our error rate" and "reduce token
consumption" are the second. Both are objectives. Neither is a separate kind of thing,
and a mission does not declare which it has.

An objective may also have a duration, and that is independent of its shape: a measure
over time can still run for a fixed period.

Stored as objective outcomes, one row each with a `met` flag, numbered per mission by a
counter that only grows: `tsk mission objective add`, `done` and `remove`.

## Decision

A record of a choice made on a mission: the text, who decided, the date, and the task it
concerns, if any. Written with `tsk decision add`. A decision has no status; once
recorded it is a fact. Supersedes the Decisions section of a briefing.

## Question

A record of something not yet answered on a mission: the text and the date asked, and
once answered, the answer, who answered and when. Status `open` or `answered`.
`tsk question list` shows the open ones across every mission. Supersedes the Open
questions section of a briefing.

## Plan

The product of planning: a provisional sequence of tasks proposed for meeting a
mission's objective. A plan belongs to a mission and has no existence apart from one.
The mission states the what and the why. The plan states the how. A plan can be
discarded and rewritten without the mission changing, because the objective is
unchanged.

A plan is provisional by definition. US Army doctrine (ADP 5-0) treats planning as
continuous, which makes a plan an interim product, based on the understanding at
one moment and subject to revision. Moltke the Elder states the same point earlier: no
plan of operations extends with any certainty beyond the first encounter with the
enemy's main force.

A plan has more than one writer, by design. The actor executing the mission owns it, and
other actors can edit the same document. Conflicts stay rare by convention rather than
by locking: once a mission is in action, the actor that spawned it does not edit it.

A plan belongs to the actor executing the mission, not to the one who wrote the
briefing. A briefing may include specified tasks, including an essential one,
but those seed the plan rather than fix it: the actor sets the plan, adds the tasks
the work implies, and rewrites it during execution. The handover between the two is expected to
be refined with use.

A plan is the mission's tasks, in the order and with the blocks they have, not a
document of its own. In the file era it was the task table of the briefing (see
`mission-briefing-template.md`); since 0.10.0 it is the task records. A plan that
outgrows a mission is a signal to delegate: break the work out as sub-missions, each
with its own briefing and its own plan. Plans nest as missions nest.

Distinguished from: the mission, which has the objective and therefore the definition
of done. Reaching the end of a plan does not mean the objective is met, and meeting the
objective does not require that the plan was followed.

Not adopted as the name of a file or of a top-level artefact. The prior plan format
(`jimbarritt/dotfiles`, `home/claude/skills/plan-format/PLAN-FORMAT.md`) used "plan" for
the whole tracking document, containing state for every mission at once. That usage is
superseded. Mission status is asked for at whatever level of scale is meant, per
[Scale](#scale), rather than read from one document.

## Planning

The activity that produces a plan. ADP 5-0 defines it as "the art and science of
understanding a situation, envisioning a desired future, and laying out effective ways
of bringing that future about", and places it alongside preparing, executing and
assessing as activities that run continuously rather than as sequential phases.

Planning starts when a mission is received and continues through execution, because the
situation keeps changing. Eisenhower's "plans are worthless, but planning is everything"
places the value in the understanding the activity produces rather than in the document
that remains.

## Intelligence (or intel for short)

The general term for input context. It covers earlier candidate terms such as research,
knowledge base, and evidence, each of which may still have its own specific meaning,
not yet defined. A whole mission can be intelligence gathering, for example a spike or a
research assignment.

## Navigation

One of tsk's four core dimensions (Navigation, Delta, Product, Scale), per the README.

Work progresses like a route being walked. The destination is not fully known at the start;
it becomes clear as work progresses along it. The route is partly planned, partly
discovered. A route can split into parallel routes, and a route can be abandoned. An
abandoned route produces no delta but produces navigational knowledge: "we tried going
that way" has value and is not discarded.

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
| 2 | Behavioural delta | A transformation, or a sequence of them, that changes observable system behaviour. A test going from failing to passing proves it. |
| 3 | Atomic deployment delta | A single deployed change to a single service. May be composed of multiple commits. The deployment event is the meaningful boundary. |
| 4 | Composite delta | A logically coherent change spanning multiple services or atomic deployments, where neither is meaningful in isolation. |
| 5+ | Higher-order groupings | Clusters of composite deltas forming releases, milestones, and so on. |

A delta is valid only if it transitions the system from one fully functional state to
another. The system stays fully functional at every point, even mid-composite-delta,
never broken or partial in production, even temporarily. A composite delta in progress
leaves the system in a known intermediate state: incomplete, not broken.

A **structural change** (refactoring) rearranges without changing observable behaviour:
a delta with no external effect. A **behavioural change** takes the system to a new
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
document. Code and documentation are both artefacts, and the term treats neither as primary.

Deliberately general, and not expressed in the code. An artefact has no identity,
no status field and no lifecycle of its own in tsk today. It is the word for what is
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
mechanism does it. A capability changes from not delivered to delivered when, and only
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

