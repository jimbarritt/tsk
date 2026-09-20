# Analysis: using beads as tsk's backing store

Status: exploratory research, no decision made. Source: Jim's research question,
2026-09-20.

## The question

If tsk used beads instead of its own custom store, what would that look like, and how
disparate are the two domain models.

## Two stores, not one

"tsk's own custom store" names two different things, and this analysis keeps them
separate rather than answering for an ambiguous target.

**The bootstrap ledger.** The concrete, currently running store: the `tsk/bootstrap`
branch, holding mission briefings and `threads/<slug>/{index.md,
continuation-state.jsonl}` as plain files, edited through a worktree and pushed by
compare-and-swap scripts (`docs/domain/bootstrap-rationale.md`,
`docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md`). This is
stage-zero scaffolding by design: `bootstrap-rationale.md` states its objective is
reached when tsk hosts its own development, at which point the scaffolding is retired,
not extended.

**The official ledger.** tsk's planned product persistence layer, settled in
`docs/domain/persistence-and-sync.md` and not yet built (mission M-BOOT-04, no briefing
yet): an append-only NDJSON event log per actor as the source of truth, under a custom
git ref, synced by a Rust daemon (`tskd`) built from scratch, using
`--force-with-lease` for compare-and-swap. SQLite is a disposable local query cache,
rebuilt from the log. Dolt was already considered and rejected here, on stated grounds:
modelling tsk's append-only event data as SQL tables changes the settled design, Dolt's
Go core means a separate process or a MySQL-wire connection from Rust, and tsk's
one-log-file-per-actor layout already avoids the write contention Dolt's shared-table
model is built to solve.

The rest of this analysis treats the official ledger as the target, since it is the
store this question bears on: the bootstrap ledger is deliberately temporary and
low-stakes, and `persistence-and-sync.md` already recorded and rejected adopting Dolt
for reasons independent of the domain-model question asked here. Where the bootstrap
ledger's concrete shape is relevant, it is called out by name.

## Beads' domain model

Verified against beads' own documentation and issue tracker, not restated from memory:

- **Issue**: `id` (hash-based, e.g. `bd-a1b2`, collision-free across concurrent agents
  and branches; dot-suffixed, e.g. `bd-a3f8.1.1`, under a parent-child edge), `title`,
  `description`, `type` (`bug`, `feature`, `task`, `epic`, `chore`), `status`,
  `priority` (0 Critical to 4 Backlog), `labels`, `created_at`, `updated_at`.
- **Dependency edges**: `blocks` (hard, the only edge type that affects readiness),
  `parent-child` (epic-to-task-to-subtask hierarchy, no readiness effect),
  `discovered-from` (provenance, no readiness effect), `related` (soft, no readiness
  effect).
- **Ready work**: an issue with no open `blocks` edge. `bd ready` lists it.
- **Atomic claim**: `bd update <id> --claim` sets assignee and in-progress status in one
  step, so two agents cannot both pick up the same issue.
- **Storage**: Dolt, a version-controlled SQL database, embedded in-process by default
  (`.beads/embeddeddolt/`) or run as `dolt sql-server` for concurrent writers
  (`.beads/dolt/`). `.beads/issues.jsonl` is export and interchange only; the
  documentation states it explicitly, "not the source of truth or a backup." The Dolt
  database is authoritative, and sync happens through Dolt's own remotes, not raw git.
- **Persistent memory**: `bd remember`, `bd prime`, layered on top of the same store.
- **Worktree redirect**: `BEADS_DIR`, or a `.beads/redirect` file, points a worktree at
  a canonical database elsewhere, so linked worktrees share one store.
- Already established in `tsk-market-position-analysis.md` and not re-verified here: a
  daemon in single-writer mode over Unix sockets, and an MCP package.

One thing beads does not itself provide: cross-project routing. Gas Town layers its own
`hq-`/`gt-` ID prefixes and a `routes.jsonl` table on top of beads for that, inside one
town. That is Gas Town's addition, not a beads-core mechanism, and this analysis treats
it as out of scope for "beads as a library."

## tsk's domain model, as tsk defines it

Full definitions: `docs/domain/ubiquitous-language.md`. Named here only as the terms
mapped against beads below: Territory, Nexus, Actor, Thread, Thread continuation,
Ledger, Task, Task scope, Mission, Mission briefing, Mission report, Objective, Plan,
Navigation, Delta, Product, Scale, Artefact, Story card, Product capability, Delta Gate,
System health.

## The mapping

### Maps cleanly

- **Stable, collision-free IDs for concurrent agents.** `bootstrap-rationale.md` names
  this as gap 5 in the old plan format tsk replaced: "Tasks have no stable identifier;
  numbering restarts per Delta. Concurrent agents need stable addressing." Beads'
  hash-based ID scheme is built to solve exactly this.
- **Claimed-by-an-agent state.** Gap 4 in the same list: "nothing for
  claimed-by-an-agent." `bd update --claim` is a direct, working answer.
- **Per-task dependencies driving a ready queue.** Gap 3: "Blockers are a plan-level
  field, not a per-task one. A queue needs per-task dependencies to select an unblocked
  item." Beads' `blocks` edge and `bd ready` are exactly this mechanism, already built.

These three are not incidental. They are the three concrete gaps tsk's own design
history already recorded against its predecessor, and beads already ships working
answers to all three. This is the strongest, most concrete point of convergence found in
this analysis.

### Maps partially, and loses something specific

- **Mission vs epic.** Beads' `epic` sits in a fixed three-tier taxonomy
  (epic-task-subtask) reached through dotted IDs. tsk's Mission is defined by
  delegation, the point a task is handed to a different actor, not by depth in a
  hierarchy, and Scale explicitly rejects "artificial tier boundaries such as epic,
  story, or sub-task," treating nesting as continuous. Storing Mission as beads' `epic`
  type would reintroduce the fixed tiers tsk's Scale dimension was built to dissolve.
- **Objective vs status.** A beads issue's `status` is a state on an enum; closing it
  is a judgement call, not a check against a stated condition. tsk's Objective requires
  a checkable state, fixed or a measure over time, by definition. This is the same gap
  `bootstrap-rationale.md` names against the old plan format, gap 1: "a task is a TODO
  marker plus a description, so done is a judgement, not a check." Beads' schema has
  this same shape, not tsk's.
- **Thread continuation's history vs a mutable row.** Beads' issue is a mutable record
  with an `updated_at` timestamp, not an append-only log. tsk's Thread continuation is
  explicitly an append-only store that "keeps its history," so a resume can read an
  earlier entry directly, for example to notice a task stalling across several pauses.
  Whether beads keeps a comparable per-issue audit trail internally is not
  confirmed by the documentation read for this analysis; what is confirmed is that
  nothing in beads' documented schema exposes append-only history as a first-class read
  path the way tsk's continuation store does.

### No beads equivalent at all

- **Actor's cardinality rule.** Beads' `assignee` is a plain reference, atomically
  claimable. It holds no rule distinguishing a human, who holds many threads at once,
  from an agent session, bound to one, and no rule that a different session taking over
  later is a different actor. This would be a layer built above `assignee`, not
  something beads models.
- **Thread and Thread continuation as execution continuity.** Beads tracks what needs
  doing and its dependency graph. It has no concept of a paused execution context, a
  per-actor snapshot taken at a pause, or a written-by chain across pause and resume
  events. This is the sharpest gap found: beads is a work-tracking memory, not a
  session-continuity mechanism, and tsk's Thread is the latter.
- **Territory and Nexus.** Beads is scoped per repository, or per worktree via
  redirect. It has no bounding isolation concept distinct from a cross-repo routing
  index. Gas Town's `routes.jsonl` is the nearest thing, and it is Gas Town's own
  addition, fused to one town's hierarchy, not the separated
  territory-defines-isolation, nexus-only-routes split tsk deliberately keeps
  (`docs/domain/territory-and-nexus.md`, "Why two separate concepts").
- **Mission briefing and Mission report.** No structured equivalent. A beads issue's
  `description` is unstructured text; nothing enforces a report's own rule, that it
  "never carries an open question," the way tsk's Mission report definition does.
- **Product, Delta, Scale, Artefact, Story card, Product capability, Delta Gate, System
  health.** None of these exist in beads' schema, as fields, edge types, or commands.
  This restates, at the concrete schema level, the conclusion
  `tsk-market-position-analysis.md` already reached at the architectural level: beads
  "has no model of Product..., no first-class Delta, and no continuous, fractal Scale."
  Here, there is no field to extend or repurpose for any of them; they are absent by
  construction.
- **Task scope's ad hoc step.** tsk's lightest step, work "asked for directly, in
  conversation, and done," is by definition "never recorded as a task." Every beads
  issue is a persisted, identified record from creation. Beads has no shape lighter
  than a record; tsk deliberately does.

## What adopting beads as the official ledger would require

Take the clean wins as given: stable IDs, atomic claims, and a `blocks`/ready graph
would not need building. tsk's own `persistence-and-sync.md` leaves this open today
("the ledger tree layout," "the manifest format" have no design yet).

Everything else in "no beads equivalent at all" would have to be built as a layer
outside beads' schema, most plausibly encoded into each issue's `description` or
`labels` as unstructured or semi-structured data, since beads has no field for any of
it. That loses the structural guarantee tsk's own model gives each of these concepts
today: a report that cannot hold an open question, an objective that is a checkable
state and not a status enum, a territory boundary independent of routing, a thread that
resumes from a genuine snapshot rather than a mutable row. None of it is stored as data
beads can query, dependency-check, or claim atomically; it is inert text riding along on
a record beads does treat natively.

## Verdict

The domain models are disparate outside Navigation, and this holds at the concrete
schema level, not only the architectural level `tsk-market-position-analysis.md` already
found. Three specific, named tsk gaps map cleanly onto beads mechanisms that already
work: stable IDs, atomic claims, blocks-based readiness. Every other tsk concept
examined here, Actor, Thread, Thread continuation, Territory, Nexus, Objective as a
checkable state, Mission briefing, Mission report, and all of Product, Delta, Scale, has
no field, edge, or command in beads to hold it. Adopting beads as the official ledger
would mean beads takes the Navigation-and-claiming layer cleanly, and everything else
tsk models is built beside it or on top of it, not inside it.

This sharpens, rather than changes, the existing conclusion in
`tsk-market-position-analysis.md`: "beads stays inside Navigation... treat beads as a
swappable substrate and a research baseline, not an ally."

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): the prior,
  architecture-level assessment of beads this analysis sharpens to the schema level.
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): full
  definitions for every tsk term used in the mapping above.
- [docs/domain/persistence-and-sync.md](../../domain/persistence-and-sync.md): the
  official ledger's settled design, and the stated reasons Dolt was rejected for it.
- [docs/domain/bootstrap-rationale.md](../../domain/bootstrap-rationale.md): the three
  gaps against the old plan format that map cleanly onto beads, and the stage-zero
  status of the bootstrap ledger.
- [docs/domain/territory-and-nexus.md](../../domain/territory-and-nexus.md): why
  territory and nexus are kept separate, the point Gas Town's `routes.jsonl` fuses.
