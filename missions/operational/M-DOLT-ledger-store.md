# Mission: Dolt as the ledger store

| Field | Value |
|---|---|
| ID | M-DOLT |
| Territory | agentic research |
| Assignee | unassigned |
| Blocked by | none |

Skeleton. The decision is made in ADR 0014. The scope of the first step is open.

## Idea, as captured

Raised by Jim, 2026-10-09, after the Dolt over git experiment:

> I think we should go for Dolt now before we build up too much data, which means we
> have to build it like a proper model. Although Dolt could just be, to start with,
> could just be an index, like replacing our JSONL stuff, I think, right? So we would
> still have, like, for example, missions could still be markdown mission briefings.
> Although I don't know, maybe we need to just go whole hog now and put in Dolt and do
> the projection layer and get that working. Well, actually, no, I don't think we need
> the projection layer to begin with because the only reason we need the projection
> layer is for a human to be able to look at the thing. So I think as long as the
> agents can talk and store things, then it's fine. So let's go Dolt. We need to, next
> mission is to shift to Dolt as the storage layer with a seam, so we could swap it
> back to our raw storage if we wanted to.

Transcribed from speech. "Dalt" and "DOM" in the transcript read as Dolt.

## Objective

- The `tsk` binary reads and writes the ledger through one storage trait. The git-file
  ledger and a Dolt database are both implementations of it, and a configuration
  setting selects one.
- With Dolt selected, every ledger read and write that works today works, from a local
  session and from a cloud session, with the data synced over git on a branch under
  `refs/heads/*`.
- `tsk ledger push` with Dolt selected rejects a stale push, merges rows and cells, and
  reports a same-cell conflict rather than overwriting.
- An agent reads `index.md`, a mission briefing and a continuation entry through `tsk`
  commands, with no file open.
- The `SessionStart` hook installs the `dolt` binary when it is absent.

## Purpose

Parent: TBD. Follows from M-BOOT-04, the official ledger, which built the git-file
ledger this mission puts behind a seam. The reasons are in
`docs/adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md` in the tsk repo.

## Intelligence

- `docs/adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md` in the tsk repo:
  the decision, the six options and the consequences.
- `docs/kb/orchestration-ecosystem/beads-as-backing-store-analysis.md` in the tsk
  repo, "Experiment: a Dolt ledger in this repo, over git, from a cloud session": the
  commands that worked, the `--ref` option, and the conflict table.
- `docs/domain/ledger-layout.md` in the tsk repo: every file in the ledger today and
  its format. The table design starts from it.
- `docs/adr/0007-event-log-as-source-of-truth.md` in the tsk repo: markdown as a
  projection, deferred by ADR 0014.
- `PROPOSAL-pluggable-storage-backends.md` in `gastownhall/beads`: a storage seam in
  Go, with conformance tests across backends, as a precedent.

## Decision authority

Jim decides the scope of the first step (open question 1) and the branch name.

The actor decides the table design, the trait's shape, how the `dolt` binary is
invoked, and the conformance test layout. The mission report records each choice and
the reason for it.

## Constraints

- Dolt runs as an external binary. The `tsk` binary does not link it.
- The Dolt data ref is a branch under `refs/heads/*`. No custom ref.
- The git-file backend keeps passing its tests throughout. The seam is tested by
  running one conformance suite against both backends.
- No projection layer in this mission.

## Out of scope

- A beads backend. The seam allows one later.
- The markdown projection for humans.
- Changing what the ledger records. This mission moves records, it does not redesign
  them.

## Open questions

1. Scope of the first step. Jim's two options, 2026-10-09: (a) everything at once,
   with each briefing's Markdown body in a text column; (b) the JSONL records first
   (continuation entries, external events, bindings, the index table), with briefings
   staying as files until the seam holds both. Recommendation: (a), because (b) means
   two stores and two push paths during the transition, and the briefing body is one
   column.
2. Ordering against M-BOOT-04 T-10 (a ledger in the nexus) and T-11 (the harness
   outside this repo). Both touch the ledger's location.
3. Whether the Dolt data branch is `tsk/ledger` itself or a branch beside it, so that
   the file ledger's history stays readable in git.

## Plan

Not yet drawn up. Drawn up when Jim settles open question 1.
