# Checking ledger documents

A ledger has documents as Markdown on a second ref, `tsk/docs`. Records go through `tsk`
commands and have no free text to check. Documents are prose. This guide shows how to
run a check on them, for example a prose lint, every time one is written or pushed.

`tsk` checks nothing by itself. It runs a script that you provide, called the documents
hook, and the script defines what to check.

## Why a git hook does not do this

`tsk` keeps the documents in a worktree of your code repo. A git hook of the code repo,
such as `pre-commit`, would apply to that worktree, so `tsk` runs its own git commands
with git hooks disabled. A code repo hook never blocks a ledger push. The documents hook
is the replacement: a separate script, owned by `tsk`, that only runs for documents.

## Set it up

A hook is an executable file named `pre-docs-commit`. `tsk` looks for it in four places and
uses the first one that exists:

| Order | Place | Use it when |
|---|---|---|
| 1 | The file that the environment variable `TSK_DOCS_HOOK` names | You want to run one command with a different hook. |
| 2 | `<state>/repos/<clone-id>/hooks/pre-docs-commit` | You want the check for one clone only. `tsk ledger path` prints the ledger store path in the same directory. |
| 3 | `~/.config/tsk/hooks/pre-docs-commit` (or `$XDG_CONFIG_HOME/tsk/hooks/pre-docs-commit`) | You want the check for every ledger you work with. This is the right place for a repo you do not control. |
| 4 | `.tsk/hooks/pre-docs-commit` in the repo | You control the repo and want every contributor checked. Commit the file. |

A repo you do not control cannot take a file at place 4, and you cannot add `.tsk` to its
`.gitignore`. Use place 3 or place 2 there. Nothing is added to the repo.

To set it up for every ledger you work with:

```bash
mkdir -p ~/.config/tsk/hooks
cp <your-script> ~/.config/tsk/hooks/pre-docs-commit
chmod +x ~/.config/tsk/hooks/pre-docs-commit
tsk config docs-hook
```

`tsk config docs-hook` prints the script that applies and where it came from:

```
docs hook: /Users/you/.config/tsk/hooks/pre-docs-commit
source: user (~/.config/tsk/hooks/pre-docs-commit)
```

It prints `docs hook: none` when no script applies. A file that exists but is not
executable is an error that names its path.

## What the script receives

- **Arguments:** the changed document paths, relative to the documents worktree, one
  argument each. A deleted document is not passed.
- **Current directory:** the documents worktree.
- **Environment:** `TSK_DOCS_WORKTREE` is the absolute path of the documents worktree.
- **Exit code:** 0 passes. Any other code blocks.
- **Output:** what the script prints on stdout and stderr shows unchanged.

A script that checks Markdown only should pass for other files:

```sh
#!/bin/sh
# Check each Markdown document. Replace my-lint with your tool.
status=0
for path in "$@"; do
  case "$path" in
    *.md) my-lint "$path" || status=1 ;;
  esac
done
exit $status
```

## When it runs

- **`tsk ledger doc put <path>`** runs the script on that path after it writes the
  document. If the script exits non-zero, `doc put` exits 1 and prints the findings. The
  document stays written, so you correct it and put it again.
- **`tsk ledger push`** runs the script once, before the documents commit, with every
  document that changed since the last push. If the script exits non-zero, the push stops
  before it pushes anything, exits 1 and prints the findings. Nothing is lost: the
  records and documents stay in the local ledger store. The background publish after a
  turn (the `Stop` hook) does the same, and the next `tsk` command shows the first 20
  lines of the findings as a warning.
- `tsk thread start`, `pause` and `stop` push too, so they run the script as well.

## Skip the check

`TSK_SKIP_DOC_CHECK=1` skips the script at both points, and `tsk` reports it on stderr. Use it
only to get a push through that cannot wait. The next push still checks the documents
that have not been checked. An agent must not skip the check: it fixes the document.

## Common failures

| What you see | What it means | What to do |
|---|---|---|
| `error: the documents hook <path> failed (exit status 1)` and findings | The script rejected a document. | Fix the document. Put it again, or push again. |
| `error: ... is not executable` | The script exists and cannot start. | `chmod +x` the path in the message. |
| `error: TSK_DOCS_HOOK names <path>, which is not a file` | The variable names a missing file. | Correct the path, or unset the variable. |
| The warning `the last background publish failed` with findings | The `Stop` hook's push met a failing check. | Fix the documents, then run `tsk ledger push "<message>"`. |
| A push stops, and you did not expect a check | A hook exists in one of the four places. | `tsk config docs-hook` prints which one. |

## Linting with Software English

The `swe` plugin for Claude Code can supply this script, so that ledger documents get the
same check as the files in your repo. See the plugin's own instructions. For a repo you
control, a script at place 4 shares the check with everyone. For any other repo, put it at
place 3 once.

## Related

- [ADR 0025](../adr/0025-a-documents-hook-point-and-no-git-hooks-in-the-ledger-worktrees.md):
  the reasons and the exact interface.
- [getting-started.md](getting-started.md): the ledger commands.
