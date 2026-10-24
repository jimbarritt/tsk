# 25. A documents hook point, and no git hooks in the ledger worktrees

Date: 2026-10-24

## Status

Accepted. Decided by Jim on 2026-10-24 after a field test of 0.26.0. ADR 0017 (the ledger
is carried by plain git) and ADR 0020 (background publish) stand.

## Context

A field test found that `tsk ledger push` stopped before it pushed anything in a repo that
had a git `pre-commit` hook. The documents worktree and the ledger transport worktree are
linked worktrees of the managed clone of the code repo. Their common git directory is the
code repo's `.git`, so git ran the code repo's hooks for the commits, merges, checkouts and
pushes `tsk` made there. The hook, a lint of staged Markdown, rejected the ledger's
documents. The `Stop` hook's background publish failed the same way, and its warning ended
at the colon, because `tsk` kept the failing command's stderr out of the result record, and
the hook had written its findings to stdout.

Two needs followed. The code repo's git hooks must not apply to the ledger. And a repo
must be able to check the ledger's documents as part of the push flow, with a lint tool of
its choice, without `tsk` depending on that tool or naming it.

## Decision

1. **Git hooks are off in every git process `tsk` starts for the ledger.** The `Git`
   helper of `cli/src/ledger/git.rs` starts git with `core.hooksPath` set to `/dev/null`
   (`NUL` on Windows), through the `GIT_CONFIG_COUNT` environment entries of
   `cli/src/gitenv.rs` and not through `-c`, so the arguments stay the plain ones and the
   setting reaches any git process that git starts. One place covers the commits, merges, rebases, checkouts, worktree
   changes and pushes in the documents worktree, the transport worktree and the managed
   clone. `--no-verify` is not enough, because it skips only `pre-commit` and
   `commit-msg`. The one helper that runs in the user's own code worktree,
   `Git::in_code_worktree`, keeps the hooks.
2. **`tsk` owns a documents hook point.** An executable script named `pre-docs-commit` is
   the hook. `tsk` does not know what it does and has no dependency on any lint tool. Any
   program can implement it.
3. **The interface.**
   - Arguments: the changed document paths, relative to the documents worktree, one
     argument each. A deleted document is not passed.
   - Current directory: the documents worktree. Environment: `TSK_DOCS_WORKTREE`, the
     absolute path of that worktree.
   - Run at `tsk ledger doc put <path>`, after the write, with that path. A non-zero exit
     makes `doc put` exit 1 and the document stays written locally.
   - Run at `tsk ledger push` and the background publish, before the documents commit,
     once, with every document changed since the last push (the documents
     `tsk ledger status` lists). A non-zero exit stops the push before anything is
     pushed and exits 1. Local state stays.
   - The hook's output passes through unchanged. In a failed background publish it is in
     the result record, and the next command's warning shows its first 20 lines.
   - `TSK_SKIP_DOC_CHECK=1` skips the hook at both points, and `tsk` says so on stderr.
   - A hook file that cannot start is an error that names its path. No file means nothing
     runs.
4. **The hook is looked up in four places, and the first that exists wins.**
   1. The file the environment variable `TSK_DOCS_HOOK` names. Set to a path that is
      not a file, it is an error that names the path.
   2. Per clone: `<state>/repos/<clone-id>/hooks/pre-docs-commit`, in the state directory
      next to `ledger-db`.
   3. Per user: `${XDG_CONFIG_HOME:-$HOME/.config}/tsk/hooks/pre-docs-commit`.
   4. In the repo: `.tsk/hooks/pre-docs-commit` at the root of the code worktree, where
      `.tsk/config.toml` is. A ledger kept in a nexus uses the hook of the repo whose
      ledger it is.

   A place with no file is skipped. A file that exists and is not executable is an error
   that names its path. `tsk config docs-hook` prints the script that applies and the
   place it came from, or `none`.

   The repo place alone is not enough. A person who works in a repo they do not control
   cannot commit `.tsk/hooks/pre-docs-commit` to it, and an untracked file there is
   noise they cannot add to `.gitignore` either. The clone, user and environment places
   let that person check the ledger documents of any repo, for one clone, for every repo,
   or for one command. The repo place stays for a repo that wants every contributor
   checked.
5. **A failed publish shows its cause.** The failure text of a git command includes its
   stdout as well as its stderr, and the warning of a failed background publish shows the
   first 20 lines of the error.

## Consequences

- A repo can enforce a rule on its ledger documents, for example the lint of its prose,
  with a one-line shell script. `swe` or any other tool plugs in through that file.
  `tsk` carries no name of such a tool.
- The hook runs in the push flow, so an agent that put a document with `TSK_SKIP_DOC_CHECK=1`
  meets the same findings at the push. The skill tells agents not to skip.
- A document that was changed before the repo added the hook is checked at the next push.
- The code repo's `pre-push`, `pre-commit` and every other git hook stay with the code.
  A repo that wants a check on the ledger uses the hook point, not a git hook.
- A hook that is slow slows each push. It runs once for the whole set of changed
  documents.
- A `post-checkout` or `reference-transaction` hook of the code repo no longer sees the
  ledger's refs.
