# tsk

Start at [docs/index.md](docs/index.md) to find design docs, domain model, user guide, and ADRs by topic.

## Migration note (2026-10-17)

Missions, objectives, constraints, out of scope, tasks, decisions, questions and thread
state are now records: typed rows in the Dolt database, written through `tsk` commands.
The ledger is at layout 10 (layout 9 plus `missions.code` of 0.30.0: a mission has an
internal ID and the code a person types, so `tsk mission mv` renames it by one cell).
Layout 9 added the `actors` and `instances` tables of 0.26.0. Markdown briefings and `index.md` are no longer the state of the work. Documents stay Markdown on `tsk/docs` for intelligence, research and reports.

- The first `tsk` write on 0.14.0 or later upgrades a ledger to layout 8. A binary older
  than that then stops with `unsupported ledger version`. Update it with `tsk self-update`.
  From 0.18.0 a `tsk` updates itself and runs the command again (`TSK_AUTO_UPDATE=never`
  switches that off). A binary released before 0.18.0 cannot, so update it by hand.
- Read state with `tsk mission list`. Do not look for `index.md` or a briefing.
- `tsk guide import` describes how to bring a plan into the ledger as records.

## Migration note (2026-10-12)

The ledger moved from files on `tsk/ledger` to a Dolt database in one commit. A session
that runs an older `tsk` must update it and install `dolt`. The `SessionStart` hook does
both. By hand:

- Run `tsk self-update`, or rebuild `tsk` with `just build-install`.
- Install `dolt` 2.5.0 or later (`plugin/hooks/install-dolt.sh`).

The file ledger's full history is on the branch `archive/tsk-ledger-files`. An older
binary that fetches the Dolt branch finds no `.tsk-ledger.toml` and stops with a missing
`.tsk-ledger.toml` error before it writes. That error means the binary needs updating.

A ledger at manifest version 1 keeps its documents in a Dolt table. Run
`tsk ledger migrate-docs` once to move them to `tsk/docs`. Until then every command except
`tsk ledger fetch` stops and names this command.

## Task and mission tracking

This repo's own task and mission tracking lives in the ledger: a Dolt database synced
over the branch `refs/heads/tsk/ledger` on the `origin` remote. The branch started from
the tip of the earlier `tsk/bootstrap` branch. Its file-era history is on
`archive/tsk-ledger-files`. It is a real branch (see
`docs/adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md` for why: the
Claude Code cloud sandbox proxy refuses to push or update anything outside
`refs/heads/*`). It comes down with a plain clone, and `git branch -a` lists it, but it
is never checked out in the code worktree. The branch holds Dolt table files, not
Markdown. From 0.21.0 a new ledger's ref holds a Dolt file remote: a directory of
`manifest`, table files and `.tsk-transport`, which `tsk` keeps as a linked worktree beside
the ledger store and moves with plain `git fetch` and `git push` (ADR 0017). Dolt never
starts git. A ledger made before 0.21.0 holds Dolt's git blobstore (transport `git-remote`)
until someone runs `tsk ledger migrate-transport` once. Every clone moves to the new layout
on its next fetch. This repo's ref is in the new layout: its `.tsk-transport` reads
`file-remote`. A clone needs `tsk` 0.21.0 or later to read it. The layout version stayed
at 8, so a binary from 0.18.0 to 0.20.x does not update itself for this change: run
`tsk self-update` by hand.

This repo keeps its ledger on branches because it is used from Claude Code cloud
sessions, and the cloud proxy refuses to push a custom ref. `.tsk/config.toml` in the
tree says so (`[ledger]` and `refs = "branches"`), so the choice is explicit and
committed. A new ledger in any other repo uses a custom ref, `refs/tsk/ledger` and
`refs/tsk/docs`, which GitHub does not list as a branch (ADR 0016). `tsk config
ledger-refs` prints the refs in use and where the choice came from, and `tsk config
ledger-refs branches|custom` writes the file. Whichever refs a ledger uses, a git
command you run by hand names the ref in full, such as `refs/tsk/ledger`. Agents only ever touch the ledger through the `tsk` commands and the ledger
store, a Dolt database at a fixed path outside the repository.

The repo's `SessionStart` script (`ops/local/claude-session-start.sh`) installs the
`tsk` plugin (`plugin/`) from the `jimbarritt/claude-plugins` marketplace, whose
`git-subdir` entry points at `plugin/` in this repo, installs `dolt` 2.5.0 when no
`dolt` 2.5.0 or later is installed, installs `tsk` from the latest release when the
installed binary is older than `plugin/tsk-version`, then runs `tsk thread session-start`.
Neither hook builds `tsk` from source, and a build at or above that version (from
`just build-install`) is never replaced. The install logic for both hooks is in `plugin/hooks/install-tsk.sh`: `tsk self-update` for
a release install, otherwise the release archive over that file (the file a symbolic link
names, written through a temporary file and a rename). With no `tsk`, the archive installs to
`TSK_INSTALL_DIR`, else `/opt/homebrew/bin` on an Apple silicon Mac when that directory
is writable, else `/usr/local/bin` when writable, else `~/.local/bin`. The script
prints a `WARNING:` line and exits 0 on failure. The plugin's own `SessionStart` hook runs
the same command. The binary claims each event by session ID and source, so the second
run exits with no output. The command fetches the ledger, refreshes the ledger store,
exports its path as `$TSK_LEDGER_WT`, and prints the thread binding prompt. With the
local Dolt server switch on (the default since 0.29.0; `TSK_DOLT_SERVER=off` or
`[ledger] server = "off"` switches it off, and `tsk config ledger-server` prints it), the
command first checks the server of the ledger store and starts a dead or stale one in the
background, which is how a cloud session that woke from
hibernation gets a live server. While a server runs, `dolt` opens the ledger store read
only: stop it with `tsk ledger server stop` before a write by hand. An idle server stops
after 1800 s (since 0.25.0; `TSK_DOLT_SERVER_IDLE_SECS` overrides it). See
`docs/adr/0018-a-local-dolt-server-per-ledger-store-behind-a-switch.md`.

The plugin steps take about 4 s for each plugin, so the script runs them in the
background while `tsk` starts, and skips them for 60 minutes after a run with no warning
(`TSK_PLUGIN_ENSURE_MINUTES=0` runs them at every start). A new plugin version is picked up
within that time. See `docs/arch/2026-10-20-session-start-and-pause.md`.

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
it. It holds records, and the `tsk` commands read and write them. Documents are Markdown
files on a second branch, `refs/heads/tsk/docs`, checked out as a worktree beside the
database. Read and write them with `tsk ledger doc` (below), and find a file on disk with
`tsk ledger doc path <path>`. `tsk ledger push` pushes the documents first and the Dolt
data second. A ledger is a Dolt database. The files backend was removed in 0.16.0 (ADR
0015). If `[ledger] backend = "git"` in
`${XDG_CONFIG_HOME:-$HOME/.config}/tsk/config.toml`, or `TSK_LEDGER_BACKEND=git`, is set,
`tsk` prints an error and exits 1. Release 0.15.1 is the last that reads a file ledger.

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

This repo overrides the user's global `~/.claude/CLAUDE.md` on task and mission
tracking. Ignore any instruction there to read or maintain a plan file, and do not
invoke any plan skill it names (`load-plan`, `update-plan`, `pause-plan`,
`resume-plan`, `prune-plan`, `init-plan`, or similar), including at session start.
Do not read `~/.planning/{project}/plan.md` or any other home-directory plan file
for this repo. The ledger is the sole source of truth for task and mission state here.

Prose says "ledger store" for the local Dolt database and "code worktree" for a worktree
of this repo's code. It never says "worktree" alone.

### Records and documents

The ledger holds records (state) and documents (free text). Read the state with these
commands:

```bash
tsk mission list                 # every mission: id, status, blocked-by, parent, name
tsk mission list <id> --json     # one mission in full: purpose, constraints, out of scope,
                                 # objectives, blocks, linked documents, tasks, decisions,
                                 # questions
tsk task list <id>               # the tasks of a mission
tsk decision list [<id>]         # the decisions of a mission, of every mission with no id
tsk question list [<id>]         # the open questions
```

`tsk mission list` is the index. The list commands print columns aligned with spaces, and
`--porcelain` prints them tab-separated for a script. There is no `index.md` and no briefing to read. Write
state with the record commands: `tsk mission create|set|block|objective|constraint|
out-of-scope|link`, `tsk task add|set|block`, `tsk decision add`, `tsk question
add|edit|answer`. Run `tsk <command> --help` for the syntax, or load the `tsk` skill.
A mission is done when its status is set to `done`. tsk does not check its objectives.

Do not repeat a record in a document. Write a document only for free text (intelligence,
research, reports, designs) with `tsk ledger doc put <path>` (body on stdin, or a file as
the second argument), and link it to its mission with `tsk mission link <id> <path> --role
intelligence|report|design`. Read one with `tsk ledger doc get <path>`, and list them with
`tsk ledger doc list [<prefix>]`.

Every write changes the local ledger store only. Write freely: records are published at
four points and nowhere else (ADR 0019): `tsk thread start` (the claim and the binding),
`tsk thread pause` and `tsk thread stop` (the hand-off), the end of the session (the
`SessionEnd` hook, and the `Stop` hook, which publishes in the background after a turn when
the store holds unpushed changes, ADR 0020), and `tsk ledger push` when asked. `thread resume`, `detach`, `set mission`, `skip` and
`prune-bindings` do not push. Taking a task is a claim, so after setting a task to
`in-progress`, run `tsk ledger push "Claim <mission> <task>"` straight away. Progress can
wait for the next point. The `Stop` hook does not ask you to push: it runs
`tsk ledger push --background`, a detached one-shot child. If that push fails, the next
`tsk` command prints `warning: the last background publish failed at <time>: <error>; run
tsk ledger push` once, and `tsk ledger status` shows a `Background publish:` line. Then:

- `tsk ledger status` shows what is not pushed, and whether origin is ahead.
- `tsk ledger push "<describe the update>"` (or `just ledger-push "<describe the update>"`
  from the repo root) commits it in Dolt, pulls the latest ledger, and pushes back, up to
  10 attempts. A merge conflict stops the push with exit 1 and names the conflicting
  tables. The local commit stays.
- `tsk ledger discard` lists what it would drop, and `tsk ledger discard --yes` drops
  every local change before the push. Never use `dolt reset`.

<!-- LEAD: confirm the T-05 migration ran before merging this section -->
This repo's missions are records. A mission that is done has a bare record, with its
final briefing as a document linked to it.

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
