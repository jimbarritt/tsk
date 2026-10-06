# Mission: Session migration

| Field | Value |
|---|---|
| ID | M-BOOT-07 |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | M-BOOT-04 |

## Objective

Kind: attainable

- Every Claude Code session Jim works in, local or cloud, runs on the `tsk` binary, the
  `tsk@jimbarritt-claude-plugins` plugin and `tsk/ledger`.
- No clone holds an old bootstrap ledger worktree.
- `refs/heads/tsk/bootstrap` and `refs/tsk/bootstrap` are deleted on origin. The tag
  `archive/tsk-bootstrap` keeps the final commit.
- The projects that used the removed daemon-backed `tsk` commands use the current
  command set.

This is the last state M-BOOT needs: no bootstrap scaffolding remains.

## Purpose

Parent: M-BOOT, bootstrap tsk self hosting. M-BOOT-04 replaced the bootstrap scripts
with the binary and moved the missions, threads and external events onto `tsk/ledger`.
Sessions started before that cut-over still hold the old harness state, and the old
refs stay on origin until those sessions move. Split out of M-BOOT-04 on 2026-10-06, at
Jim's request, from its "Clean-up for later" list.

## Intelligence

- M-BOOT-04 briefing, Decisions, and the clean-up items it held.
- `CLAUDE.md` on `main`: the ledger rules and the `tsk/bootstrap` name collision
  warning.
- `docs/user-guide/installation.md` and `docs/user-guide/new-nexus-on-a-clean-machine.md`
  on `main`: the install and plugin commands.
- `ops/local/claude-session-start.sh` on `main`: installs the plugin and builds `tsk` from
  `cli/` in this repo.
- M-BOOT-04 Decisions, 2026-10-03: Jim has a small number of live usages of the old
  `thread`, `task`, `context` and `where` commands in other projects, migrated by hand
  after M-BOOT-04.

## Decision authority

Jim decides when each session migrates and when the refs are deleted.

## Constraints

- Case by case: a session migrates when Jim returns to it. No bulk migration.
- Before removing an old bootstrap ledger worktree, check that `git status` in it is
  clean. Uncommitted work there is read first, not discarded.
- The refs on origin are deleted only after every session is migrated and working.

## Out of scope

- New harness features.
- The deferred T-10 items in M-BOOT-04: the `local` to `url` transition, a registration
  command, and moving an in-repo ledger worktree to the nexus.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Migrate sessions, case by case | Each session Jim returns to pulls `main`, runs on `tsk` and `tsk@jimbarritt-claude-plugins`, and resumes its thread with `/tsk:resume-thread`. That clone's old bootstrap ledger worktree at `.../tsk/repos/<clone-id>/bootstrap` is removed with `git worktree remove` once clean | none | IN PROGRESS: Jim's main local clone of tsk is migrated (2026-10-04) |
| T-02 | Migrate the old daemon command usages | The other projects that called `tsk thread`, `task`, `context` or `where` on `tskd` use the current commands or drop them | none | TODO |
| T-03 | Delete the bootstrap refs | `refs/heads/tsk/bootstrap` and `refs/tsk/bootstrap` are deleted on origin, and the name collision warning in `CLAUDE.md` is removed or reduced to what still applies | T-01 | TODO |

**Essential task**: T-03. Its end state and M-BOOT's objective are the same.

## Decisions

- 2026-10-06: keep both `tsk/bootstrap` refs on origin until every session is migrated
  and working. Remove the old bootstrap ledger worktrees case by case, as Jim returns to
  each session.

## Open decisions

None.
