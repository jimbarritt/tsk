# Put a repo's ledger in a nexus: quick start

Use this page to give a repo a new ledger in a nexus. The full guide is
[new-nexus-on-a-clean-machine.md](new-nexus-on-a-clean-machine.md). To copy a repo's
existing ledger to a nexus, see [Copy an existing ledger to the nexus](#copy-an-existing-ledger-to-the-nexus).

You need `tsk` 0.36.5 or later, `git` with push access to the nexus repo, and `dolt`
2.5.0 or later (the plugin installs it).

## 1. Make the nexus repo, once

A nexus is a private git repo with a `nexus.json` on its default branch. An empty repo is
enough: the first `tsk nexus register-repo` in section 3 creates `nexus.json`.

```bash
gh repo create <owner>/<name>-nexus --private
```

Writing `nexus.json` by hand is optional:

```bash
gh repo clone <owner>/<name>-nexus
cd <name>-nexus
printf '{"version":1,"territories":[{"id":"work","name":"Work","repos":[]}]}\n' > nexus.json
git add nexus.json
git commit -m "Add nexus.json"
git push -u origin HEAD
```

## 2. Attach the nexus to the machine, once

```bash
tsk nexus add <nexus url>
tsk nexus list
```

`tsk nexus list` prints the nexus URL and its territories.

## 3. Register each repo

Run these in the repo, in this order.

```bash
tsk ledger fetch
tsk mission prefix <PREFIX>
tsk nexus register-repo --ledger-refs branches
tsk ledger fetch
tsk ledger push "Create the ledger"
```

1. The first `tsk ledger fetch` makes the local ledger. `tsk mission prefix` and
   `tsk nexus register-repo` need it.
2. `<PREFIX>` is 3 or 4 capital letters. It starts every mission ID in this repo, such as
   `M-<PREFIX>-NAME`. `tsk nexus register-repo` fails with an error when another repo in the nexus has the prefix.
3. On an empty nexus, the first repo also passes `--territory <id> --territory-name <name>`,
   for example `--territory work --territory-name Work`. The command creates `nexus.json`
   with that territory, and stops with an error that names `--territory` without it.
   `--ledger-refs branches` keeps the ledger on branches. Use it for any repo that a Claude
   Code cloud session works in: the cloud proxy rejects a push to a custom ref.
4. The second `tsk ledger fetch` reads the nexus entry. The push creates the ledger.
5. For a repo with no remote URL, add `--local` to `tsk nexus register-repo`.

Check the result:

```bash
tsk ledger status
git ls-remote <nexus url>
```

The status shows `refs branches (nexus)`. The nexus has the branches `ledgers/<repo-id>` and
`ledgers/<repo-id>-docs`. The repo ID is the repo name in lower case.

## 4. Install the plugin and start

```bash
tsk install-plugin claude-cli --scope local
claude
```

## 5. A Claude Code cloud environment

A cloud session finds the ledger only when both of these are set up:

- The setup script of the environment runs `tsk nexus add <nexus url>`.
- The session can fetch from and push to the nexus repo on GitHub, so add the nexus repo to the repos of the
  environment.

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
   `ledgers/<repo-id>` and `ledgers/<repo-id>-docs` in the nexus. It stops when either ref
   in the nexus points at a different commit, and skips a ref that points at the same one.
3. Sets the repo's entry in `nexus.json` to `"ledger": "nexus"` with `ledger_refs` and the
   mission prefix, and adds the entry when the nexus has none.
4. Leaves the old refs on origin unchanged, and prints the command that deletes them.

The clone that ran the command, and every other clone after its next `tsk ledger fetch`,
reads and writes the ledger in the nexus. The old refs and their history stay readable on
origin until you delete them. Delete them once every clone has fetched. A Claude Code cloud
session cannot delete a branch: run the delete from a local clone.

## Another repo

Run section 3 in that repo. Run sections 1 and 2 once.

## What this page was checked against

Section 3 was run against a local bare repo as the nexus, with a repo that had a local
origin (so with `--local`). The ledger and its documents appeared as the branches
`ledgers/app` and `ledgers/app-docs` in the nexus. Section 5 and a push from a cloud session
to a nexus on GitHub were not run. The creation of `nexus.json` on an empty nexus is
covered by the end-to-end tests against a local bare repo with no commits. It was not run
against an empty repo on GitHub.
