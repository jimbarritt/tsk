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

`$WT` resolves to
`${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/bootstrap`: outside the
repository, per clone, and identical whether the session is local or a fresh cloud
checkout. `<clone-id>` is minted once and stored in `.git/tsk-clone-id`, so it
survives the clone directory being renamed or moved. It used to sit inside `.git/`;
see `docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md` for why it moved.
`fetch-bootstrap-ref.sh` migrates an existing clone off the old location
automatically, and refuses rather than discarding if that worktree holds
uncommitted work.

**`$WT` is a checkout location, not a ref. Editing a file there is not "editing the
ref."** A ref is a pointer file under `.git/refs/` (or packed). `$WT` is where a
linked worktree's ordinary working-tree files live: `missions/*.md`, `index.md`, no
different in kind from a file in the main checkout. Editing a file inside `$WT`,
then running `push-bootstrap-ref.sh`, is the correct and only sanctioned way to
change `tsk/bootstrap`'s content. The real trap in this territory is the ref-name
collision below, not the checkout.

**To read the path, use `ops/local/bootstrap-wt-path.sh`, not
`fetch-bootstrap-ref.sh`.** The fetch script also refreshes the worktree to
origin's latest, so calling it merely to resolve a path is a write operation.
`bootstrap-wt-path.sh` is pure and has no side effects.

**Never fetch, update or push the bootstrap data by hand. Use the scripts.** Do not
run `git fetch`, `git rebase`, `git reset` or `git push` against `tsk/bootstrap`
yourself, in the worktree or anywhere else, and never fetch into a new or randomly
named directory. This holds for a read-only check as much as for an update: a bare
`git fetch origin tsk/bootstrap`, or `git log`/`git ls-tree` against a bare
`origin/tsk/bootstrap`, run only to verify something, hits the exact same collision
below and returns silently wrong content, with no error to flag it. Read with
`ops/local/fetch-bootstrap-ref.sh`, write with `ops/local/push-bootstrap-ref.sh`, or
use their `just` equivalents. This holds even when a hand-run command looks
equivalent to what the script does.

To check the branch's real state directly, without running either script, spell out
the ref in full: `git fetch origin refs/heads/tsk/bootstrap`, then read `FETCH_HEAD`.
Anything shorter is a guess, not a check.

**This also binds a script you write, not only a command you run directly.** A
script that needs to commit and push to `tsk/bootstrap` calls
`ops/local/push-bootstrap-ref.sh` (or `fetch-bootstrap-ref.sh` for a read) from
inside itself. It does not inline its own `git add` / `git commit` / `git fetch
origin refs/heads/tsk/bootstrap` / `git push origin HEAD:refs/heads/tsk/bootstrap`
sequence, even spelled out in full and even when it looks correct: a second copy
of that sequence is a second thing to keep in sync with the real script, and the
whole point of the two scripts existing is that there is exactly one place this
logic lives. This was found and fixed in M-BOOT-02-01: its first drafts of
`thread-start.sh`, `thread-append-handover.sh` and `thread-resume.sh` each
duplicated the push sequence inline instead of calling
`push-bootstrap-ref.sh`.

The reason is a name collision on `origin`. Two refs share the name `tsk/bootstrap`:
the live branch `refs/heads/tsk/bootstrap`, and an orphaned custom ref
`refs/tsk/bootstrap` left behind by the original design (ADR 0008). Git resolves an
unqualified `tsk/bootstrap` against `refs/tsk/bootstrap` first, so
`git fetch origin tsk/bootstrap` exits 0, prints a plausible success line, and
returns the orphaned ref's content, frozen at 2026-09-14. No error is raised.
Rebasing onto that state reverts work already on the branch. The scripts spell out
`refs/heads/tsk/bootstrap` in full and are not affected.

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

## Shallow clones

The `SessionStart` hook (`ops/local/claude-session-start.sh`) unshallows the clone
automatically, before anything else runs, if it finds one. Do not remove that step.

Background, for the case where a clone is shallow anyway (the hook did not run, or
ran before this fix landed): a shallow clone truncates history at a fetch-depth
boundary and marks the commit there as having no parent, even though a parent
exists on GitHub. A second shallow fetch, run later, can truncate at a different
point. Comparing two branches that were each shallow-fetched at different times
then finds no common ancestor and looks exactly like a rewritten, unrelated
history, on a repository where nothing was actually rewritten.

If `git merge-base` or `git log` ever reports two branches as unrelated and that
is surprising, check `git rev-parse --is-shallow-repository` before concluding a
history rewrite happened. If it prints `true`, run `git fetch --unshallow origin`
and compare again. This is a real, recorded failure mode, found and fixed during
M-STORY: a session spent real time and real concern diagnosing `main` as
force-pushed with a disjoint history, when the clone was simply shallow at two
different boundaries.

## Commit attribution

Attribution follows whoever runs the commit and push, not a fixed rule for the repo.
Git config in this environment already distinguishes an agent's commits from a human's.
When a human runs the commit and push themselves, the commit is attributed to that
human. When an agent runs it, agent attribution is fine.

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
