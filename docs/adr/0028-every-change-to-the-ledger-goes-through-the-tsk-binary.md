# 28. Every change to the ledger goes through the tsk binary

Date: 2026-10-29

## Status

Accepted. Decided by Jim on 2026-10-29 (mission M-TSK-ADHOC, task T-43). ADR 0011 (logic
lives in the binary), ADR 0014 (Dolt is the ledger store, behind a storage seam) and ADR 0021
(records are written only through `tsk`) stand. This ADR states the rule that they imply as
one rule, with its reason.

## Context

`tsk` is the abstraction over the ledger. Today the ledger is a Dolt database in a ledger
store outside the repository, carried to origin by plain git on two refs: one for the
database and one for the documents (ADR 0017). ADR 0014 put a storage seam in the binary, so
that the store can change. The files backend was removed in 0.16.0 (ADR 0015) and the seam
stayed.

The backend has already changed three times, and each change altered what an agent could
see:

- The ledger moved from Markdown files on a branch to a Dolt database (ADR 0014).
- The refs moved between branches and custom refs (ADR 0016), because the cloud proxy
  refuses a push outside `refs/heads/*`.
- The transport moved from Dolt's git blobstore to a Dolt file remote (ADR 0017).

An agent that reads or writes a ref, a branch, a table file or a Dolt database by hand
depends on one of these layouts. Each such dependency breaks at the next change, and a
`tsk` test cannot see it.

A by-hand command also gives wrong answers without an error. Two refs share the name
`tsk/bootstrap` on `origin`. `git fetch origin tsk/bootstrap` exits 0 and returns the
orphaned ref, frozen at 2026-09-14. A running Dolt server opens the ledger store read only,
so a by-hand `dolt` write fails while the server runs.

The rule already stands as an instruction in `CLAUDE.md` and `README-AGENT.md`. It has no
reason attached, and a skill step can still contradict it.

## Decision

1. **Every change of state to the ledger goes through a `tsk` command.** State is records,
   documents, thread bindings, actors, the manifest and the refs that carry them.
2. **An agent never edits a file in the ledger store or in the documents worktree.** An
   agent writes a record with `tsk mission`, `task`, `decision`, `question`, `thread` or
   `actor`, and a document with `tsk ledger doc put`. An agent can read a document file at
   the path that `tsk ledger doc path` prints. It does not edit that file.
3. **An agent never runs `git` or `dolt` against the ledger store, the ledger refs or the
   ledger branches.** This covers `refs/heads/tsk/ledger`, `refs/heads/tsk/docs`,
   `refs/tsk/ledger`, `refs/tsk/docs`, and every other ref and branch under `tsk/`. It
   covers a read-only command, such as a fetch to compare the tip. It covers a script too:
   a script that needs the ledger calls `tsk`, and does not inline its own `git` or `dolt`
   sequence.
4. **The `tsk` commands that publish and fetch are valid for an agent.** These are
   `tsk ledger fetch`, `tsk ledger push`, `tsk ledger status` and `tsk ledger discard`. The
   publish points of ADR 0019 stand. The rule bans `git` and `dolt` against the ledger. It
   does not ban `tsk`.
5. **A missing command is a defect in `tsk`.** When an agent needs an operation that no
   command gives, it records a task for the command, and it waits or asks Jim. It does not
   go around the binary.
6. **The command line is the contract with the agent.** The commands, their output and their
   exit codes are the contract. The ledger layout, the refs, the transport and the tools that
   run under them are not. A change to any of them changes no skill, hook or agent
   instruction.
7. **The code worktree is not the ledger.** An agent runs `git` in the code worktree as for
   any repository: status, commit, and push to `main`. A skill step that checks the code
   worktree is correct.

## Consequences

- A backend swap, a ref layout change or a new transport is a change inside the binary, with
  its tests. The plugin, the skills and the instructions of an agent stay as they are.
- An operation that today needs a by-hand step needs a `tsk` command first. Each such step
  that is found is a task.
- A skill, a hook or a script of the plugin holds no `git` or `dolt` command against the
  ledger. A review of a plugin change checks this with a search of `plugin/` for both.
- Text for an agent says "ledger store" and names `tsk` commands. It names the backend only
  where a maintainer needs it.
- The rule is held by instruction and review. The `tsk` plugin has no `PreToolUse` hook that
  stops a `git` or `dolt` command against the ledger store. A hook that denies a `Bash`
  command naming the ledger store path or a ledger ref is a possible control. This ADR does
  not decide it.
