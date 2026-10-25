# Intel: stopping cloud sessions from creating branches

Date: 2026-10-22. Raised by Jim. Linked to M-TIDY.

Question: how to stop a Claude Code cloud session creating a `claude/<name>` branch in
`jimbarritt/tsk`, where the rule is to work on `main`.

Evidence classes used below: **official** (Anthropic documentation, read in full on
2026-10-22), **observed** (this container, 2026-10-22), **tool schema** (the
`create_session` tool in this session), **reported** (a public GitHub issue on
`anthropics/claude-code`; the reporter's claim, not verified), and **general** (not read
for this note). Nothing below was tested by changing a setting.

## Summary

- No documented setting stops the branch. The session setup creates a local branch
  `claude/<adjective-surname>-<suffix>` at every container start, and the platform's
  system prompt names it as the branch to develop on.
- The branch is local until a session pushes it. In this repo no session has pushed one:
  `git ls-remote --heads origin` lists no `claude/*` branch.
- The proxy does not stop a push to `main`. This repo's own `CLAUDE.md` rule ("Work on
  `main`") has been followed in this session, with the platform's instruction overridden.
- Four controls are worth review. Two are untested, and one needs a change on GitHub.

## What happens

**Observed.** The reflog of this clone, 2026-10-22:

```
06:33:16 checkout: moving from d9a4c63... to main
06:33:17 checkout: moving from main to claude/vigilant-edison-fdcvvk
         claude/vigilant-edison-fdcvvk: branch: Created from HEAD
```

The first line is the startup step of `ops/local/claude-session-start.sh`. The second is
the session setup, before any command of the agent ran. The agent's `git checkout main`
came at 06:39:28.

**Observed (the session prompt).** The platform prompt for this session says: "DEVELOP all
your changes on the designated branch above", "CREATE the branch locally if it doesn't
exist yet", "PUSH to the specified branch when your changes are complete", and "NEVER push
to a different branch without explicit permission". `CLAUDE.md` overrides this ("Work on
`main`... This overrides any session instruction naming a designated branch").

**Observed (the stop hook).** `/root/.claude/stop-hook-git-check.sh` is a platform file,
rewritten at session start (modified 2026-10-22 07:11). It exits 2 with a message to push
when the working tree has uncommitted or untracked files, or when the current branch has
unpushed commits against `origin/<branch>` (or `origin/HEAD` when that ref does not
exist). On `main`, with `origin/main` present, a push to `main` satisfies it.

**Official.** "Each task gets its own session and its own branch", and "when Claude reaches
a stopping point, it pushes its branch to GitHub" (web quickstart). The GitHub proxy
"rejects branch deletions and pushes of anything other than a branch, such as a tag. It
doesn't limit which branches a push can update. To do that, use branch protection rules or
rulesets on GitHub." (cloud environments, GitHub proxy.) The cloud page documents no
setting for the branch name or for turning branch creation off. In a cloud session
`.claude/settings.json` of the repo is read, and `~/.claude/settings.json` is not.

## Controls

| # | Control | Effect | Evidence | Fit for tsk |
|---|---|---|---|---|
| 1 | A `CLAUDE.md` rule to work on `main` | The agent commits and pushes to `main`. No remote `claude/*` branch is created | Observed in this repo, 2026-10-16 to 2026-10-22 | In place. Reported unreliable elsewhere: the injected block wins in #56865 (closed, not planned) and #91018 (open) |
| 2 | Select `main` as the base branch in the session picker | Reported to remove the message "Claude will push directly to <branch>" that other base branches show | Reported, #91018 comment of 2026-10-02 | Untested. Applies to sessions started from the browser |
| 3 | `outcome_branch` on `create_session` | "When set, the session pushes directly to this branch (no session-derived suffix appended)" | Tool schema. Reported as not exposed in the composer, #24535 comment of 2026-08-27 | Untested. Applies to a session started from another session |
| 4 | A GitHub ruleset that blocks creation of `claude/**` refs | A push of a `claude/*` branch fails at GitHub, so none can exist on origin | Official: the proxy defers branch limits to GitHub. Reported: a repo that disallows `claude/` names stops the agent's push and its PR, #24535 comment of 2026-02-10 | Untested. A session that obeys the platform prompt fails on push, loudly. The stop hook may then nag on a local branch |
| 5 | "Allow unrestricted branch pushes" | Reported as a permission on a cloud scheduled task, default off, meaning only `claude/*` branches | Reported, #44949 (closed, not planned): a task pushed to `main` with it off. #24535 comment of 2026-04-10: the prompt still names a `claude/*` constraint with it on. Not found on the official pages read | Meaning unclear. Not a stop control |
| 6 | A branch prefix setting | The Desktop app has one. Feature requests ask for the CLI (#28945, #42365) | Search results only | No web setting found |
| 7 | Delete the remote branch afterwards | Removes a `claude/*` branch that exists | Official: the proxy rejects deletions, so a session cannot. General: GitHub's "Automatically delete head branches" applies to branches of merged pull requests only | Needs a person on GitHub, or a clone with a personal credential |
| 8 | Disable the stop hook | None possible from the repo | Observed: the hook is a platform file rewritten at start. Reported: #103232 (open) describes the hook demanding a push of the session branch after a push to `main`; the reporter's workaround is to delete the local ref `refs/remotes/origin/<session-branch>` | Not needed while work stays on `main` |

## What this means for tsk

- The local `claude/...` branch is harmless. It is created at each container start and
  holds no commits that `main` lacks. Deleting it in a container has no lasting effect.
- A branch reaches origin only when a session pushes it. The risk is a session that obeys
  the platform prompt and ignores `CLAUDE.md`, for example one started with a different
  prompt or from a scheduled task or a spawned session.
- Origin holds two branches of unknown purpose or leftover state: `push-test-2026-10-15`
  (the ref-push test, known) and `tsk/spike-lease` (origin unknown). An earlier listing of
  this clone also held a tracking ref `origin/claude/new-session-rdlapc`, which no longer
  exists on origin.

## Options to review

1. Keep control 1 only. Cheapest. Accepts the risk for sessions that lose the override.
2. Add control 4 on top, a ruleset on `claude/**`. It turns a stray branch into a visible
   push failure. Check first that the ruleset does not block pushes to `main`.
3. Test control 3 with a spawned session, and control 2 with a browser session, then
   record the result here.
4. Delete the leftover branches on GitHub (control 7).

None is decided.

## Not established

- Whether any setting removes the "designated branch" block from the session prompt.
- Whether a ruleset on `claude/**` makes the proxy return a clear error, and what the
  agent does after it.
- What "Allow unrestricted branch pushes" does in an interactive session.
- Who created `tsk/spike-lease` and `origin/claude/new-session-rdlapc`.
- The state of the public issues beyond what their pages showed on 2026-10-22.

## Sources

- Claude Code docs: [Use Claude Code in the cloud](https://code.claude.com/docs/en/claude-code-on-the-web),
  [Get started with Claude Code in the cloud](https://code.claude.com/docs/en/web-quickstart),
  [Configure cloud environments](https://code.claude.com/docs/en/cloud-environments),
  [Settings files and precedence](https://code.claude.com/docs/en/settings).
- Public issues on `anthropics/claude-code`: #56865, #91018, #24535, #44949, #103232, #66109.
- This container: the reflog of `/home/user/tsk`, `/root/.claude/stop-hook-git-check.sh`,
  `git ls-remote --heads origin`, and the `create_session` tool schema.
