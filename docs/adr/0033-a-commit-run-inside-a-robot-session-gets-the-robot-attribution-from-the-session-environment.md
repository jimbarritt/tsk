# 33. A commit run inside a robot session gets the robot attribution from the session environment

Date: 2026-11-15

## Status

Accepted. ADR 0030 (an actor belongs to a session) and ADR 0032 (a thread has one open
session and a robot works in many) remain in force. This ADR states how a commit gets the
robot's git identity and signing settings.

## Context

A robot is an actor that `tsk actor register --kind robot` registers, and an agent session
works as one.

`tsk thread session-start` wrote the robot name, email, signing key and gpg settings with
`git config --worktree`, and a `tsk.attribution` marker beside them. When the session runs
in the main clone (the original checkout, not a linked worktree), those values are saved in
`.git/config.worktree` of the main clone. They stay after the session ends. A person's later
commit in that clone takes the robot name, email and signature. This happened twice. One
commit was authored and signed by the robot, and the person could not re-sign it, because
the session had no access to the person's key.

A linked worktree (made with `git worktree add`) has its own `config.worktree`. A person
who commits by hand there also takes the robot identity.

Checks run on 2026-11-15:

- A SessionStart hook exports variables to later commands through the file named by
  `CLAUDE_ENV_FILE` (https://code.claude.com/docs/en/hooks).
- A live `claude -p` test showed the exported variables in a later Bash command and in a
  subagent's Bash command.
- Git 2.54 honours `GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_n` and `GIT_CONFIG_VALUE_n` for
  `commit.gpgsign`, `gpg.format`, `gpg.ssh.program` and `user.signingkey`. A test commit
  called the configured signer while `.git/config` contained no gpg line.
- Not verified: whether a command run with the `!` prefix sees the variables, and whether
  Claude Code clears the file named by `CLAUDE_ENV_FILE` between session sources.

## Decision

1. **A commit made by the agent inside a robot session has the robot as author, committer
   and signer.** A git command a person runs in their own terminal has that person as sole
   author and uses the person's own git config. This applies in any checkout: a harness
   worktree and the main clone. tsk writes no robot setting to any git config file.

2. **The SessionStart hook writes the robot attribution to the file named by
   `CLAUDE_ENV_FILE`.** It writes `export` lines for `GIT_AUTHOR_NAME`,
   `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME` and `GIT_COMMITTER_EMAIL`. It writes
   `GIT_CONFIG_COUNT` with a `GIT_CONFIG_KEY_n` and `GIT_CONFIG_VALUE_n` pair for each of
   `commit.gpgsign`, `gpg.format`, `gpg.ssh.program` and `user.signingkey`. It also writes
   `TSK_SESSION`, the URN of the session.

3. **The hook replaces its own lines on every source.** The sources are startup, resume,
   clear, compact and fork.

4. **A supervised session adds a co-author.** A session is supervised when a person joined
   it with `tsk actor join`. The commit-msg hook then adds a `Co-authored-by` trailer that
   names the person. An unsupervised session, such as a cloud session, gets no trailer.

5. **The `git config --worktree` writes are removed.** This covers the robot name, email,
   signing key, gpg settings and the `tsk.attribution` marker. Session start removes any
   robot set-up that an older tsk left in a worktree config.

6. **A command a person runs with the `!` prefix inside the session has the robot
   attribution when it runs with the session environment.** A person who wants their own
   attribution commits in a separate terminal.

| Who commits | Author, committer and signature | Trailer |
| --- | --- | --- |
| Agent, supervised session | Robot | Names the person |
| Agent, unsupervised session | Robot | None |
| Person, own terminal | The person | None |

The agent never signs with a person's key.

## Alternatives

- **Keep the config-file write and remove it at session end.** A session that crashes
  leaves the robot identity in the config file.
- **Apply the robot identity only in linked worktrees, and ask a person in the main clone
  whether the robot is a co-author.** A person's hand commit in a linked worktree still
  takes the robot identity. The agent would also need to sign as the person.
- **Pass `git -c` options on each commit that tsk runs.** An agent's own `git commit` does
  not go through tsk.

## Consequences

- The commit-msg hook reads `TSK_SESSION` from the session environment and looks up in
  the ledger whether a person joined that session. `tsk actor join` runs after the session starts, so
  the SessionStart hook cannot record the answer.
- A file `tsk-robot-name` in the worktree git directory stores the robot name for tsk's own
  ledger commits. It is not a git config file.
- Every run of the SessionStart hook rewrites its lines in the file named by
  `CLAUDE_ENV_FILE`.
- `tsk config attribution` keeps its settings.
