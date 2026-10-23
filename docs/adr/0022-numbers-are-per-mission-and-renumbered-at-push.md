# 22. Numbers are per mission and renumbered at push

Date: 2026-10-23

## Status

Accepted. Decided by Jim on 2026-10-18 (M-TIDY, the review's questions). Built in 0.19.0
(M-TIDY T-09). Extracted from the decision records in M-TIDY T-11. ADR 0017 (the ledger
is a Dolt file remote carried by plain git) stands: the renumbering runs inside its
publish.

## Context

People and agents refer to work by short numbers: `T-03` in `M-PERF`, `Q-02` in
`M-NAMES`. A clone mints the next number for its own new rows without asking origin,
because a write never waits for origin (ADR 0019). Two clones that each add a task to the
same mission before either publishes mint the same number. Before 0.19.0 the publish then
stopped with a merge conflict, and a person had to resolve it by hand.

Jim wanted the short numbers kept, because they are what people say across sessions, and
the collision handled by `tsk`.

## Decision

1. **Tasks, questions, objective outcomes, constraints, out-of-scope items and decisions
   are numbered per mission.** Each is a row keyed by the mission and a number. A clone
   mints the next number from the mission's counter.
2. **A publish that has to merge renumbers the publishing clone's own new rows that
   collide** with rows origin already holds, to the next free numbers. Rows the clone did
   not add never move.
3. **References follow.** Task blocks and the task a decision names move with the
   renumbered row. The mission counters are raised past the highest number on either
   side.
4. **The publish prints one line per change**, for example `T-03 is now T-05 in M-X`,
   before the commit hash. Free text that names an old number does not change, so the
   printed lines are the record.
5. **A real conflict still stops the publish.** Two clones that change the same field of
   the same row conflict. The renumbering is undone first, so the local rows keep their
   numbers, and the command exits 1 and names the tables.
6. **A number freed by a removal is not reused.**

## Alternatives not taken

- **Globally unique identifiers** (a UUID or a hash). They never collide, but nobody can
  say them in a conversation or type them.
- **A per-clone prefix** in the number. It avoids the collision but makes every number
  longer and ties a task to the clone that made it.
- **Asking origin for the next number at write time.** It makes every write wait for the
  network, against ADR 0019.

## Consequences

- Two actors who work the same mission at once can both add tasks and publish. The
  second publish succeeds and says which of its numbers changed.
- A number a person saw before the publish can change. The printed lines say to what.
- Conflicts left are the ones that need a person: two edits to the same field.
