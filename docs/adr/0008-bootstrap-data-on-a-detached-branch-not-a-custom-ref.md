# 8. Bootstrap data lives on a detached branch, not a custom git ref

Date: 2026-09-15

## Status

Accepted.

## Context

The original design (see `docs/domain/bootstrap-rationale.md` and ADR history)
held tsk's own task and mission tracking in a custom git ref, `refs/tsk/bootstrap`,
rather than a branch. The intent: keep this data out of `git branch -a`, out of a
plain clone's branch list, and out of GitHub's PR-target dropdown, since it is
data, not a line of development. Agents accessed it via a fixed, detached
`git worktree`, fetching and pushing the ref directly with plain git commands.

This broke the first time an agent tried to push a change to it from a Claude
Code cloud session (`claude.ai/code`, "Claude Code Remote"):

```
error: RPC failed; HTTP 403 curl 22 The requested URL returned error: 403
```

Investigation, in order:

1. **Ruled out a git/proxy transport problem.** The cloud sandbox's outbound
   HTTPS agent proxy reported no relay failures, and TLS negotiated fine.
2. **Ruled out the worktree itself.** From that exact worktree, `git push` to
   push a brand-new branch (`refs/heads/tmp-worktree-push-test`) succeeded
   immediately, and an existing branch (`main`) both received a normal push
   and, separately, a fresh pull, without issue. Only the custom ref failed.
3. **Ruled out it being about "which repo" in a multi-repo session.** The
   session that hit this had `tsk` and `tsk-nexus` both attached, prompting the
   suspicion that repo count was the cause. It was not: the identical push,
   run again from a clean single-purpose worktree, still failed the same way.
4. **Tried deleting a branch, as a second data point.** `git push origin
   --delete <branch>` also returned HTTP 403, from the same worktree that had
   just successfully created that branch. So the restriction wasn't specific
   to the `refs/tsk/*` namespace either: any ref *deletion*, and any *update*
   to a ref outside `refs/heads/*`, both failed; ref *creation* under
   `refs/heads/*` and ordinary branch pushes both succeeded.
5. **Suspected a missing Claude GitHub App permission** (e.g. a "push custom
   refs" scope) and searched for it. Found a precedent, not a match:
   [anthropics/claude-code#61307](https://github.com/anthropics/claude-code/issues/61307)
   documents Claude Managed Agents' internal git proxy restricting pushes to
   only the `claude/<session-id>` branch namespace, closed "not planned" by
   Anthropic as an intentional, undocumented, non-configurable restriction with
   no user-facing fix. Different product, different exact rule, but the same
   shape of problem: an Anthropic-side proxy gating which refs a token may
   write, independent of what the GitHub App itself is scoped to.
6. **Confirmed directly**, rather than staying on inference: called the GitHub
   REST API directly (`POST /repos/{owner}/{repo}/git/refs`) with `curl`, no
   manually-supplied credentials (this sandbox injects them transparently).
   The response was not from GitHub at all:

   ```json
   {"message":"Write access to this GitHub API path is not permitted through this proxy."}
   ```

   This is Anthropic's own proxy, refusing the write before it ever reaches
   GitHub's permission model. It rules out a GitHub App permissions fix
   entirely: there is no scope to grant that changes this, because GitHub is
   never consulted.

## Decision

Move the bootstrap data store off the custom ref and onto an ordinary branch,
`tsk/bootstrap`, which this sandbox has proven it can push, update, and fetch
via plain git commands. Everything else about the design is unchanged:

- Still materialised at a fixed, well-known worktree path
  (`$(git rev-parse --git-common-dir)/tsk/bootstrap-ref-wt`), created with
  `git worktree add --detach`, and refreshed in place with `git reset --hard`.
- Still never checked out in the main working copy; the detached worktree keeps
  it isolated from whatever branch a session's own work is on.
- `ops/local/fetch-bootstrap-ref.sh` and `ops/local/push-bootstrap-ref.sh` keep
  their names and interface; only the ref they fetch/push
  (`refs/heads/tsk/bootstrap` in place of `refs/tsk/bootstrap`) changed.
- The `SessionStart` hook and `CLAUDE.md` instructions are otherwise unaffected.

The pre-existing history was carried across losslessly: the branch was created
directly from the tip of the old custom ref's commit history, not rebuilt from
scratch.

## Consequences

- `tsk/bootstrap` now appears in `git branch -a`, comes down with a plain
  `git clone`, and appears in GitHub's branch list and PR-target dropdown. It is
  no longer invisible by construction. Isolation from real development branches
  is now by naming convention and by the fact that tooling only ever touches it
  through the fetch/push scripts and the fixed worktree, not by the ref
  namespace hiding it.
- The old custom ref, `refs/tsk/bootstrap`, is left orphaned on `origin`. An
  agent cannot delete it (ref deletion hits the same proxy restriction as ref
  creation outside `refs/heads/*`); a human with direct repo access can remove
  it via the GitHub UI or API if wanted. It is otherwise harmless to leave in
  place.
- General lesson for any future tsk design: from a Claude Code cloud session,
  git state can only be written to `refs/heads/*` (ordinary branches). Custom
  ref namespaces cannot be written by any path available in that sandbox: not
  plain `git push`, not the GitHub REST API called directly, and not the GitHub
  MCP connector's own tool set (which exposes branch, PR, and file operations
  only, nothing for arbitrary refs). A local, non-cloud Claude Code session may
  not have this restriction, since it does not route git through this proxy,
  but that was not tested here.

## References

Investigated and decided in a 2026-09-15 session. Precedent:
[anthropics/claude-code#61307](https://github.com/anthropics/claude-code/issues/61307).
