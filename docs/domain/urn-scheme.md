# The URN scheme for the domain types

Every domain type of tsk can be named by a URN in the `urn:tsk:` namespace. ADR 0029 set the
first two forms, for a session and a harness instance, and the rules of the scheme. Jim's
direction of 2026-10-30 (M-TSK-ADHOC T-35) extends the scheme to every domain type. This
page is the specification: the rules, the forms, where each is used, and the points still
open. The forms marked *decided* are in an ADR. The others are proposed here and become
decided when an ADR names them.

## Rules

1. The prefix is `urn:tsk:`. The segment after it names the type, in the singular and in
   lower case, with the domain term of the ubiquitous language: `session`, `instance`,
   `actor`, `thread`, `mission`, `ledger`.
2. Each type has a fixed number of segments. A reader splits on that number of colons only,
   so the last segment may itself hold colons.
3. A form never changes once a URN of it is stored or written into a commit. A type that
   needs a different form gets a new type segment.
4. A set of names inside a segment may grow: the harness names of a session, the kinds of
   an instance. A new name adds to the set and changes no stored URN.
5. A URN names one thing for ever and across ledgers. A type whose identifier is unique only
   inside one ledger carries the ledger's own identifier as a segment. A type whose
   identifier is a short number that a publish can change (ADR 0022) is named by a stable
   identifier instead, or has no URN until it has one.

## Forms

| Type | Form | Segments after the type | Status |
|---|---|---|---|
| Session | `urn:tsk:session:<harness>:<id>` | the harness that minted the ID, then the ID | decided, ADR 0029 |
| Harness instance | `urn:tsk:instance:<kind>:<key>` | `cloud` or `worktree`, then the instance key | decided, ADR 0029 |
| Actor | `urn:tsk:actor:<uid>` | the actor's uid | decided, ADR 0030 |
| Ledger | `urn:tsk:ledger:<ledger-id>` | the ledger's own identifier | decided, M-TSK-ADHOC 2026-10-30, built in 0.39.0 |
| Thread | `urn:tsk:thread:<ledger-id>:<thread-id>` | the ledger, then the thread ID | decided, built in 0.39.0 |
| Mission | `urn:tsk:mission:<ledger-id>:<mission-id>` | the ledger, then the internal ID `missions.id`, never the code | decided, built in 0.39.0 |
| Task | `urn:tsk:task:<ledger-id>:<uid>` | the ledger, then the task's uid | decided, built in 0.40.0 |
| Decision | `urn:tsk:decision:<ledger-id>:<uid>` | the ledger, then the decision's uid | decided, built in 0.40.0 |
| Question | `urn:tsk:question:<ledger-id>:<uid>` | the ledger, then the question's uid | decided, built in 0.40.0 |
| Document | none | | not planned |

The harness names in use are `claude-cloud`, `claude-cli`, `copilot-cli` and `pi`
(ADR 0029). The instance key is the cloud session ID, or `<clone-id>/<worktree-name>`.

### Why each form is what it is

- **Session and instance** carry the harness or the kind, because an ID is unique only
  within the harness that minted it, and two harnesses can mint the same string.
- **Actor** carries the uid alone. A uid is a UUID, unique everywhere, and the name is not
  in the URN because a name can change (ADR 0030).
- **Ledger** needs an identifier of its own. The manifest holds `repo_id`, which is a nexus
  entry ID, absent for an in-repo ledger, and unique only inside one nexus. The manifest
  key `ledger_id` (layout 12) is a UUID: minted at creation for a new ledger, and derived
  from the hash of the root Dolt commit at the upgrade for a ledger that has none, so
  every clone of one ledger derives the same value.
- **Thread** carries the ledger, because a thread ID is 8 characters unique only among the
  threads of its ledger.
- **Mission** carries the ledger and the internal ID, because the code a person types is
  renamed by `tsk mission mv` (ADR 0026) and a mission code is unique only within a ledger.
- **Task, decision and question** are numbered per mission, and a publish that merges can
  renumber a clone's own new rows (ADR 0022), so the number is not the name. Each row has a
  `uid` (layout 13): a UUID minted when the row is first written, derived from the mission
  and the number at the upgrade for a row that already existed, so two clones that upgrade
  on their own write the same value. A renumbering keeps the uid, and the URN follows the
  row. A record moved to another mission with `tsk task mv`, `decision mv` or `question mv`
  is a new row and gets a uid of its own: the old URN names a record that no longer exists.
  The column has no unique key, because a moved record is written under its new mission
  before the old row is removed.
- **Document** is named by its path inside a ledger, and `tsk ledger doc mv` updates every
  link that names the old path. A URN would need a uid per document in a record. Not planned.

## Where a URN is used

- A commit trailer: `Agent-Session: <session URN>` (ADR 0029), and the actor trailer of
  M-TSK-ATTRIB, `<actor URN>` in place of a plain name, so a commit still names the right
  actor after a rename.
- `written_by` of a continuation entry: the session URN from 0.39.0 (ADR 0030); the instance
  key or the instance URN before it; a reader accepts all of them and the two older forms.
- A task claim (M-TSK-COMMS T-03): the actor URN and the session URN.
- The session start context: the thread, the session URN, the instance URN and the actors
  (ADR 0030 decision 8).
- The `--json` output of a record prints its URN beside its ID: `tsk actor list`, `tsk
  thread list` and `tsk mission list` from 0.39.0, a task, a decision and a question in `tsk
  mission list <id> --json` from 0.40.0, and `tsk ledger id` prints the ledger's.
- A link from one ledger to a record of another, which a nexus-level view needs, uses the
  thread or mission URN with the ledger in it.

## Reading a URN

A reader takes the type from the second segment and splits the rest on the number of colons
that type has. Unknown type: the URN is not tsk's, and the reader says so. A reader of
`written_by` also accepts `urn:tsk:cloudsession:<id>` and `urn:tsk:worktree:<name>`, which
0.16.0 and earlier wrote, and `urn:tsk:instance:<kind>:<key>`, which 0.38.0 writes. Those
name an instance, not a session, and a reader that needs the session treats them as an
unknown session of a known instance.

## Open points

- **Document URNs.** Not planned. Reopen if a link from outside a ledger needs one.
