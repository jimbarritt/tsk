# 7. Event log as source of truth; markdown and the TUI as read-only projections

Date: 2026-09-14

## Status

Accepted.

## Context

Two candidate designs existed for which artefact holds tsk's authoritative state:

- Markdown (`plan.md` and similar) as the source of truth, with tsk indexing it.
- Everything held in tsk's own store, with markdown generated as a convenience.

Markdown is efficient for humans to read: tables, notes, links. It is inefficient to
manipulate: finding an exact line to edit or remove, then rewriting the file, is slow
and error-prone for a program. That cost is compounded when an agent spends tokens
doing the manipulation rather than a human editing by hand.

tsk already has an append-only NDJSON event log as its record of what happened, with
SQLite as a throwaway query cache rebuildable from the log.

## Decision

The event log is the sole source of truth. Every other view, the SQLite cache, the
TUI, and any generated markdown, is a projection of it: read-only, and versioned to
the last event it reflects.

- `plan.md` (or an equivalent generated file) may still exist and be committed, but it
  is a projection, not an input. tsk never reads it back or reconciles it against the
  log.
- The projection is one-way: the event log cannot be reconstructed from the markdown
  alone. Consequently, when the repository commits state, it commits both the event
  log and the generated markdown; the markdown by itself is not sufficient.
- The append-only structure, with events tagged by actor and thread, lets concurrent
  writers interleave in the log without a merge or lock. This also gives multi-user
  and multi-task writes a natural join key (`user:thread_id`).

## Consequences

- No bidirectional sync problem: nothing ever writes markdown back into the event
  log, so there is no reconciliation logic to get wrong.
- Agents never spend tokens on surgical markdown edits; they only append events.
  Humans keep a readable, committed plan file, generated rather than hand-maintained.
- The repository must always commit the event log alongside any generated markdown.
  A markdown-only commit loses information that cannot be recovered from the file
  itself.
- This generalises the SQLite-cache pattern already in place for the daemon: the
  event log is authoritative, and every other artefact, cache, view, or generated
  file, is disposable and rebuildable from it.

## References

Source: 2026-09-14 design conversation, "markdown-as-projection". See
`docs/domain/ubiquitous-language.md` for related terms (Delta, Thread).
