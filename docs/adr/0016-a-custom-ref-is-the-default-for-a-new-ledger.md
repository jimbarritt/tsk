# 16. A custom ref is the default for a new ledger

Date: 2026-10-19

## Status

Accepted. Supersedes the part of ADR 0008 that says the ledger is a branch and never a
custom ref, for a new ledger. The rest of ADR 0008 stands: the Claude Code cloud proxy
refuses to push or update a ref outside `refs/heads/*`, tsk passes full ref names to git,
and a repo used from cloud sessions keeps branches. Decided by Jim on 2026-10-15 and
2026-10-16 (M-TSK-SCHEMA, objective 7).

## Context

The ledger and its documents sit on `refs/heads/tsk/ledger` and `refs/heads/tsk/docs`.
GitHub lists them as branches and shows a "Compare & pull request" banner after every
push. GitHub has no setting to hide a branch. For a product with users, the banner and the
branch list are noise.

A ref outside `refs/heads/*` does not appear in either place. ADR 0008 moved off a custom
ref because a cloud session could not push it. A test from a cloud session on 2026-10-15
confirmed the limit still holds:

| Route | Result |
|---|---|
| `git push origin HEAD:refs/tsk/<name>` | Cut off by the proxy. The ref does not exist afterwards |
| `POST /repos/{owner}/{repo}/git/refs` through `gh` | Refused: "Write access to this GitHub API path is not permitted through this proxy" |
| A remote `dolt sql-server` or DoltHub | Blocked: raw TCP and gRPC do not pass the proxy |

Every other client can push a custom ref. Dolt 2.5.0 accepts one for its git remote:
`dolt remote add --ref refs/tsk/ledger` and `dolt clone --ref refs/tsk/ledger` work, and
push, fetch and merge behave as they do on a branch. Git fetches and pushes the ref when
the full name is given.

## Options considered

| Option | What it is | Finding |
|---|---|---|
| A. Branches for everyone | The state before this ADR | Works in every client. Leaves a branch and a banner in every repo |
| B. A custom ref for everyone | Drop branches | Hides the data. A cloud session cannot push it |
| C. A custom ref by default, branches as an override | A setting per repo, read from the nexus, the repo, or discovery | Hides the data where it can be hidden. A cloud repo opts out once |
| D. An Action in each repo that moves the branch to a custom ref after each push | The cloud session pushes a branch, a workflow renames it | Adds a workflow and a delay to every repo, and the branch exists in between |
| E. A nexus repo per user for every ledger | Keep ledgers out of the code repo | Jim declined it: it asks every user to run a second repo |

## Decision

1. **A new ledger uses a custom ref.** The refs are `refs/tsk/ledger` and `refs/tsk/docs`
   for a repo's own ledger, and `refs/tsk/ledgers/<id>` and `refs/tsk/ledgers/<id>-docs`
   for a ledger held in a nexus.
2. **Branches are an override.** A repo used from Claude Code cloud sessions sets
   `branches`, and the refs stay as ADR 0008 had them.
3. **The setting is read in this order**, and the first source with an answer wins: the
   `ledger_refs` field of the repo's entry in the nexus, then `[ledger] refs` in
   `.tsk/config.toml` at the root of the code repo, then discovery. `TSK_LEDGER_REFS` sets
   the mode for one process and outranks all three.
4. **Discovery looks at what exists.** An existing ledger store decides, by the ref its
   Dolt remote records. Else one `git ls-remote` asks origin for `refs/heads/tsk/ledger`
   and `refs/tsk/ledger`, and the answer is cached. A ledger that exists nowhere is new:
   `custom`, except in a Claude Code cloud session, where the first push of a custom ref
   would fail, so it takes `branches` and says so.
5. **tsk does not move a ledger between the modes.** A mode that disagrees with the ledger
   store or with origin stops the command with an error that names both refs. It never
   makes a second ledger beside the first.
6. **A host that refuses a custom ref is named.** The push stops with an error that names
   the refused ref, says the host may accept only branches, and gives the fix
   `tsk config ledger-refs branches`. The local commits stay.
7. **This repo keeps branches.** It is used from cloud sessions. `.tsk/config.toml` in the
   tree says so, so the choice is explicit and committed.

## Consequences

- A new ledger on a user's machine leaves no branch on the host, and GitHub shows no
  banner.
- A repo that someone uses from a cloud session must set `branches` before the first
  push. The cloud default does that for a repo with no ledger yet, and the command that
  creates the ledger prints one sentence about it. A repo whose ledger a local session
  already made as a custom ref cannot be used from the cloud. If the proxy does not list a
  custom ref to `git ls-remote`, a cloud session takes the cloud default and starts a
  second ledger on branches. Nobody measured whether the proxy lists one. Setting
  `branches` in the repo config before any ledger exists avoids the question.
- An existing ledger on branches keeps working with no change. Its store records the ref,
  so discovery finds it without a network call.
- Mode switching is not built. A person who chose wrongly before the first push runs
  `tsk config ledger-refs <mode>`, which also points an unpublished ledger store at the
  new refs. A ledger that origin holds stays where it is.
- Dolt still writes its own `__dolt_remote_info__` branch on a push unless
  `DOLT_REMOTE_INFO_BRANCH` is empty. tsk sets it empty, so a custom ledger adds no branch.
- ADR 0008 is not edited. Its decision for a ledger that already exists as a branch is
  read together with this ADR.
