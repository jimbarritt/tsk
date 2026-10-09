# Mission: Dolt as the ledger store

| Field | Value |
|---|---|
| ID | M-DOLT |
| Territory | agentic research |
| Assignee | unassigned |
| Blocked by | none |

Skeleton. The decision is made in ADR 0014. The scope and the branch were settled on
2026-10-09, see Decisions. The plan is not yet drawn up.

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

## Decisions

- **Scope: everything at once.** Every ledger record moves to Dolt in one step, with
  each briefing's Markdown body in a text column. Decided by Jim 2026-10-09.
- **The Dolt data branch is `tsk/ledger`.** The same branch the file ledger uses
  today. Decided by Jim 2026-10-09. The file ledger's history stays in that branch's
  earlier commits.
- **M-DOLT runs before M-BOOT-07's remaining clean-up.** "Dolt first, then clean up."
  Decided by Jim 2026-10-09. M-BOOT-07 T-01, migrating sessions, continues as sessions
  are returned to.
- **Projections are decided later.** They may not be committed at all. Decided by Jim
  2026-10-09.

## Decision authority

Jim decided the scope and the branch name, see Decisions.

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

1. Answered 2026-10-09, see Decisions: scope.
2. Withdrawn 2026-10-09: M-BOOT-04 T-10 and T-11 are both DONE, so nothing orders
   against them. A nexus-held ledger (T-10) is a Dolt database on a namespaced branch
   in the nexus, which the `--ref` option covers.
3. Answered 2026-10-09, see Decisions: branch.
4. How the cut-over runs: the file ledger's last commit on `tsk/ledger`, then the first
   Dolt push on the same branch, and whether every clone's ledger worktree re-fetches
   cleanly across that boundary.

## Plan

Not yet drawn up. The actor who starts the mission drafts it from the objective and
the decisions, for Jim's approval.
