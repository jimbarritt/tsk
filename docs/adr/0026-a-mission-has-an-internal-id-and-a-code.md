# 26. A mission has an internal ID and a code

Date: 2026-10-24

## Status

Accepted. Decided by Jim on 2026-10-24 (M-ADMIN-02 T-25): a mission gets an internal ID
now, because renames could happen often. The form below replaces the first design, which
keyed every table by a new `uid`. Built in 0.30.0. ADR 0022 (numbers per mission) stands.

## Context

Every table named a mission by its ID, the text a person types, such as `M-NAMES`. A
rename was then a change of the primary key of the mission and of a column in ten more
tables, and in the threads. Jim wants renames to be cheap and safe, for example to put a
repo prefix on every mission ID.

The first design gave each mission a `uid` as its new primary key, and made every other
table name the mission by it. A test with real Dolt found that this cannot be published:

- Dolt does not merge a table whose primary key changed. A clone that upgrades, with the
  tables rebuilt, cannot merge with origin when origin changed the same way, or when the
  clone has unpushed changes. The merge stops with `cannot merge because table missions
  has different primary keys in its common ancestor`.
- Every clone with an unpushed change would meet that error once, after the release, and
  an agent in auto mode cannot clear it (`tsk ledger discard --yes` is refused).

## Decision

1. **`missions.id` stays the primary key and becomes the internal ID.** Every other table
   keeps naming the mission by it, and no key changes. For a mission that exists at the
   upgrade it is the first ID. A mission made later gets a random UUID.
2. **`missions.code` is the mission ID a person types and sees.** It is a new nullable,
   unique column. The upgrade adds it and sets it to the `id`. A `code` that is null reads
   as the `id`, so a row that an older `tsk` wrote after the upgrade still reads.
3. **A rename changes `code` and nothing else.** A task, a block, a parent link and a thread
   follow because they hold the internal ID. A clone that adds a task to the mission while
   another renames it merges with no conflict.
4. **The upgrade only adds a column and an index.** Two clones that each upgrade write the
   same change and Dolt merges them. A clone with unpushed changes merges with an
   upgraded origin.
5. **Commands take and print the code.** `tsk mission list --json` gains `uid`, the
   internal ID.
6. **Continuation entries keep the ID they were written with.** A resume reads the mission
   of the thread, which follows a rename.
7. **`tsk mission mv`, `tsk task mv`, `tsk decision mv`, `tsk question mv` and `tsk ledger
   doc mv`** rename and move, and print one line per change.

## Consequences

- A mission made before the upgrade has an internal ID that is its first ID, not a UUID.
  A later rename leaves the first ID in `id`. Nothing shows it except `uid` in the JSON.
- A new mission can take an ID that a rename freed, because its internal ID is its own.
- A rename made in the same publish window as the upgrade of another clone conflicts with
  it on the cell `missions.code`. A rename after the upgrade is published does not.
- A text that names an old ID (a task, a decision, a document) is not rewritten. `tsk
  mission mv` prints each record that mentions it.
- The new mission ID has to pass the rule of `tsk mission create` (`M-` and capitals and
  digits in groups). A different ID scheme needs that rule changed first (M-ADMIN-02 T-28).
