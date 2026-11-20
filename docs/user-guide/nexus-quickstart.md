# Put a repo's ledger in a nexus: quick start

Use this page to give a repo a new ledger in a nexus. The full guide is
[new-nexus-on-a-clean-machine.md](new-nexus-on-a-clean-machine.md). To copy a repo's
existing ledger to a nexus, see [Copy an existing ledger to the nexus](#copy-an-existing-ledger-to-the-nexus).

You need `tsk` 0.57.0 or later, `git` with push access to the nexus repo, and `dolt`
2.5.0 or later (the plugin installs it).

## 1. Make the nexus repo, once

A nexus is a private git repo with a `nexus.json` on its default branch. Create the repo
empty. The first `tsk ledger init` in section 3 writes `nexus.json` and its first territory.

```bash
gh repo create <owner>/<name>-nexus --private
```

## 2. Attach the nexus to the machine, once

```bash
tsk nexus add <nexus url>
tsk nexus list
```

`tsk nexus list` prints the nexus URL and its territories.

## 3. Create each repo's ledger

You need `tsk` 0.57.0 or later for `tsk ledger init`. Run these in the repo:

```bash
tsk ledger status
tsk config ledger-refs branches
tsk ledger init --nexus --prefix <PREFIX>
```

1. `tsk ledger status` prints `Local   no ledger in this clone` and a `Next` line. Go on
   when `Next` names `tsk ledger init`. With a nexus attached, `tsk ledger fetch` stops in a
   repo that has no ledger, and creates no ledger on origin.
2. `tsk config ledger-refs branches` writes `.tsk/config.toml`, which keeps the ledger on
   branches. Commit the file. Use it for any repo that a Claude Code cloud session works
   in: the cloud proxy rejects a push to a custom ref.
3. `<PREFIX>` is 3 or 4 capital letters. It starts every mission ID in this repo, such as
   `M-<PREFIX>-NAME`. The command checks it against the other repos in the nexus, adds the
   repo to `nexus.json` with `"ledger": "nexus"`, creates the ledger and pushes it.
4. Add `--territory <id>` for the first repo of a new nexus, which creates `nexus.json` and
   the territory, and whenever the nexus has more than one territory. `--territory-name
   <name>` names a territory that the command creates.
5. `--workflow main` or `--workflow pr` saves the push workflow in the repo's entry. On a
   terminal the command prompts for the workflow when the entry has none. For an entry that
   exists, the command writes the workflow after it creates and pushes the ledger, so the
   entry stays as it was after a failed init. For a repo with no entry, the entry and the workflow are one commit
   that precedes the ledger.
6. With `[attribution] identity` set to `robot`, the command registers the robot of the
   session in the new ledger and sets the mission prefix before its first commit. The
   command reads the robot name from the `tsk-robot-name` file in the git directory of the
   worktree, or from the session environment. With no name it stops before it changes anything, and names
   `tsk actor register "<name>" --kind robot` and the `/tsk:construct-robot` skill.

In Claude Code, the `/tsk:init-ledger` skill runs the same steps, with a prompt for where the ledger
goes. See [session-start.md](session-start.md#a-repo-with-no-ledger).

Check the result:

```bash
tsk ledger status
git ls-remote <nexus url>
```

The status shows `refs branches (nexus)`. The nexus has the branches `ledgers/<repo-id>/ledger` and
`ledgers/<repo-id>/docs`. The repo ID is the repo name in lower case. The ledger home on this
machine is `~/.local/state/tsk/ledgers/<repo-id>`, one for every clone of the repo.

## 4. Install the plugin and start

Install the plugins once per machine, at user scope:

```bash
claude plugin install tsk@jimbarritt-claude-plugins --scope user
claude plugin install swe@jimbarritt-claude-plugins --scope user
claude
```

On the first turn, the agent prompts for the robot of the session and the work to do. See
[session-start.md](session-start.md).

## 5. A Claude Code cloud environment

A cloud session finds the ledger only when both of these are set up:

- The setup script of the environment passes `--nexus <nexus url>` to
  `ops/cloud/setup-environment.sh`. See
  [new-cloud-environment.md](new-cloud-environment.md#3-add-the-setup-script).
- The account's GitHub connection has read and push access to the nexus repo.

A new cloud session has only the opened repo attached, so the fetch of the nexus at
session start fails. On the first turn the agent attaches the nexus with the `add_repo`
tool, which `--nexus` pre-approves, and runs `tsk ledger fetch`, which fetches the nexus. See [session-start.md](session-start.md#a-cloud-session).

## Copy an existing ledger to the nexus

One command copies a ledger from a repo's own `tsk/ledger` and `tsk/docs` branches to the
nexus (tsk 0.53.0 or later). Attach the nexus first (section 2), then in the repo:

```bash
tsk ledger push "Before the move"
tsk nexus move-ledger --ledger-refs branches
```

The command:

1. Stops when the ledger store on this machine has anything that origin does not have, or when origin
   cannot be checked.
2. Copies origin's ledger ref and documents ref, with their full history, to
   `ledgers/<repo-id>/ledger` and `ledgers/<repo-id>/docs` in the nexus, through the bare clone
   of the clone's ledger home. It stops when either ref in the nexus points at a different
   commit, and skips a ref that points at the same one. It also stops, with the delete command,
   when the nexus has an old layout name or a backup ref of the same id. It pushes both refs
   in one atomic push, so the nexus is unchanged after a failed push. It then renames the ledger home of the
   repo to the home of the ledger in the nexus.
3. Sets the repo's entry in `nexus.json` to `"ledger": "nexus"` with `ledger_refs` and the
   mission prefix, and adds the entry when the nexus has none.
4. Leaves the old refs on origin unchanged, and prints the command that deletes them.

The clone that ran the command, and every other clone after its next `tsk ledger fetch`,
reads and writes the ledger in the nexus. The old refs and their history stay readable on
origin until you delete them. Delete them after every clone runs `tsk ledger fetch`. A Claude Code cloud
session cannot delete a branch: run the delete from a local clone.

## Edit nexus.json

These commands edit `nexus.json` on the nexus's default branch, commit and push the
change with the machine's git credentials. Each prints one line that names the change,
or `unchanged`, then the pushed commit.

```bash
tsk nexus add-territory play --name Play
tsk nexus rename-territory play --name "Play and tests"
tsk nexus set-repo app --workflow pull-request --prefix APP
tsk nexus move-repo app --territory play
tsk nexus remove-repo app
tsk nexus remove-territory play
```

- `add-territory` adds an empty territory. The name defaults to the id. An id that exists
  changes nothing.
- `remove-territory` stops when the territory still contains repos, and names how many.
- `set-repo` needs at least one option. The `--workflow` value is `main` or `pull-request`.
  The `--prefix` value is 3 or 4 capital letters that no other repo in the nexus uses.
  A value equal to the stored one changes nothing.
- `move-repo` stops when the territory does not exist.
- `remove-repo` stops when the entry keeps its ledger in the nexus. The ledger branches stay
  in the nexus, and removing the entry hides them. `--force` removes the entry anyway.

## Another repo

Run section 3 in that repo. Run sections 1 and 2 once.

## What this page was checked against

An earlier section 3, with `tsk ledger fetch`, `tsk mission prefix` and
`tsk nexus register-repo`, was run against a local bare repo as the nexus, with a repo that
had a local origin. The ledger and its documents appeared as the branches `ledgers/app` and
`ledgers/app-docs` in the nexus (the names of 0.69.0 and earlier; they are now
`ledgers/app/ledger` and `ledgers/app/docs`). `tsk ledger init` in the nexus is covered by the
integration tests of release 0.57.0. The current section 3 was not run by hand. Section 5
and a push from a cloud session to a nexus on GitHub were not run.
