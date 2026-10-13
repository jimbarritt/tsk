# tsk

Start at [docs/index.md](docs/index.md) to find design docs, domain model, user guide, and ADRs by topic.

## Migration note (2026-10-12)

The ledger moved from files on `tsk/ledger` to a Dolt database in one commit. A session
that runs an older `tsk` must update it and install `dolt`. The `SessionStart` hook does
both. By hand:

- Rebuild `tsk` from `cli/`, or run `cargo install --path cli --locked`.
- Install `dolt` 1.88.2 or later (`plugin/hooks/install-dolt.sh`).

The file ledger's full history is on the branch `archive/tsk-ledger-files`. An older
binary that fetches the Dolt branch finds no `.tsk-ledger.toml` and stops with a missing
`.tsk-ledger.toml` error before it writes. That error means the binary needs updating.

## Task and mission tracking

This repo's own task and mission tracking lives in the ledger: a Dolt database synced
over the branch `refs/heads/tsk/ledger` on the `origin` remote. The branch started from
the tip of the earlier `tsk/bootstrap` branch. Its file-era history is on
`archive/tsk-ledger-files`. It is a real branch (see
`docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md` for why: the
Claude Code cloud sandbox proxy refuses to push or update anything outside
`refs/heads/*`). It comes down with a plain clone, and `git branch -a` lists it, but it
is never checked out in the code worktree. The branch holds Dolt table files, not
Markdown. Agents only ever touch the ledger through the `tsk` commands and the ledger
store, a Dolt database at a fixed path outside the repository.

The repo's `SessionStart` script (`ops/local/claude-session-start.sh`) installs the
`tsk` plugin (`plugin/`) from the `jimbarritt/claude-plugins` marketplace, whose
`git-subdir` entry points at `plugin/` in this repo, installs `dolt` 1.88.2 when no
`dolt` 1.88.2 or later is installed, builds `tsk` from `cli/` when the installed binary
is not at the workspace version, then runs `tsk thread session-start`. The plugin's own `SessionStart` hook runs
the same command. The binary claims each event by session ID and source, so the second
run exits with no output. The command fetches the ledger, refreshes the ledger store,
exports its path as `$TSK_LEDGER_WT`, and prints the thread binding prompt.

The repo script runs the command itself because Claude Code reads plugin hooks and
skills once, when its process starts. A plugin installed during startup has no hooks in
that process, and `/clear` starts a new session in the same process without reading them
again. For the same reason, the first `/clear` after a plugin version bump runs the old
version's hooks and skills, even though the repo script installs the new version. A new
process loads the new version: open a new cloud session, or restart this one.

**Before anything else this session**, confirm `$TSK_LEDGER_WT` is set and the
directory it names exists. If it is not (the hook did not run, or failed: check the
`SessionStart` context message), run `tsk ledger fetch` (or `just ledger-fetch`), which
prints the path:

```bash
WT="${TSK_LEDGER_WT:-$(tsk ledger fetch)}"
```

The ledger store path is
`${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/ledger-db`: outside the
repository, per clone, and identical whether the session is local or a fresh cloud
checkout. `<clone-id>` is minted once and stored in `.git/tsk-clone-id`, so it survives
the clone directory being renamed or moved. See
`docs/adr/0009-bootstrap-worktree-outside-the-git-directory.md` for why it sits outside
`.git/`.

**The ledger store is a Dolt database, not a directory of files.** Do not open files in
it. Read and write documents with `tsk ledger doc` (below). The backend is `dolt` by
default. `[ledger] backend = "git"` in `${XDG_CONFIG_HOME:-$HOME/.config}/tsk/config.toml`,
or `TSK_LEDGER_BACKEND=git`, selects the file ledger. This repo uses the default.

**To read the path, use `tsk ledger path`, not `tsk ledger fetch`.** The fetch also
refreshes the ledger store to origin's latest, so calling it merely to resolve a path
is a write operation. `tsk ledger path` has no side effects.

**Never fetch, update or push the ledger by hand. Use `tsk ledger fetch` and
`tsk ledger push`** (or `just ledger-fetch` and `just ledger-push`). Do not run
`dolt push`, `dolt pull`, `dolt commit` or `dolt add` against the ledger store, and do
not run `git fetch`, `git rebase`, `git reset` or `git push` against `tsk/ledger`
yourself, and never clone into a new or randomly named directory. A script you write that
needs the ledger calls `tsk` from inside itself; it does not inline its own `dolt` or
`git` sequence. The binary holds that logic once, with tests.

`tsk/bootstrap` is retired. Its final commit is tagged `archive/tsk-bootstrap`, and
nothing writes to the branch.

Two refs share the name `tsk/bootstrap` on `origin`: the retired branch
`refs/heads/tsk/bootstrap`, and an orphaned custom ref
`refs/tsk/bootstrap` left behind by the original design (ADR 0008). Git resolves an
unqualified `tsk/bootstrap` against `refs/tsk/bootstrap` first, so
`git fetch origin tsk/bootstrap` exits 0, prints a plausible success line, and returns
the orphaned ref's content, frozen at 2026-09-14, with no error. The `tsk` binary
passes full ref names to git and is not affected. A git command you run by hand against
`tsk/bootstrap` or `tsk/ledger`, even a read-only check, must spell out the ref in full:
`git fetch origin refs/heads/tsk/ledger`, then read `FETCH_HEAD`. Anything shorter is a
guess, not a check.

Read the index with `tsk ledger doc get index.md` for current state and next steps, then
the briefing for the mission you are working, with
`tsk ledger doc get missions/<group>/<file>.md`. `tsk ledger doc list missions/` lists
the briefings.

This repo overrides the user's global `~/.claude/CLAUDE.md` on task and mission
tracking. Ignore any instruction there to read or maintain a plan file, and do not
invoke any plan skill it names (`load-plan`, `update-plan`, `pause-plan`,
`resume-plan`, `prune-plan`, `init-plan`, or similar), including at session start.
Do not read `~/.planning/{project}/plan.md` or any other home-directory plan file
for this repo. The ledger is the sole source of truth for task and mission state here.

To update the index or a mission file, write it with `tsk ledger doc put <path>` (body on
stdin, or a file as the second argument), then run
`just ledger-push "<describe the update>"` from the repo root, or
`tsk ledger push "<describe the update>"`. `doc put` changes the local store only. The
push commits it in Dolt, pulls the latest ledger, and pushes back, up to 5 attempts. A
merge conflict stops the push with exit 1 and names the conflicting tables. The local
commit stays.

Prose says "ledger store" for the local Dolt database and "code worktree" for a worktree
of this repo's code. It never says "worktree" alone.

Design rationale for this setup: `docs/domain/bootstrap-rationale.md` and
`docs/domain/ledger-layout.md`.

## Run transcripts

Disabled. Do not push session transcripts anywhere. The transcript repo
`jimbarritt/ksobr-transcripts` is not set up yet. When it is, this section states how
to push to it (see M-BOOT-02, T-06).

## Branches

Work on `main` in the main repo. Commit and push there directly. Do not create a
development branch for a change, and do not open a pull request unless asked. This
overrides any session instruction naming a designated branch to develop on.

The exception is `tsk/ledger`, which is never checked out in the code worktree and is
only ever written through the ledger worktree and `tsk ledger push`.

## Shallow clones

The repo's `SessionStart` hook (`ops/local/claude-session-start.sh`) unshallows the clone
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
[`swe`](https://github.com/jimbarritt/claude-plugins)
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
