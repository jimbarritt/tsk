# Keep the nexus on local disk

Use this page to run tsk on a machine that keeps every ledger on a local disk.

The set-up differs from a nexus on GitHub in one way. The nexus repo is a bare git repo in
a folder, and tsk takes its `file://` address, not an `https://` URL.
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

## 2. Create and attach the nexus

```bash
tsk nexus init-local
```

The command lists what it will do and prompts for confirmation before it acts. `--yes` skips the prompt, for
a script or an agent. An argument names another path: `tsk nexus init-local <path>`. The
default path is `nexus.git` in the tsk data directory, `~/.local/share/tsk`, and
`XDG_DATA_HOME` moves it. The command fails on a path that exists and is not an empty folder
or an empty bare repo. It fails while another nexus is attached, unless `--replace` is
given. A nexus that `--replace` replaces keeps its ledgers: they are not moved. After it makes
the folder, the command attaches it, as step 3 describes.

The command resolves `..` and symbolic links in the existing part of the path before it
compares the path with the attached nexus, and attaches the resolved path. When the folder
is a bare repo that is the attached nexus, the command prints
`already set up: <path> is the attached nexus` and exits 0, whether the repo has refs or
not. An empty bare repo keeps its folder, and the command sets its HEAD to `refs/heads/main`.

The command runs the commands below, which create the local equivalent of an empty repo on
GitHub. A machine with an older tsk runs them by hand:

```bash
mkdir -p ~/.local/share/tsk
chmod 700 ~/.local/share/tsk
git init --bare --initial-branch=main ~/.local/share/tsk/nexus.git
```

- `~/.local/share/tsk` is the XDG data directory for tsk, the place for files that a user
  keeps. The nexus is the only copy of every ledger on the machine, so this guide puts it there. tsk
  keeps its ledger homes and a clone of the nexus in `~/.local/state/tsk/`, which it
  can rebuild from the nexus (see [The folders on disk](#the-folders-on-disk)).
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

`tsk nexus init-local` attaches the nexus it makes. By hand, run:

```bash
tsk nexus add file://$HOME/.local/share/tsk/nexus.git
```

This is the one command that differs from a nexus on GitHub: the address is a `file://`
path. tsk stores the address in its user config. It keeps a bare clone of the nexus in
`~/.local/state/tsk/nexus/`, fetches into that clone on each command, and pushes ledger
changes back to the address.

## The folders on disk

This is the whole layout on a machine with a local nexus and two repos, `foo` and `bar`, after
both have a ledger. The names `foo` and `bar` are the entry ids in `nexus.json`.

```
~/.local/share/tsk/                      XDG data directory: the files you keep
└── nexus.git/                           the local nexus, a bare git repo: the only copy of every ledger
    └── refs/                            the refs that `git for-each-ref` lists
        ├── heads/main                   branch with nexus.json, the entry of each repo
        └── tsk/ledgers/
            ├── foo/ledger               the Dolt data of foo
            ├── foo/docs                 the documents of foo
            ├── bar/ledger               the Dolt data of bar
            └── bar/docs                 the documents of bar

~/.local/state/tsk/                      XDG state directory: what tsk rebuilds from the nexus
├── nexus/                               tsk's bare clone of the nexus, fetched on each command
├── harnesses/<clone-id>/                state of the harness instances of one clone
├── repos/<clone-id>/                    per-clone files only: harness names, the robot signing key, hooks
└── ledgers/                             one ledger home for each repo, shared by every clone of it
    ├── foo/                             the ledger home of foo, named by its entry id
    │   ├── git/                         tsk's bare clone of the ledger refs of foo
    │   ├── ledger-db/                   the Dolt database: missions, tasks, decisions, questions
    │   ├── ledger/                      worktree of git/ with the Dolt file remote
    │   ├── docs/                        worktree of git/ with the documents, as Markdown files
    │   ├── scratch/                     input documents that stay out of the ledger
    │   └── push.lock                    one writer at a time in this home
    └── bar/                             the ledger home of bar, with the same folders
        ├── git/
        ├── ledger-db/
        ├── ledger/
        ├── docs/
        ├── scratch/
        └── push.lock
```

- Each repo has one ledger home on the machine, however many clones and code worktrees of it
  you have. They share the home, its lock and its Dolt server.
- The code repos contain no ledger data. `git worktree list` in `foo` lists no ledger folder,
  `git for-each-ref` lists no ledger ref, and tsk adds no ledger object to its object database.
  The repo keeps three small files in `.git/`: `tsk-clone-id`, `tsk-repo-id` and `tsk-ledger-home`.
- Everything under `~/.local/state/tsk/` can be rebuilt from the nexus, except a change you
  made and did not push. Run `tsk ledger push` before you delete a home.
- `tsk ledger path` prints the `docs/` folder of the current repo. Do not edit the other
  folders by hand.
- Before 0.70.0, tsk kept these folders in `~/.local/state/tsk/repos/<clone-id>/` and named the
  nexus refs `ledgers/<repo-id>` and `ledgers/<repo-id>-docs`. The first ledger command of
  0.70.0 moves them, with no step by hand (ADR 0035).

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
  the territory. `--territory-name <name>` sets the display name of a territory that the
  command creates, which is the id otherwise. An existing territory keeps its name.
- `--workflow main` or `--workflow pr` saves the push workflow in the repo's entry. On a
  terminal, without the option, the command prompts for it. With no terminal and no option
  the workflow stays unset, and `tsk ledger status` shows `workflow: not set`.
- The command writes the repo's entry to `nexus.json`, creates the ledger, and pushes both
  to the nexus folder. The repo itself and its origin are not changed.
- `init` fetches the repo's origin to check it for an existing ledger, and stops when the
  fetch fails. Git on this machine must be able to fetch that origin.

Check the result:

```bash
tsk ledger status
git -C ~/.local/share/tsk/nexus.git for-each-ref
```

The nexus has the refs `ledgers/<repo-id>/ledger` and `ledgers/<repo-id>/docs`, under `refs/tsk/`.
A new ledger outside a Claude Code cloud session uses these custom refs, which `git branch`
does not list (ADR 0016).

`tsk harness new` reads the push workflow, `main` or `pull-request`, from the repo's entry
in `nexus.json`, and states it in the first prompt of each new session. `tsk ledger init`
sets it. To change it later:

```bash
tsk nexus set-repo <repo-id> --workflow main
```

`<repo-id>` is the id that `tsk ledger init` printed, and `tsk nexus list` shows it.

### With a Claude session, which can also import old plan files

1. In the repo, start `claude`. Use plain `claude` for the first session, not
   `tsk harness new`: `harness new` lists the threads, robots and missions of the ledger,
   and the repo has no ledger yet.
2. At session start in a repo with no ledger, the agent offers a choice between initialising a ledger
   and working without tsk. With no nexus attached, it offers to create a local nexus
   (`tsk nexus init-local`), to attach a nexus by URL, or to work without tsk. Work without
   tsk prompts for whether to remember the choice, and on yes runs `tsk ledger not-used`. That
   records the choice in the `tsk` field of the repo's nexus entry when the attached nexus
   lists the repo, and in `.git/tsk-not-used` of the clone otherwise.
   `tsk ledger not-used --forget` removes the record. To initialise, the agent runs the
   `/tsk:init-ledger` skill, which prompts you for the place to keep the ledger, the
   prefix and the push workflow. Choose the local nexus.
3. To import old plan files, run `/tsk:migrate-plan` or ask the agent to read `tsk guide import`
   and follow it. By the
   guide's rules, the agent shows you how it maps the plan to missions and tasks before it
   writes a record. The import does not change the plan files.
4. From the next session on, start each piece of work with `tsk harness new`.

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
