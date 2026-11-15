# Keep the nexus on local disk

Use this page to run tsk on a machine that keeps every ledger on its own disk. The nexus
is a bare git repo in a folder on that disk. tsk clones it and pushes to it through a
`file://` address, the same way it uses a nexus on GitHub.

Tested with tsk 0.68.1 and git 2.43.0 on Linux, on 2026-11-15.

## 1. Install tsk

```bash
brew install jimbarritt/tap/tsk
```

Or download a release archive, as [installation.md](installation.md) describes. You also
need `dolt` 2.5.0 or later. The tsk plugin's session start hook installs it when it is
missing.

## 2. Create the nexus

```bash
git init --bare --initial-branch=main ~/tsk-nexus.git
```

This is the local equivalent of an empty repo on GitHub. Keep `--initial-branch=main`:
tsk writes `nexus.json` to `main`, and git's default branch for a new repo can be another
name. With another default, every later command of tsk 0.68.x stops with `couldn't find
remote ref HEAD`. The option needs git 2.28 or later.

## 3. Attach the nexus

```bash
tsk nexus add file://$HOME/tsk-nexus.git
```

tsk stores the address in its user config. It keeps a bare clone of the nexus in
`~/.local/state/tsk/nexus/`, fetches into that clone on each command, and pushes ledger
changes back to the address.

## 4. Create each repo's ledger

Run this in each repo that tsk manages:

```bash
tsk ledger init --nexus --prefix <PREFIX> --territory <territory>
```

- `<PREFIX>` is 3 or 4 capital letters. It starts every mission ID in this repo.
- `<territory>` names a territory in the nexus. The first `init` creates `nexus.json` and
  the territory.
- The command writes the repo's entry to `nexus.json`, creates the ledger, and pushes both
  to the nexus folder. The repo itself and its origin are not changed.
- `init` fetches the repo's origin to check it for an existing ledger, and stops when the
  fetch fails. Git on this machine must be able to fetch that origin.

Check the result:

```bash
tsk ledger status
git -C ~/tsk-nexus.git for-each-ref
```

The nexus has the branches `ledgers/<repo-id>` and `ledgers/<repo-id>-docs`.

## 5. Install the plugins

```bash
claude plugin install tsk@jimbarritt-claude-plugins --scope user
claude plugin install swe@jimbarritt-claude-plugins --scope user
```

## 6. Back up the nexus

The folder `~/tsk-nexus.git` contains every ledger and its full history. Copy the folder to
back it up. To move the ledgers to a nexus on a server later, push the folder's branches
to the new nexus repo.

## Times

Measured on the test machine, with a local nexus:

| Command | Time |
|---|---|
| `tsk mission list` | 0.02 s |
| `tsk task add` | 0.04 s |
| `tsk ledger push` | 0.15 s |
| `tsk ledger status` | 0.05 s |

## Known limits

- **Concurrent writes.** Use tsk 0.69.0 or later. With an earlier release, two writes at
  the same moment in one clone can fail with a Dolt `serialization failure`, or one can
  replace the other with exit 0. From 0.69.0, writes in one clone run one at a time
  (ADR 0034), and a push renumbers a task number that two clones both took.
- **A repo with no origin.** `tsk ledger init` fetches the origin, so a repo with no
  origin is untested.
- **macOS.** The steps were tested on Linux only.
