# Analysis: using beads as tsk's backing store

Status: exploratory research, no decision made. 2026-09-20. Updated 2026-10-09 with
Memory Beads, the Beads Protocol and versioning, in
[Update 2026-10-09](#update-2026-10-09-memory-beads-the-beads-protocol-and-versioning).

## The question

If tsk used beads instead of its own custom store, what would that look like, and how
disparate are the two domain models.

## Two stores, not one

"tsk's own custom store" names two different things, and this analysis keeps them
separate rather than answering for an ambiguous target.

**The bootstrap ledger.** The concrete, currently running store: the `tsk/bootstrap`
branch, holding mission briefings and `threads/<slug>/{index.md,
continuation-state.jsonl}` as plain files, edited through a ledger worktree and pushed by
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
- **Worktree sharing**: linked worktrees share one `.beads` workspace. The
  documentation also lists an external `BEADS_DIR` setup and database redirects. The
  redirect mechanism's file format was not read.
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
  hierarchy, and Scale rejects "artificial boundaries such as epic, story, or
  sub-task," treating nesting as continuous. Storing Mission as beads' `epic`
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
`tsk-market-position-analysis.md`: "Treat beads as a swappable substrate and a research
baseline, not an ally."

## Update 2026-10-09: Memory Beads, the Beads Protocol and versioning

Source: the Gas City blog post
[Extending Beads: Memories, Versions and the Wire Protocol](https://blog.gascity.com/posts/extending-beads-memories-versions-and-the-wire-protocol/),
by Donna Box, dated 2026-10-01, credited to Stephanie Jarmak, Jim Wordelman, Chris Sells
and Donna Box. The page was fetched as raw HTML and read as text. Quoted phrases were
matched against that text. The post gives beads more than 27,000 GitHub stars and over
1.5 million release downloads as of 2026-10-01.

Status of the work: a preview on the `integration` branch of `versioned-beads/beads`.
"None of it has landed in upstream Beads yet." The preview "may corrupt your Beads
data". HTTP writes, HTTP history, user-installed Types and cross-Scope References are
still ahead.

Since 2026-09-20 tsk's official ledger has been built as git ledgers, and `tskd` is
retired (ADR 0012). The mapping below uses tsk's domain terms, which did not change.

### What the post proposes

| Proposal | Mechanism |
|---|---|
| Memory Bead | Long-lived knowledge, such as "a project policy, a decision and its reasoning, or something an agent learned while doing the work". It has an identity, a title and a Markdown body. "Unlike an Issue, a Memory doesn't become ready or blocked, and we don't close it". It can be corrected or retired |
| Generic graph | "A Bead is an identified, typed thing with properties. A Link is an identified, typed, directed relationship with properties of its own." An Issue and a Memory are kinds of Bead. A blocking Dependency is a kind of Link. Links have their own identity |
| Types | Each Bead or Link has one Type, identified by a URL. A Type descriptor can supply a JSON Schema for properties and, for a Link, constraints on its endpoints. An open `metadata` record holds data specific to an application |
| Beads Protocol (BDP) | A shared data model and an HTTP interface. A tool reads a Bead, inspects its Type and follows its Links without knowing the database schema |
| Scope | "the owning boundary", with a canonical base URL. Each Bead and Link belongs to one Scope. A Link can cross a Scope boundary, and its far end is carried as a Reference: a URL |
| Versioning | Earlier states are addressable, with attribution. A citation follows the current state by URL, or pins one retained version. A Type can own its outgoing Link Types, so changing an owned Link changes the source Bead's version |
| Guarded writes | A writer names the revision it started from. If another write has landed, the guarded write fails. An unguarded write must record the version it replaced |
| History contract | "an old address must continue to mean the same state", and a store that cannot serve an old state must say so, "never substitute the current version" |
| Agent guidance | Graph initialisation writes instructions for remembering knowledge into `AGENTS.md`, and registers a Claude Code `Stop` hook |

The post's motivating example is the split tsk draws between a story card and a product
capability. A code flow policy stored as an Issue raises the question of what closing
it means. "Updating the code flow policy is work we can finish. We want an Issue for the
change and a Memory for the knowledge it leaves behind, with a Link connecting them."

### How the mapping changes

| tsk concept | 2026-09-20 finding | With the proposals |
|---|---|---|
| Story card and Product capability | No beads equivalent | The Issue and Memory split matches the distinction: work with a lifecycle in days, and a record that persists, is corrected and is retired, never closed. A Memory has no acceptance criteria and no health state |
| Delta and `Delta Gate` | No beads equivalent | "an Issue for the change and a Memory for the knowledge it leaves behind" has the shape of a Delta that updates a capability. Nothing gates the Memory's change on the Issue's outcome, and nothing checks production health |
| History that is never deleted, only superseded | Beads exposes a mutable row, with no append-only read path | A Memory's versions are addressable, with attribution, and an old address keeps its meaning. This covers tsk's rule for a product capability's history. It does not give a paused execution snapshot |
| Thread continuation | No beads equivalent | Unchanged. Versions record states of a Bead, not a paused execution context per actor |
| Territory and Nexus | No beads equivalent | A Scope matches a ledger more than a territory: one store, one writer, one URL. A cross-Scope Link carries a URL to another store. BDP v0 has no index of Scopes, so no nexus, and nothing groups Scopes into a territory. See [Storage and Scope, from the source](#storage-and-scope-from-the-source) |
| Ledger push | Not compared | Guarded writes with a revision token are the same compare and swap that `tsk ledger push` performs |
| Mission briefing, Mission report, Objective | Inert text in an issue's `description` | Typed Beads with a JSON Schema could hold them as structured data that tools validate. This depends on user-installed Types, which are still ahead |
| Actor | No beads equivalent | Unchanged. Versions carry attribution, but nothing models an actor's cardinality |
| Scale | Fixed epic, task and subtask tiers | Unchanged in the post. A generic graph of typed Links could hold nesting without fixed tiers, but the post does not propose it |

### Storage and Scope, from the source

Read 2026-10-09 from two shallow clones: `versioned-beads/beads` on the `integration`
branch at `648db76`, and the BDP specification repo `gastownhall/bdp` at `182f1fc`. The
files named below were read directly.

**Storage is still Dolt.** `docs/architecture/index.md`: "Beads uses **Dolt** as its
sole storage backend", embedded in-process by default (`.beads/embeddeddolt/`) or as a
`dolt sql-server` (`.beads/dolt/`). Every write auto-commits to Dolt history.
`PROPOSAL-pluggable-storage-backends.md` records a SQLite adapter beside Dolt, and
PostgreSQL and MySQL adapters that were rolled back.

- **The graph is more tables in the same database.**
  `internal/storage/graphstore/schema.go` adds seven `graph_preview_*` tables: links,
  scope, types, catalog, payloads, versions and issue versions. Versions hold a full
  snapshot per change, with the actor and a timestamp.
- **HTTP is a front end, not a store.** `bd serve` mounts BDP as a second route table
  on the existing `internal/httpapi` server, over the same Dolt database.
  `engdocs/BDP_GRAPH_ARCHITECTURE.md`: BDP is served "only from SQL-server
  workspaces"; "`bd serve` refuses embedded Dolt permanently".
- **Git is a transport for Dolt, not the store.** A Dolt remote can be DoltHub, S3, GCS,
  a file path, or a git remote. With a `git+ssh://` remote, `bd dolt push` writes Dolt's
  data to `refs/dolt/data` on that git repository, "separate from standard Git refs".
  `bd init` sets the project's git `origin` as the default sync remote. This is a
  custom ref outside `refs/heads/*`, the same kind of ref ADR 0008 moved tsk off,
  because the Claude Code cloud proxy refuses to push one.

**A Scope is one store with one writer.** From `docs/specs/bdp.md`, "Scopes and
identity": a Scope "is the boundary within which BDP interprets local identifiers,
evaluates selections, and commits atomic mutations". Every Bead and Link belongs to
exactly one Scope, each mutation applies to one Scope, and Scopes do not nest. Each
Scope has one canonical URL ending in `/`, and a resource URL is never reused for an
unrelated resource.

- **In the code, one database holds one Scope.** `graph_preview_scope` has a single
  row: the workspace, the Scope URL, an authority ID and a writer token.
  `bd --graph-mode link serve` mints the Scope on its first serve.
- **One writer per Scope.** The plan excludes "independently writable replicas and
  multi-authority merge of one Scope history": a Scope has "One serialized serving
  authority". The authority is a clone-local file, `graph-authority.local.json`, which
  is git-ignored, checked against a hash-chained ledger and, on a shared server, a
  lease row.
- **Cross-Scope Links are URLs, with no index.** A Link may point outside its Scope by
  URI. At least one end must be in the Scope. "A future cross-Scope indexing profile may
  define ownership, lifecycle, authorization, and duplicate handling" for such Links.
  BDP v0 defines no list of Scopes and no way to discover one from another.
- **Access is per request.** An authorization view decides what a caller may read.
  Holding a URL grants no permission.

**Scope compared with territory, nexus and ledger:**

| tsk | BDP | Match |
|---|---|---|
| Ledger: one per repo, written through compare and swap | Scope: one per beads database, one serialised writer, guarded writes | Close. A Scope is the nearer match to a ledger than to a territory |
| Nexus entry: where a repo's ledger lives | The Scope's canonical URL | Close. Both locate one store |
| Nexus: an index of the repos in an area, with links to other nexuses | None in v0. A "future cross-Scope indexing profile" is named | No equivalent yet |
| Territory: the isolation boundary, which governs whether a link may be followed across it | Authorization views per request, and Scope boundaries that only bound identity and atomic writes | No equivalent. Nothing groups Scopes into an area with its own rules |

### Verdict on the update

The overlap with tsk's model has grown, and it now reaches outside Navigation:

- **Product**: Memory Beads give beads a persistent record that is distinct from work,
  which is the core of tsk's Product dimension. The post reaches the split by the same
  argument tsk's ubiquitous language makes against complecting a story card with a
  capability.
- **Delta**: the Issue and Memory pair has the shape of a Delta that updates the product
  record, without a gate.
- **History**: versioning gives beads the "never deleted, only superseded" rule.
- **Structure**: Types with a JSON Schema remove the earlier finding that tsk's concepts
  would ride along as inert text, once user-installed Types land.

What stays outside beads: thread continuation as execution continuity, the actor model,
objectives as checkable states, the mission report, the `Delta Gate` on production
health, and continuous Scale.

The earlier conclusion, beads as "a swappable substrate and a research baseline", gains
weight on the substrate side. If the proposals land upstream, tsk's Product records and
the Navigation layer could be typed Beads in a BDP store, with tsk's own concepts as
Types. That is a question for a later mission, once the preview reaches upstream beads.

### Beads as a substrate for tsk, revisited

Asked 2026-10-09: can tsk still use beads as a substrate and build on top of it?

Technically yes, once the preview lands upstream:

- **Navigation**: IDs, atomic claims and the ready queue map cleanly, as found on
  2026-09-20.
- **Product records**: Memory Beads, with versions whose history is superseded, not
  deleted.
- **tsk's own concepts**: missions, objectives and reports as typed Beads with a JSON
  Schema, not as text in a description.
- **The language boundary**: BDP over HTTP removes the reason `persistence-and-sync.md`
  gave against Dolt, a Go core reached from Rust. tsk would call BDP and never link
  Dolt.

What blocks it today:

- BDP over HTTP serves reads only. Writes, history over HTTP, user-installed Types and
  cross-Scope References are still ahead, and none of it is upstream.
- A Scope has one serialised writer, so every tsk actor would write through one shared
  server. A cloud session reaches that server only over HTTPS.
- Thread continuation, the actor model, the nexus and territories stay in tsk.

### Dolt in a Claude Code cloud session

Probed 2026-10-09 from a cloud session, under that environment's network policy:

| Path | Result |
|---|---|
| Embedded Dolt, the beads default | Works: it runs inside `bd` and writes local files |
| `dolt sql-server` inside the container | Works, and its data ends with the container |
| A remote `dolt sql-server` | Blocked: the MySQL protocol needs raw TCP on port 3306, and outbound raw TCP is blocked. SSH on port 22 is blocked too |
| Sync to a git remote, default ref | Refused. Dolt writes `refs/dolt/data`, a ref outside `refs/heads/*`, and the proxy returns HTTP 403 (ADR 0008). Tested, see below |
| Sync to a git remote, a branch ref | Works. `dolt remote add --ref refs/heads/<branch>` puts the data on an ordinary branch. Tested, see below |
| Sync to DoltHub, S3 or GCS | Reachable over HTTPS. DoltHub's remote API uses gRPC, not tested through the proxy. S3 and GCS need credentials in the environment |

### Experiment: a Dolt ledger in this repo, over git, from a cloud session

Run 2026-10-09 from a Claude Code cloud session, with Dolt 1.88.2 installed from the
GitHub release. The remote was this repository, `git+https://github.com/jimbarritt/tsk.git`,
through the session's git proxy. Each writer was a separate Dolt clone in its own
directory, standing in for two sessions.

| Step | Result |
|---|---|
| `dolt push` with the default ref, `refs/dolt/data` | Failed after 58 seconds: `RPC failed; HTTP 403`. Nothing was written |
| `dolt remote add --ref refs/heads/experiment/dolt-ledger`, then `dolt push` | Succeeded in 9 seconds. The branch `experiment/dolt-ledger` exists on GitHub |
| `dolt clone` from that branch into a fresh directory | Succeeded, and returned the row written by the first clone |
| Second writer pushes while behind | Rejected: "Updates were rejected because the tip of your current branch is behind" |
| `dolt pull`, then push | Merged one writer's status change with the other's new row, and pushed |
| Both writers change the same cell | `CONFLICT (content)`. `dolt_conflicts_missions` lists base `TODO`, ours `BLOCKED`, theirs `DONE`. The merge stops for a person or agent to resolve |

Findings:

- **Dolt over git works from a cloud session** when the data ref is under
  `refs/heads/*`. The `--ref` option on `dolt remote add` is the whole fix. Beads sets its
  own remote, so beads would need the same option exposed.
- **The branch holds opaque files.** The git tree has a `manifest` and content-addressed
  table files. Each push adds a git commit titled `gitblobstore: checkandput manifest`.
  The data is readable only through Dolt, unlike tsk's ledger, whose Markdown and JSONL
  read in any git viewer.
- **Concurrency is compare and swap plus merge.** A stale push is rejected, as with
  `tsk ledger push`. Dolt then merges rows and cells, and stops on a same-cell conflict
  with a queryable conflict table. tsk's ledger merges at file and line level through git.
- **Cost**: one binary of 127 MB, unpacked from a 44 MB archive, and no account,
  server or secret. Measured in the cloud session: 1.1 seconds to download and 1.4
  seconds to unpack. The repo's `SessionStart` hook took 35 seconds to build `tsk` from
  source in the same session. Within one session the container keeps the binary, and
  `/clear` does not reset the container. A new session starts in a fresh container, so a
  permanent install goes in the environment's setup script or in the repo's
  `SessionStart` hook, where `tsk` itself is installed.

The branch `experiment/dolt-ledger` remains on GitHub. The proxy refuses ref deletion
from a cloud session (ADR 0008), so it is deleted by hand.

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
