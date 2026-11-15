# 34. One writer at a time in a clone

Date: 2026-11-15

## Status

Accepted. Built in 0.69.0 for M-TSK-LOCAL T-07. ADR 0020 (the push lock) and ADR 0022
(numbers are renumbered at push) remain in force. This ADR extends the push lock of ADR
0020 to every command that writes.

## Context

The test of M-TSK-LOCAL T-06 ran ten writers at once against a local bare nexus, five in
each of two clones. Each ran `tsk task add`, then `tsk ledger push`. It found three faults
with tsk 0.68.1, and the same test found them again with 0.68.3:

- Up to 6 of 10 `task add` commands failed with the Dolt error `serialization failure:
  this transaction conflicts with a committed transaction from another client`.
- A `task add` exited 0, and its task was in neither clone afterwards.
- A push stopped on a conflict.

The causes, read in the code and confirmed by the test:

1. **A write saves from a stale read.** A record command loads every mission, changes one
   in memory, and saves it in one transaction (`mission::change`). The transaction starts
   at the save, after the load. Two `task add` commands in one clone read the same
   `next_task`, and both write the same task number. When the two transactions overlap,
   Dolt rejects the second with the serialization failure. When they do not overlap, the
   second `insert ... on duplicate key update` replaces the first task with no error. That
   is the lost write.
2. **A refresh resets over a write.** A command that refreshes the store reads whether the
   store has a local change, and resets it to origin when it has none and is behind.
   A write that another process makes between that check and the reset is discarded.
3. **A write runs inside a push.** A push commits, renumbers and merges in several steps.
   The push lock of ADR 0020 allows one push at a time, and a write could run between
   the steps of a push.

Clashes between two clones are a separate case. Each clone has its own store, and a push
merges them. ADR 0022 renumbers a task number that both clones took.

## Decision

1. **One lock per ledger store, the clone lock.** It is an `flock` on `push.lock`, the file
   of the push lock of ADR 0020, in the state directory of the clone. A push of an older
   release takes the same file, so the two releases also exclude each other.
2. **A command that can write takes the clone lock at its first access to the store, and
   keeps it until the store is dropped.** It then reads and writes one state that no other
   tsk process changes. Writes in one clone run one after another.
3. **A command that only reads takes no lock at its first access.** The list is in
   `is_read_only_command` in `cli/src/main.rs`: the `list`, `show`, `status`, `path`,
   `title`, `name` and `doc get` commands, `ledger push --background` (whose child takes
   the lock), the tmux commands, and the harness commands that run for a long time. A
   write, a refresh, a discard or a push from one of these commands takes the lock until
   that step ends.
4. **A refresh and a discard take the clone lock.** The check for local changes and the
   reset happen under it.
5. **The lock is re-entrant in a process.** A command that has it can push, and a push
   inside it counts up. A command that has it and starts a background publish passes the
   lock descriptor to the child.
6. **A process waits up to 60 seconds for the lock** (`TSK_PUSH_LOCK_SECS`), then stops
   with an error that names the lock file.
7. **A transaction that Dolt rejects with a serialization failure runs again**, up to 5
   attempts with a random wait of 50 to 250 ms, and prints a note on each retry. The
   transaction applied nothing. Under the clone lock, the failure has one cause, a writer
   outside the lock, such as an older tsk or `dolt sql` run by hand.

## Consequences

- The T-06 test with ten writers in two clones gives 10 of 10 writes, present in both
  clones, with distinct numbers, in each of five runs. Three conformance cases in
  `cli/tests/concurrent.rs` check this: concurrent `task add` in one clone, concurrent
  writers and pushes in two clones against a local bare nexus, and concurrent fetches
  beside writes in one clone. All three fail on 0.68.3.
- A write waits for a push of the same clone. With a local bare nexus a push takes about
  0.15 s. With GitHub a push takes 1.7 to 1.9 s, so a write made during a background
  publish waits up to that long.
- A command on the read-only list does not wait for a push.
- A command that is not on the list and only reads waits for a write or a push. A command
  missing from the list gives the same result, later.
- The lock is per clone, so two clones of one repo on one machine do not wait for each
  other. They merge at push.

## Alternatives

- **Retry the whole read, change and save on a serialization failure, with no lock.** It
  does not catch the lost write: two saves that do not overlap in time produce no error.
- **Read inside the write transaction.** The record commands load through the store trait
  and save through a separate call. One transaction across both needs a change to
  each of about 40 call sites, and has no equivalent on the `dolt sql` fallback path,
  which runs one process for each query.
- **A lock taken by every command, readers included.** It is simpler, and a read would
  then wait for a push, which costs up to 2 s against GitHub.
