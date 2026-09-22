# 10. The ledger stays a branch, not a directory on `main`

Date: 2026-09-22

## Status

Accepted. Refines ADR 0008 and ADR 0009, both of which stand: the ledger is still
`refs/heads/tsk/bootstrap`, checked out at the fixed worktree path ADR 0009 sets. This
ADR decides a different question: branch versus directory, not which branch or where the
checkout lives.

## Context

Raised by Jim, 2026-09-22: an alternative to the branch has come up before, holding the
ledger as a `.tsk/` directory committed directly on `main`, beside the code it tracks,
rather than on a separate branch fetched and pushed through
`ops/local/fetch-bootstrap-ref.sh` and `ops/local/push-bootstrap-ref.sh`.

Three options exist for where the ledger lives, not two:

1. **A separate branch** (`tsk/bootstrap`, the current design, ADR 0008). The ledger's
   own commit history is independent of `main`'s.
2. **A directory on `main`** (`.tsk/`). Every ledger write is a commit on `main`,
   interleaved with the code history it sits beside.
3. **Fully external to the repository**, addressed rather than fetched: a store outside
   any git history the repo owns, reached over a protocol instead of `git fetch`. The
   nexus is the current candidate shape for this, already recorded as its own idea,
   "tsk metadata in the nexus, not the repo",
   `future-missions-tbd.md` on `tsk/bootstrap`, 2026-09-16.

## Decision

Keep the ledger on a separate branch. Reject option 2 for this repo's own use. Leave
option 3 open, not decided here.

**Why not `.tsk/` on `main`.** It is the simpler mechanism: no second branch, no
fetch/push scripts, no fixed worktree. Simplicity is a real cost against it, not a wash.
Against it: it mixes two kinds of commit on one line of history. `main`'s history is
meant to read as the product's development, one commit per meaningful code change. A
mission's own bookkeeping, an idea captured, a continuation state entry appended, a
task's status flipped, is frequent, small, and not development in that sense. Jim's own
words for what this produces: "lots of minor commits of agent chatter." Every one of
those lands in `git log main`, `git blame`, and every PR built off `main`, diluting the
signal a reader of the product's own history is looking for. The branch keeps that
chatter on its own line, where it belongs, and `main`'s history stays a record of the
product changing.

This is not a new argument invented for this decision. It is the same instinct already
in the domain model: `docs/domain/ubiquitous-language.md` under Ledger, "distinguished
from the artefacts: the artefacts are what a mission builds, the ledger is the account
of the building. A change to one leaves the other untouched, and neither appears in the
other's diff." A `.tsk/` directory on `main` breaks that second sentence directly: a
ledger write would appear in the artefact's own diff, on the artefact's own branch,
every time.

**Why option 3 stays open rather than being decided against.** Pulling the ledger fully
outside the repository, addressed over a protocol rather than fetched as a git ref, is a
different kind of move from choosing branch over directory: it changes what "the repo"
even needs to hold, not just where inside it the ledger sits. That is a bigger design
question, already tracked on its own, and not one this ADR settles by deciding branch
over directory. Both the branch and a `.tsk/` directory are "inside this one repository"
answers; the external option is a different category, and remains valuable enough to
keep live rather than fold into either.

## Consequences

- No change to the mechanism in place. `tsk/bootstrap`, ADR 0008 and ADR 0009's
  worktree, and the fetch/push scripts are unaffected.
- `M-BOOT-04`, "the official ledger", inherits this decision as a constraint on its own
  design: the ledger tree layout, manifest format, and push/pull protocol it defines are
  scoped to a separate branch, not a directory on `main`. Whether the *official* ledger
  (as opposed to this bootstrap scaffolding) uses `refs/heads/*` or something else is
  still M-BOOT-04's own question; this ADR settles branch-versus-directory, not the exact
  ref.
- The external-substrate option, "tsk metadata in the nexus, not the repo," is unaffected
  by this decision and remains open future work, tracked where it already was.

## References

Decided with Jim, 2026-09-22. Refines
[ADR 0008](0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md) and
[ADR 0009](0009-bootstrap-worktree-outside-the-git-directory.md). Connects to
`docs/domain/ubiquitous-language.md` under Ledger, and to "tsk metadata in the nexus, not
the repo" on `future-missions-tbd.md`.
