# A new nexus on a clean machine

These steps set up tsk on a machine that has never run it, with a new nexus that holds
the ledger of a repo you work in. Use them when you cannot, or do not want to, push a
ledger branch to the repo itself: a work repo, for example.

The result:

- a nexus repo, holding `nexus.json` on its default branch and one ledger branch,
  `ledgers/<repo-id>`, per managed repo;
- the `tsk` binary and the `tsk` Claude Code plugin on the machine;
- a managed repo whose ledger lives in the nexus, ready for a first mission.

You need push access to the nexus repo. You do not need push access to the managed repo.

## 1. Install the prerequisites

- git, with credentials that can push to the host the nexus lives on.
- The Rust toolchain. See [installation.md](installation.md#prerequisites).
- Claude Code.

## 2. Install tsk

```bash
cargo install tsk-bin --locked
tsk --version
```

## 3. Create the nexus repo

A nexus is an ordinary git repo with a `nexus.json` at its root. It holds no code. Create
it as a private repo on any git host that the machine can push to.

On GitHub, with the GitHub CLI:

```bash
gh auth login
gh repo create <owner>/<name>-nexus --private --clone
cd <name>-nexus
```

On another host, or without the GitHub CLI: create an empty private repo in the host's
web interface, with no README, licence or `.gitignore`, then:

```bash
mkdir <name>-nexus
cd <name>-nexus
git init -b main
git remote add origin <nexus url>
```

### What the nexus repo holds

One file, `nexus.json`, on the default branch. You write it in step 4, and
`tsk nexus register-repo` adds each managed repo's entry in step 6. tsk creates the ledger
branches. Nothing else is required.

```
<name>-nexus
├── main                     branch you write
│   └── nexus.json           the index of territories and repos
├── ledgers/work-api         branch tsk creates in step 7
│   ├── .tsk-ledger.toml
│   ├── index.md
│   ├── missions/
│   ├── threads/
│   └── external-events/
└── ledgers/<other-id>       one branch per managed repo with "ledger": "nexus"
```

A `README.md` on `main` is optional. tsk does not read it.

The nexus URL is the repo's clone URL, in HTTPS or SSH form, for example
`https://github.com/<owner>/<name>-nexus`. Step 5 uses the same URL. tsk fetches and
pushes with the machine's own git credentials, so use the form those credentials work
with.

## 4. Write `nexus.json`

In the nexus repo, create `nexus.json` at the root with one territory and no repos. Pick
the territory's ID and name:

```json
{
  "version": 1,
  "territories": [
    {
      "id": "work",
      "name": "Work",
      "repos": []
    }
  ]
}
```

Commit and push it to the nexus's default branch:

```bash
git add nexus.json
git commit -m "Add nexus.json"
git push origin HEAD
```

`"territories": []` also works: step 6 then creates the territory, with
`--territory work --territory-name Work`.

## 5. Attach the nexus

```bash
tsk nexus add <nexus url>
tsk nexus list
```

`tsk nexus add` writes the nexus URL to `~/.config/tsk/config.toml`. A machine holds one
nexus. `tsk config attach-nexus <nexus url>` does the same.

`tsk nexus list` prints the config path and the nexus URL, fetches the nexus and prints
each territory in `nexus.json` with its repos. After step 6:

```
config: /home/me/.config/tsk/config.toml
nexus: https://github.com/<owner>/<name>-nexus
territory: work (Work)
  work-api  url: <origin url>  ledger: nexus
```

`tsk nexus list --json` prints the same as one JSON object. When the nexus cannot be
fetched and no copy is held from an earlier fetch, the command exits 1 with the error.

## 6. Register the managed repo

In the managed repo:

```bash
cd <path-to-managed-repo>
tsk nexus register-repo
```

The command reads the repo's `origin` URL, fetches the nexus's default branch, adds an
entry to `nexus.json`, and commits and pushes it with the machine's git credentials. It
prints the entry and the next step:

```
registered in territory work: {"id":"work-api","ledger":"nexus","url":"<origin url>"}
pushed <commit> to refs/heads/main of <nexus url>
next: run tsk ledger fetch in this repo
```

| Field | Value |
|---|---|
| `id` | The repo ID: `--id <id>`, or the repo name in lower case with each run of other characters as `-`. Lower case letters, digits and `-`, starting with a letter or digit. The ledger branch is `ledgers/<id>`. |
| `url` | The managed repo's `origin` URL. The HTTPS and SSH forms of one URL match each other. |
| `ledger` | `--ledger nexus`, the default, holds the ledger in the nexus. `--ledger repo` holds it on `tsk/ledger` in the managed repo. |

With more than one territory in `nexus.json`, name one with `--territory <id>`. With none,
`--territory <id> --territory-name <name>` creates it.

A second run prints `already registered` and changes nothing. An ID or URL that another
entry already uses stops the command with exit status 1 and an error naming that entry.

In a Claude Code session with the plugin installed, `/tsk:register-repo` runs the same
command.

### By hand

Without the command, read the managed repo's `origin` URL:

```bash
git -C <path-to-managed-repo> config --get remote.origin.url
```

Add an entry to the territory's `repos` in `nexus.json`, replacing `<origin url>` with
that URL:

```json
{ "id": "work-api", "url": "<origin url>", "ledger": "nexus" }
```

Then commit and push it to the nexus's default branch, as in step 4.

## 7. Create the ledger

Run this in the managed repo, after step 6:

```bash
cd <path-to-managed-repo>
tsk ledger fetch
```

The command finds the `work-api` entry, writes `work-api` to `.git/tsk-repo-id`, and
reports that `ledgers/work-api` does not exist yet. It creates a new ledger in the ledger
worktree: `.tsk-ledger.toml` and an `index.md` stub. It prints the ledger worktree path.

Push it to create the branch in the nexus:

```bash
tsk ledger push "Create the ledger"
git ls-remote <nexus url> refs/heads/ledgers/work-api
```

The push creates the branch only when it does not exist, so it never overwrites a
ledger another machine created.

## 8. Install the Claude Code plugin

In the managed repo:

```bash
tsk install-plugin claude-cli --scope local
```

The command adds the `jimbarritt/claude-plugins` marketplace, installs the `tsk` plugin,
refreshes the marketplace and updates the plugin, through the `claude` CLI. See
[installation.md](installation.md#installation). The same, by hand:

```bash
claude plugin marketplace add jimbarritt/claude-plugins
claude plugin install tsk@jimbarritt-claude-plugins --scope local
```

`--scope local` enables the plugin for this repo only and writes nothing that is
committed. At user scope, the plugin's `SessionStart` hook runs in every repo on the
machine and creates an in-repo ledger worktree in each one.

## 9. Start a session

```bash
claude
```

The plugin's `SessionStart` hook checks for `tsk` at the version the plugin requires and
installs it when it is missing. It then runs `tsk thread session-start`, which fetches the
ledger, exports its path as `$TSK_LEDGER_WT`, and asks for a thread binding.

## 10. Add a first mission

1. Write a mission briefing under `missions/` in the ledger worktree. The template is in
   [mission-briefing-template.md](../domain/mission-briefing-template.md).
2. List it in the ledger's `index.md`.
3. Push the ledger:

   ```bash
   tsk ledger push "Add the first mission"
   ```

4. In Claude Code, run `/tsk:start-thread <mission>`.

## Another managed repo

Run steps 6, 7 and 8 in that repo.

## A repo with no remote URL

Run step 6 with `--local`:

```bash
tsk nexus register-repo --local
```

The entry carries `"local": "<machine name>"` in place of `url`. The machine name is
`$TSK_MACHINE_NAME` when set, otherwise the output of `hostname`. Other machines skip the
entry. The ID defaults to the working tree's directory name, and the command writes it to
the repo's git directory, `$(git rev-parse --git-common-dir)/tsk-repo-id`, where step 7
reads it.

By hand: give the entry `"local": "<machine name>"` in place of `url`, push it, and write
the ID before step 7:

```bash
echo work-api > "$(git rev-parse --git-common-dir)/tsk-repo-id"
```

## When `tsk ledger fetch` ran before the entry existed

Without a matching entry, `tsk ledger fetch` creates an in-repo ledger worktree. It stays
in-repo after the entry is added. Read its path with `tsk ledger path`, move that
directory aside, run `git worktree prune` in the managed repo, then run step 7 again.
