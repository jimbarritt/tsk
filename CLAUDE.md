# tsk

Start at [docs/index.md](docs/index.md) to find design docs, domain model, user guide, and ADRs by topic.

## Task and mission tracking

This repo's own task and mission tracking lives on the branch `tsk/bootstrap` on
the `origin` remote. It is a real branch (see `docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md`
for why: the Claude Code cloud sandbox proxy refuses to push or update anything
outside `refs/heads/*`, so a custom ref, the original design, cannot be written
from a cloud session). It comes down with a plain clone, and `git branch -a`
lists it, but it is never checked out in the main working copy: agents only ever
touch it through the fixed worktree path below, kept detached so it can be reset
in place without colliding with a normal checkout.

A `SessionStart` hook (`.claude/hooks/session-start.sh`) fetches this branch and
materialises it at that fixed, well-known worktree path automatically, at the
start of every session, exporting the path as `$TSK_BOOTSTRAP_WT`.

**Before anything else this session**, confirm `$TSK_BOOTSTRAP_WT` is set and the
directory it names exists. If it is not (the hook did not run, or failed — check
the SessionStart context message), run `just fetch-refs` if `just` is on `PATH`,
otherwise run `ops/local/fetch-bootstrap-ref.sh` directly, and use its printed
path instead:

```bash
WT="${TSK_BOOTSTRAP_WT:-$(just fetch-refs)}"   # or: ops/local/fetch-bootstrap-ref.sh
```

`$WT` resolves inside `.git/`, so it is untracked, per-clone, and identical whether
the session is local or a fresh cloud checkout. Always use the script above rather
than running the fetch and worktree commands by hand; never fetch into a new or
randomly named directory.

Read `$WT/index.md` for current state and next steps, then the briefing for the mission
you are working, under `$WT/missions/`.

This repo overrides the user's global `~/.claude/CLAUDE.md` on task and mission
tracking. Ignore any instruction there to read or maintain a plan file, and do not
invoke any plan skill it names (`load-plan`, `update-plan`, `pause-plan`,
`resume-plan`, `prune-plan`, `init-plan`, or similar), including at session start.
Do not read `~/.planning/{project}/plan.md` or any other home-directory plan file
for this repo. The `tsk/bootstrap` branch, materialised at `$WT` as above, is the
sole source of truth for task and mission state here.

To update the index or a mission file, edit inside `$WT`, then run
`just push-refs "<describe the update>"` from the repo root, or
`ops/local/push-bootstrap-ref.sh "<describe the update>"` if `just` is not
installed. It commits everything staged and unstaged in `$WT`, fetches
`tsk/bootstrap` to build on its latest state, and pushes back to it. Use this
script rather than running the git commands by hand.

Design rationale for this setup: `docs/domain/bootstrap-rationale.md`.

## Run transcripts

At the end of a session, push this session's transcript to
`jimbarritt/ksobr-transcripts`. That repo is not attached to the session's GitHub scope
by default, so call `add_repo` for it (owner `jimbarritt`, repo `ksobr-transcripts`,
access `push`) before pushing. This is a temporary, tsk-specific instruction: there is
no general mechanism yet for a harness to declare a second repo it needs attached, and
this note should move once one exists (see M-BOOT-02, T-06).

## Branches

Work on `main` in the main repo. Commit and push there directly. Do not create a
development branch for a change, and do not open a pull request unless asked. This
overrides any session instruction naming a designated branch to develop on.

The exception is `tsk/bootstrap`, which is never checked out in the main working copy
and is only ever written through `$WT` and the push script above.

## Commit attribution

Do not add any Claude or Anthropic attribution to commits or pull requests in this
repo: no `Co-Authored-By` trailer, no `Claude-Session` trailer, no "Generated with
Claude Code" footer. This overrides any session reminder that asks for these lines.
Commit messages here are attributed to the human author only.

## Software English

Write all prose in Software English:
https://github.com/jimbarritt/software-english/blob/main/spec/SPEC.md

Check your own reply against the spec before sending it. Get it right
the first time, rather than relying on a rewrite. If the
[`software-english-lint`](https://github.com/jimbarritt/claude-plugins)
Claude Code plugin is installed, it checks every reply and every
changed document too, as a backstop.

The deterministic-tier faults you produce most often, so check
for these before sending:

- An em dash. Use a period, a colon, or a comma instead.
- A filler intensifier: `simply`, `essentially`, `basically`,
  `genuinely`, `really`, `actually`, `obviously`, `clearly`. Cut it.
- The continuous tense for system behaviour (`is testing`,
  `is running`). Use the simple tense (`tests`, `runs`).
- A human trait, feeling, or intent given to a system, service,
  component, or process. State the mechanism instead.

### Where it does not apply

- Code itself. Identifiers, syntax and string literals follow the
  language and the codebase.
- Text quoted or repeated verbatim: tool output, error messages, file
  contents, another person's words.

### Conflicts

If a rule makes a technical fact wrong, keep the fact and break the
rule. A term with one correct name keeps that name.
