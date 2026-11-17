# Check prose with Software English

This step is optional. `tsk` does not depend on `swe` or Software English. The `tsk` binary and plugin do not install the check and do not enforce it. The cloud environment script `ops/cloud/setup-environment.sh` installs it for now (M-TSK-ENV T-45). Every step on this page changes your own machine, and none changes `tsk` or its ledger.

The `swe` plugin for Claude Code checks prose against [Software English](https://github.com/jimbarritt/software-english/blob/main/spec/SPEC.md). A git `pre-commit` check blocks a commit of markdown that has no clean check on record. This guide installs the plugin and the check, in one repo or in every repo you own.

## 1. Install the plugin

Install the plugin at user scope:

```bash
claude plugin install swe@jimbarritt-claude-plugins --scope user
```

## 2. Install the check in one repo

Run this slash command in a Claude Code session inside the repo:

```text
/swe:install-commit-hook
```

The command poses a question about a `session-start` entry. Answer no, because that entry commits files into the repo.

The command writes the check to `.git/hooks/pre-commit` of the clone. Every worktree of the clone shares that hook. Nothing is committed.

- An existing `pre-commit` hook is renamed `pre-commit.local` and still runs first.
- A commit that stages a markdown file is blocked unless that exact staged content has a clean verdict from `/swe:lint-file` or `/swe:lint-fix`.
- The verdicts are recorded in `.git/swe/lint-log.ndjson`.

## 3. Install the check in every repo you own

This step needs no per-repo action. A `SessionStart` hook in your user settings runs a script at the start of each session. The script installs the check when both conditions are met:

- The repo has no check yet.
- The owner in the `origin` URL equals your GitHub login, from `gh api user`.

Save the script as `~/.claude/hooks/swe-install-commit-hook.sh`:

```sh
#!/bin/sh
git rev-parse --git-dir >/dev/null 2>&1 || exit 0

hook="$(git rev-parse --git-path hooks)/pre-commit"
if [ -f "$hook" ] && grep -q "swe" "$hook" 2>/dev/null; then
  exit 0
fi

url=$(git remote get-url origin 2>/dev/null) || exit 0
login=$(gh api user -q .login 2>/dev/null) || exit 0
[ -n "$login" ] || exit 0

owner=$(printf '%s' "$url" | sed -E 's#^(git@[^:]+:|ssh://[^/]+/|https?://[^/]+/)##; s#/.*##')
[ "$owner" = "$login" ] || exit 0

installer=$(ls -d "$HOME"/.claude/plugins/cache/*/swe/*/scripts/install-commit-hook.sh 2>/dev/null | sort -V | tail -1)
[ -n "$installer" ] || exit 0

"$installer" >/dev/null 2>&1
exit 0
```

Add the hook to `~/.claude/settings.json`, inside the top-level `hooks` object:

```json
"SessionStart": [{ "hooks": [{ "type": "command", "command": "sh ~/.claude/hooks/swe-install-commit-hook.sh", "timeout": 10 }] }]
```

What the script does:

- It reads the newest installed `swe` version.
- A repo owned by someone else is untouched. A repo owned by an organisation counts as someone else's.
- It runs `gh api user` only in a repo that has no check.
- A cloud session reads the settings of its environment, not yours. An environment set up
  with `ops/cloud/setup-environment.sh` gets its own version of this hook, which installs the
  check in every repo of the session with no owner check. See
  [new-cloud-environment.md](new-cloud-environment.md).

Claude Code reads hooks at startup. The entry applies from the next session.

## 4. Check a file by hand

Check one file and record the verdict:

```text
/swe:lint-file <path>
```

Check one file, fix each finding and check the edit:

```text
/swe:lint-fix <path>
```

## 5. Skip or remove the check

Skip the check for one commit:

```bash
git commit --no-verify
```

Remove the check from a repo. Run this slash command in the repo:

```text
/swe:install-commit-hook uninstall
```

The command runs the installer with `--uninstall`. The installer is `scripts/install-commit-hook.sh` in the installed plugin directory, and it takes an optional repo path:

```bash
install-commit-hook.sh --uninstall [<repo-path>]
```

## Limits

- The check applies to markdown files only.
- A cloud session has no check, unless the repo commits the `session-start` entry or its environment runs `ops/cloud/setup-environment.sh`.
- A file with old violations is blocked on its next staged change, until it has a clean verdict.
