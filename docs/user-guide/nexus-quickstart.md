# Put a repo's ledger in a nexus: the short version

Use this page to give a repo a new ledger that lives in a nexus. The long version is
[new-nexus-on-a-clean-machine.md](new-nexus-on-a-clean-machine.md). It does not move an
existing ledger.

You need `tsk` 0.36.5 or later, `git` with push access to the nexus repo, and `dolt`
2.5.0 or later (the plugin installs it).

## 1. Make the nexus repo, once

A nexus is a private git repo with a `nexus.json` on its default branch.

```bash
gh repo create <owner>/<name>-nexus --private --clone
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
   `M-<PREFIX>-NAME`. The nexus refuses a prefix that another repo in it has.
3. `--ledger-refs branches` keeps the ledger on branches. Use it for any repo that a Claude
   Code cloud session works in: the cloud proxy refuses a custom ref.
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

A cloud session needs two things before it finds the ledger:

- The setup script of the environment runs `tsk nexus add <nexus url>`.
- The session can reach the nexus repo on GitHub, so add the nexus repo to the repos of the
  environment.

## Another repo

Run section 3 in that repo. Run sections 1 and 2 once.

## What this page was checked against

Section 3 was run against a local bare repo as the nexus, with a repo that had a local
origin (so with `--local`). The ledger and its documents appeared as the branches
`ledgers/app` and `ledgers/app-docs` in the nexus. Section 5 and a push from a cloud session
to a nexus on GitHub were not run.
