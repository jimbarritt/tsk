# 27. A mission prefix belongs to the ledger

Date: 2026-10-24

## Status

Accepted in part. Decided by Jim on 2026-10-24 (M-TSK-ADHOC): where the prefix is stored
and who chooses it, and what a trailing number means. The behaviour of `tsk mission
create` is not decided; see Open points. ADR 0026 (a mission has an internal ID and a
code) stands: a rename of every mission to the new form uses `tsk mission mv`.

## Context

A mission ID is typed by the person or agent who creates the mission, such as `M-BOOT-06`
or `M-TSK-SCHEMA`. `tsk` checks only its form (`valid_id` in `cli/src/mission/model.rs`)
and that no mission has it. Nothing generates or numbers an ID, and nothing names the repo
or context the mission belongs to.

Several ledgers hold a mission with the same name, such as `M-ADMIN`. Jim reads missions
from many repos in one mobile session list, and wants the ID alone to say which ledger a
mission is in. The new default form is `M-<prefix>-<name>`, with a prefix of 3 to 4
capital letters.

A trailing number meant two things before this decision: a part of a parent (`M-BOOT-01`,
whose parent is `M-BOOT`) and a separate mission of the same kind (`M-ADMIN-03`, which has
no parent). `tsk` checked neither.

## Decision

1. **The prefix belongs to the ledger, not to the repo.** It namespaces every mission in
   that ledger. A ledger kept in a nexus for several repos has one prefix, not one for
   each repo.
2. **It is stored as the manifest key `mission_prefix`**, a row of the `manifest` table
   beside `version` and `repo_id`. It travels with the ledger, so every clone and every
   robot reads the same value after a fetch, and it does not change when a repo is renamed.
3. **An agent infers it once.** When a ledger has no `mission_prefix`, the agent that needs
   one chooses 3 to 4 capital letters from the name and purpose of the repo, and stores it.
   A person can overwrite it with a command.
4. **A nexus checks it.** When a ledger is registered in a nexus, `tsk` refuses a prefix
   that another ledger in `nexus.json` already has. Only the nexus can see other ledgers, so
   it is the only place a clash can be found.
5. **A trailing number is for a part of a parent, and for nothing else.** The form is
   `M-<prefix>-<NAME>` for a mission and `M-<prefix>-<NAME>-nn` for a part of the mission
   `M-<prefix>-<NAME>`. A separate mission of the same kind gets its own name, so
   `M-ADMIN-03` becomes a name such as `M-TSK-VULNS`, not a number.

## Consequences

- The prefix is data in the ledger, so a change to it is a write that two clones can merge
  as one cell. Two clones that each choose one for a ledger that has none conflict on that
  cell, and the second to publish keeps the first's value.
- A ledger outside a nexus has no check against other ledgers. Two such ledgers can choose
  the same prefix, and the mobile list then shows the clash.
- Existing missions keep their IDs until a person renames them with `tsk mission mv`.
  `mv` renames one mission and does not rename its children.
- `tsk` has to check decision 5. A numbered ID is refused when the mission it is a part of,
  the ID without its trailing number, is not a mission. `tsk mission mv` applies the same
  check. This is built under M-TSK-ADHOC T-31.
- A series such as `M-ADMIN-03`, `M-LAB-01` and `M-LAB-02` is read case by case: `M-LAB-01`
  and `M-LAB-02` are parts of `M-LAB`, and `M-ADMIN-03` is a separate mission that needs
  its own name.
- The rule of `valid_id` still accepts any ID of the form `M-` and groups of capitals and
  digits until the check is built.

## Open points

- Whether `tsk mission create` refuses an ID with no prefix, adds the prefix when it is
  missing, or warns.
