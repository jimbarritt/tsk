# Report: renaming missions (M-TSK-ADHOC T-25 and T-31)

Date: 2026-10-25. Robot: Plain Statement Of Fact. Releases 0.27.0 to 0.32.0.

## Summary

Jim asked for commands that rename and move missions, tasks, decisions and questions. The work grew into three changes to the ledger: an internal ID for each mission, a mission prefix for each ledger, and a command that rewrites old IDs in record text. 24 missions were renamed with the prefix `TSK`, and the ledger was published with every record naming the new IDs.

## Timeline

1. T-25 asked for `mv` commands. The first plan renamed a mission by changing its ID in every table that names it.
2. Jim asked whether missions have an internal ID in the database. They did not. Jim decided to add one before renaming became common.
3. Layout 10 added the internal ID (0.30.0).
4. The prefix (ADR 0027) gave each ledger a code such as `TSK` and made `M-<PREFIX>-` the start of every new mission ID (0.31.0).
5. 24 missions were renamed, and the documents and links that named them moved with them.
6. A fault in link handling was found and fixed (0.31.1).
7. Record text that still named old IDs was rewritten by a new command (0.32.0).

## The internal ID

A mission has two identifiers. `missions.id` is the internal ID. Every other table names the mission by it, and it never changes. `missions.code` is the ID a person types. A rename changes one cell in one row. ADR 0026 records the decision.

The first design made a new column the primary key of `missions`. A test against a real Dolt database showed that Dolt cannot merge a table whose primary key changed: `cannot merge because table missions has different primary keys in its common ancestor`. Two clones that upgrade independently would then fail to merge. That design was dropped and its diff saved.

The second design is additive. `code` is a new nullable column with a unique key, and the upgrade copies `id` into `code` for existing rows. Two clones that upgrade independently make the same change, and Dolt merges them. A mission made before layout 10 keeps its first ID as `id`. A mission made after it gets a random UUID. A test covers the two-clone upgrade merge. Another test covers a rename on one clone merged with a task added under the old code on another.

Threads store the internal ID. Each read maps it to the code, so a renamed mission shows its new ID in every thread.

## The prefix

A prefix belongs to the ledger, not to a repository, because it namespaces every mission in that ledger. It is a manifest key of 3 to 4 capitals. `tsk mission create` and `tsk mission mv` add `M-<PREFIX>-` to an ID that lacks it. A trailing numbered group (`-02`) marks a part of a parent mission. The parent has to exist, and it becomes the parent when none is given. A ledger with no prefix has none of these rules. ADR 0027 records the decisions. Task T-32 holds the open decision: a nexus has to refuse a prefix that another ledger in it uses.

## The commands

- `tsk mission mv <old> <new> [--dry-run] [--rewrite]` renames a mission, renames linked documents named for it, follows threads, and prints one line for each change.
- `tsk task mv`, `tsk decision mv` and `tsk question mv` move a record to another mission and keep its links and blocks consistent.
- `tsk ledger doc mv` renames a document and updates every link to it.
- `tsk mission prefix [PREFIX]` prints or sets the prefix.
- `tsk mission rewrite <old> <new> [--dry-run]` replaces an old ID, as a whole word, in the text of every mission.

## Faults found while using the commands

1. **Links of other missions.** `mission mv` updated the links of the mission it renamed. A document that a second mission also linked kept its old path in that mission's link. The push stopped with `mission M-X links the document ..., which is not on the tsk/docs branch`. Two links were repaired by hand. 0.31.1 updates the links of every mission that names a moved path.
2. **A thread with a stale briefing path.** One thread kept a briefing path that named an old mission. `tsk thread set mission` bound it to the right mission.
3. **Old IDs in record text.** A rename changes keys. It does not change free text in decisions, constraints, out-of-scope items, objectives, answered questions, purposes and task descriptions. `tsk` had no command to edit most of these. 0.32.0 adds `mission rewrite`. It runs over the whole ledger, so it also covers answered questions, which a command for one kind of record would not.

## Result

- 27 old IDs were rewritten across the ledger: 24 renamed missions and the three administrative missions (`M-ADMIN-01` is now `M-TSK-IDEA`, `M-ADMIN-02` is `M-TSK-ADHOC`, `M-ADMIN-03` is `M-TSK-VULNS`).
- Three old IDs remain by choice: a generic `M-ADMIN` in a task, a real document path, and `M-BOOT-05`, a mission that was merged and never had a record.
- Tests: 373 unit and 466 integration tests pass.
- ADR text, the user guide, the tsk skill, the ledger layout document and the ubiquitous language name the new IDs.

## What the work showed

- A schema change that alters a primary key cannot merge in Dolt. Test a schema upgrade against two clones before choosing it.
- A rename command needs a plan step that lists every record, document, thread and link it will touch. `--dry-run` printed that list, and the list exposed the missing links before the real run.
- Free text names IDs. A rename needs a second command for text, or the text is out of date after it.
- A test that builds an old layout has to rewrite child rows to the old keys, or it tests a state no ledger has.

## Open follow-ups

- T-32: nexus check for the mission prefix.
- T-28: session title format. `tsk thread title` prints `<mission ID>: <robot name>`, so this task is partly done.
- Release notes of earlier versions, and dated design notes in `docs/arch`, still name old IDs. They describe the ledger as it was.
