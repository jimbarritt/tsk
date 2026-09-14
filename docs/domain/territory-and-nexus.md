# Territory and nexus

Status: concepts settled; several mechanics open. Source: 2026-09-14 bootstrap design
session. Term definitions are in
[docs/domain/ubiquitous-language.md](ubiquitous-language.md#territory); this file
holds the rationale.

## Why two separate concepts

Territory and nexus were fused in the first version of this design, then separated:
a territory is a bounding area, where isolation is defined; a nexus is only the
discovery and routing mechanism, holding an index of projects and links to other
nexuses. A nexus does not define a boundary; the territory does. Isolation, whether a
link may be followed across a territory boundary, is a territory property, not a
nexus one.

A consequence: a tsk install does not hold a fixed list of roots. It connects to one
nexus, and the wider network becomes available by following links from there. Each
project repo keeps its own event log under its own data ref; the nexus only records
where to find it.

## Why a coordination repo, not a GitHub feature

- GitHub custom properties are restricted to five value types, must be printable
  ASCII without double quotes, are scoped to one organisation, and need organisation
  owner permission to define. Their purpose is governance, not application data.
- GitHub Projects can span repositories but hold their own data model.
- Nothing native provides an arbitrary, writable, cross-organisation metadata store.

## Comparison with Gastown

- Gastown is multi-repo, not a monorepo. Each managed project is a rig, and each rig
  is its own git repository.
- Gastown has a separate town repo for configuration and cross-rig coordination.
- Gastown uses two levels of beads: town-level, with an `hq-` prefix, for cross-rig
  coordination, and rig-level for project work.
- The town holds a `routes.jsonl` routing table from ID prefix to rig. That file is
  the cross-repo index, the nearest existing equivalent to a nexus.
- Wyvern, cited as a Gastown example, is Steve Yegge's game project: one of the
  rigs, not a repo structure in its own right.

## Open

- Where the missions live in the tsk repository once cloud sessions must read them:
  a directory, or a git ref. (Resolved for the bootstrap: a directory,
  `~/.planning/tsk/missions/` initially, moving into the repository before the first
  cloud test, per M-BOOT-02.)
- What the nexus index holds per project.
- Link direction: whether a link from nexus A to nexus B implies B knows about A, or
  links are one-way and a wider view is a traversal from wherever you start.
- How the active territory filter is expressed in the command line interface.

Expected territories, from the bootstrap sequence: work, personal, Ubiqtek, agentic
research. Territories are independent of GitHub organisations.
