# 23. A tsk older than the ledger updates itself

Date: 2026-10-23

## Status

Accepted. Decided by Jim on 2026-10-18 (M-TSK-TIDY, the review's questions). Built in 0.18.0
(M-TSK-TIDY T-10). Extracted from the decision records in M-TSK-TIDY T-11.

## Context

The ledger records its layout version. A `tsk` newer than the ledger upgrades the ledger
on its first write. An older `tsk` then stops with `unsupported ledger version` and names
`tsk self-update`. Several sessions share one ledger, and a session can be halfway
through its work when another session upgrades the ledger. Before 0.18.0 that session
stopped, and a person had to update `tsk` and run the command again.

Jim ruled that a command that fails because the ledger is newer should warn and do the
upgrade itself.

## Decision

1. **Only the unsupported-layout error starts an update.** When a command stops because
   the ledger layout is newer than this `tsk` supports, `tsk` warns, runs the same update
   as `tsk self-update` (by the method it was installed with: Homebrew, Cargo or a
   release binary), and runs the command again with the new binary.
2. **The hook commands never update.** `tsk thread session-start` and `tsk thread guard`
   run inside hooks, where an install would delay the session or fail without a person to
   see it. They keep the error.
3. **A loop guard stops a second update** for the same command (`TSK_AUTO_UPDATED`). If
   the new binary still cannot read the layout, the command prints the error and one
   extra line, and stops.
4. **It can be switched off.** `TSK_AUTO_UPDATE=never` or `TSK_NO_UPDATE_CHECK=1` keeps
   the old behaviour.
5. **A newer `tsk` upgrades the ledger on a write, never on a read.** Reading a ledger
   never forces other sessions to update.

## Consequences

- A session in the middle of its work survives another session's upgrade of the ledger.
- A binary released before 0.18.0 cannot update itself. Its sessions must be updated by
  hand once.
- A layout change can arrive in a session without a person choosing it. The warning line
  says so, and the release notes name the change.
