# Commit attribution

A commit names its author, its committer, its signer and its trailers. The rules below decide each of them. They do not depend on the harness instance that runs the session. A cloud session, a code worktree and a terminal session follow the same rules.

## Rules

| Who made the commit | Author and committer | Signed with | Trailers |
|---|---|---|---|
| A person, at the command line, with no agent | The person | The person's key | None |
| A robot, in a supervised session | The robot | The robot's key | `Agent-Session`, and `Co-authored-by` naming the supervising person |
| A robot, in an autonomous session | The robot | The robot's key | `Agent-Session` |

A robot commit has the robot's GitHub identity as author and as committer, whatever the harness instance. A person's commit has the person's identity. A mixed commit, with the robot as author and the person as committer, does not exist.

## Terms

A **robot commit** is a commit that an agent makes with `git commit` in a session whose primary actor is a robot. A **manual commit** is a commit that a person makes at the command line with no agent running.

A **supervised session** has a human as a supporting actor. `tsk actor join <uid>` adds the human (ADR 0030). An **autonomous session** has no human supporting actor.

## Identities

The robot's name is the name of its actor. The robot's email is the `email` setting of `[attribution]`: the GitHub noreply address of the robot's account. The robot's signing key is either the secret `TSK_ROBOT_SIGNING_KEY` or a public key that an SSH agent holds.

The person's name, email and signing key are the `user.*` settings of git. A public repository keeps them in an untracked machine file, not in a tracked file.

The `Co-authored-by` value is the `co_author` setting of `[attribution]`: `Name <email>`, with the GitHub noreply address of the person's account.

## Trailers

`Agent-Session` contains the session URN (ADR 0029). The `commit-msg` hook of a robot session writes it, and removes the `Claude-Session` and `Co-Authored-By: Claude` trailers of the harness.

`Co-authored-by` appears once, and only on a robot commit in a supervised session. A person's commit and an autonomous robot commit have none.

## Public repositories

A commit in a public repository stores its author and committer emails in plain text. Use the GitHub noreply address of each account, so that no mailbox appears in the history.
