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

You write one file, `nexus.json`, on the default branch (step 4). tsk creates the ledger
branches. Nothing else is required.

```
<name>-nexus
├── main                     branch you write
│   └── nexus.json           the index of territories and repos
├── ledgers/work-api         branch tsk creates in step 6
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

## 4. Write `nexus.json`, with an entry for the managed repo

In the managed repo, read its `origin` URL:

```bash
git -C <path-to-managed-repo> config --get remote.origin.url
```

In the nexus repo, create `nexus.json` at the root with this content. Replace
`<origin url>` with the URL from the command above, and pick a territory and repo ID:

```json
{
  "version": 1,
  "territories": [
    {
      "id": "work",
      "name": "Work",
      "repos": [
        { "id": "work-api", "url": "<origin url>", "ledger": "nexus" }
      ]
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `id` | The repo ID. Lower case letters, digits and `-`, starting with a letter or digit. The ledger branch is `ledgers/<id>`. |
| `url` | The managed repo's `origin` URL. The HTTPS and SSH forms of one URL match each other. |
| `ledger` | `"nexus"` holds the ledger in the nexus. `"repo"`, or no field, holds it on `tsk/ledger` in the managed repo. |

Commit and push it to the nexus's default branch:

```bash
git add nexus.json
git commit -m "Add work-api"
git push origin HEAD
```

## 5. Attach the nexus

```bash
tsk config attach-nexus <nexus url>
tsk config show
```

This writes the nexus URL to `~/.config/tsk/config.toml`. A machine holds one nexus.

## 6. Create the ledger

Run this in the managed repo, after step 4 is pushed:

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

## 7. Install the Claude Code plugin

In the managed repo:

```bash
claude plugin marketplace add jimbarritt/tsk
claude plugin install tsk@tsk --scope local
```

`--scope local` enables the plugin for this repo only and writes nothing that is
committed. At user scope, the plugin's `SessionStart` hook runs in every repo on the
machine and creates an in-repo ledger worktree in each one.

## 8. Start a session

```bash
claude
```

The plugin's `SessionStart` hook checks for `tsk` at the version the plugin requires and
installs it when it is missing. It then runs `tsk thread session-start`, which fetches the
ledger, exports its path as `$TSK_LEDGER_WT`, and asks for a thread binding.

## 9. Add a first mission

1. Write a mission briefing under `missions/` in the ledger worktree. The template is in
   [mission-briefing-template.md](../domain/mission-briefing-template.md).
2. List it in the ledger's `index.md`.
3. Push the ledger:

   ```bash
   tsk ledger push "Add the first mission"
   ```

4. In Claude Code, run `/tsk:start-thread <mission>`.

## Another managed repo

Repeat step 4 with a new entry, then steps 6 and 7 in that repo.

## A repo with no remote URL

Give the entry `"local": "<machine name>"` in place of `url`. The machine name is
`$TSK_MACHINE_NAME` when set, otherwise the output of `hostname`. Other machines skip the
entry. Before step 6, write the ID to the repo's git directory:

```bash
echo work-api > "$(git rev-parse --git-common-dir)/tsk-repo-id"
```

## When `tsk ledger fetch` ran before the entry existed

Without a matching entry, `tsk ledger fetch` creates an in-repo ledger worktree. It stays
in-repo after the entry is added. Read its path with `tsk ledger path`, move that
directory aside, run `git worktree prune` in the managed repo, then run step 6 again.
