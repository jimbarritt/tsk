# 21. State is typed records; free text is Markdown documents

Date: 2026-10-23

## Status

Accepted. Decided by Jim on 2026-10-15 to 2026-10-17 in mission M-TSK-SCHEMA, and
confirmed on 2026-10-20 (M-PERF Q-01). Built from 0.14.0 (ledger layout 8). Extracted
from the decision records in M-TIDY T-11. ADR 0014 (Dolt is the store) stands. ADR 0007
(an event log as the source of truth, Markdown as a read-only projection) is replaced for
missions and tasks: the source of truth is the typed records. Its idea of a read-only
projection survives as M-TSK-SCHEMA T-14.

## Context

Before layout 8, the state of the work was Markdown: a briefing per mission and an
`index.md` that listed them. An agent read and rewrote those files by hand to change a
task or record a decision. A change of status meant editing a table in prose, the index
and the briefing could disagree, and no command could answer "which questions are open"
across missions without parsing Markdown.

## Decision

1. **The state of the work is records.** Missions, objective outcomes, constraints,
   out-of-scope items, tasks, task blocks, decisions, open questions and thread state are
   typed rows in the Dolt database. They are written only through `tsk` commands: one
   command noun per record kind (`tsk mission`, `task`, `decision`, `question`,
   `thread`).
2. **Free text is documents.** Intelligence, research, reports and designs are plain
   Markdown files on a second branch (`tsk/docs`), written with `tsk ledger doc put` and
   read with `tsk ledger doc get`. A document never repeats a record.
3. **A mission links to a document** by a record of mission, path and role. The role is
   `intelligence`, `report` or `design`. `tsk` checks that the path exists on the
   documents branch.
4. **The index is a query, not a document.** `tsk mission list` prints every mission.
   `tsk mission list <id> --json` prints one mission in full. There is no `show` command
   and no Markdown output. An agent renders its own summary until projections exist.
5. **Status values are `todo`, `in-progress` and `done`,** for missions and tasks.
   Blocked is derived from the blocks. An administrative mission stays `in-progress`.
   There is no deferred status.
6. **Purpose and decision authority are text fields on the mission.** Constraints and
   out of scope are lists of one-line items, numbered per mission like the objective
   outcomes.
7. **A decision record and an ADR are different things.** A decision is a one-line record
   of a choice. An ADR is a document in the code repository that explains an
   architectural choice. A decision can name an ADR by its path (`decision add --adr`),
   and `decision list --adr` finds the decisions that name one.
8. **Documents stay on their own branch, not in Dolt rows** (M-PERF Q-01). After the
   round-trip work of 0.18.0, the second branch costs nothing on a read and only one push
   when a document changed. Rows would merge a whole document as one cell, so two actors
   who edit one document would conflict where git merges their lines.

## Consequences

- One command answers questions across the whole ledger. `tsk question list` lists every
  open question in every mission.
- An agent changes the state with short commands instead of rewriting prose, and the
  commands check what they write: an unknown mission or task ID is refused.
- People read the state through `tsk` commands or an agent's summary, not by opening a
  file. Projections (M-TSK-SCHEMA T-14) are the planned read-only view.
- Each existing mission was moved to records by an agent through the commands, with no
  parser. A mission that is done is a bare record, with its final briefing as a linked
  document.
