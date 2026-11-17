# Keep the nexus on local disk

Use this page to run tsk on a machine that keeps every ledger on its own disk.

The set-up differs from a nexus on GitHub in one way: the nexus repo is a bare git repo in
a folder on the machine, and tsk gets its `file://` address in place of an `https://` URL.
Git clones from a `file://` address and pushes to it the same way as with a server, so every
tsk command then works as normal.

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
mkdir -p ~/.local/share/tsk
chmod 700 ~/.local/share/tsk
git init --bare --initial-branch=main ~/.local/share/tsk/nexus.git
```

These commands create the local equivalent of an empty repo on GitHub.

- `~/.local/share/tsk` is the XDG data directory for tsk, the place for files that a user
  keeps. The nexus is the only copy of every ledger on the machine, so this guide puts it there. tsk
  keeps its ledger stores and a clone of the nexus in `~/.local/state/tsk/`, which it
  can rebuild from the nexus.
- `chmod 700` lets only your user account read the folder. The ledgers can contain
  sensitive planning records.
- `--bare` creates a repo with no checked-out files. Git accepts a push to any branch of a
  bare repo, so tsk can push to it.
- `--initial-branch=main` sets the repo's default branch to `main`. Without the option, git
  uses the setting `init.defaultBranch` from your git config, and `master` when that is
  not set. tsk writes `nexus.json` to `main` and reads the default branch, so the two must
  match. Set the branch here, so the result does not depend on a config setting. GitHub
  sets `main` as the default branch of a new repo in the same way.
- The option needs git 2.28 or later.

## 3. Attach the nexus

```bash
tsk nexus add file://$HOME/.local/share/tsk/nexus.git
```

This is the one command that differs from a nexus on GitHub: the address is a `file://`
path. tsk stores the address in its user config. It keeps a bare clone of the nexus in
`~/.local/state/tsk/nexus/`, fetches into that clone on each command, and pushes ledger
changes back to the address.

## 4. Install the plugins

```bash
claude plugin install tsk@jimbarritt-claude-plugins --scope user
claude plugin install swe@jimbarritt-claude-plugins --scope user
```

Install the plugins before the first Claude session in a repo: the session uses the tsk
skills to create the ledger and to import old plan files.

## 5. Create each repo's ledger

Create a repo's ledger by hand, or let a Claude session create it.

### By hand

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
git -C ~/.local/share/tsk/nexus.git for-each-ref
```

The nexus has the refs `ledgers/<repo-id>` and `ledgers/<repo-id>-docs`, under `refs/tsk/`.
A new ledger outside a Claude Code cloud session uses these custom refs, which `git branch`
does not list (ADR 0016).

Then set the repo's push workflow, `main` or `pull-request`. `tsk ledger init` does not
set it. `tsk harness new` reads it from the repo's entry in `nexus.json`, and states it in
the first prompt of each new session:

```bash
tsk nexus set-repo <repo-id> --workflow main
```

`<repo-id>` is the id that `tsk ledger init` printed, and `tsk nexus list` shows it.

### With a Claude session, which can also import old plan files

1. In the repo, start `claude`. Use plain `claude` for the first session, not
   `tsk harness new`: `harness new` lists the threads, robots and missions of the ledger,
   and the repo has no ledger yet.
2. At session start the agent runs `tsk ledger status`, which names `tsk ledger init` as the
   next step. The agent then runs the `/tsk:init-ledger` skill, which prompts you for the
   place to keep the ledger and for the prefix. Choose the nexus.
3. To import old plan files, ask the agent to read `tsk guide import` and follow it. By the
   guide's rules, the agent shows you how it maps the plan to missions and tasks before it
   writes a record. The import does not change the plan files.
4. Ask the agent to set the repo's push workflow with `tsk nexus set-repo <repo-id>
   --workflow main`, or `pull-request`, as in the steps by hand above.
5. From the next session on, start each piece of work with `tsk harness new`.

## 6. Back up the nexus

The folder `~/.local/share/tsk/nexus.git` contains every ledger and its full history. On
this machine it is the only copy, so a lost disk loses every ledger. The ledgers can also
contain sensitive planning records, so a backup is a copy of sensitive data. Keep each
backup copy only where you would keep the ledgers themselves.

- Check that your backup tool includes `~/.local`. Some tools skip hidden folders.
- Check where your backup tool writes each copy: an external disk, a network disk or a cloud service.
  A backup to a cloud service sends the ledgers off the machine.
- Encrypt every backup copy, for example with an encrypted Time Machine disk on macOS.
- Keep the folder out of any folder that a sync service copies, such as iCloud,
  Dropbox or OneDrive.
- To copy it by hand, copy the whole folder to an encrypted disk:

  ```bash
  cp -R ~/.local/share/tsk/nexus.git /Volumes/<encrypted disk>/tsk-nexus.git
  ```

To move the ledgers to a nexus on a server later, push the folder's branches to the new
nexus repo. That push also sends the ledgers off the machine.

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
