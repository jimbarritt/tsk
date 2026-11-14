# Move a thread from a cloud session to a terminal

Use this page to continue a thread in a terminal on your machine when a cloud session
works it. The thread moves through the ledger. The terminal session resumes the thread and
becomes its holder. The cloud session keeps running until you stop it.

## Prerequisites

- `tsk` 0.65.0 or later on the machine for take-over, and 0.67.0 or later for
  `tsk harness new`. Run `tsk --version`, and `tsk self-update` to update.
- The plugin installed at user scope, for `/tsk:resume-thread`. See
  [installation.md](installation.md).
- The nexus that holds the repo's ledger, attached to the machine, and `git` with read
  and push access to it.
- A clone of the repo on the machine. `tsk harness new` also needs `tmux`.

Attach the nexus once, and check it:

```bash
tsk nexus add <nexus url>
tsk nexus list
tsk ledger status
```

`tsk ledger status` must show `refs branches (nexus)` or the nexus refs. With no nexus
attached, `tsk ledger fetch` creates a new, empty ledger on `origin`. Attach the nexus
before any write.

## 1. Pause the thread in the cloud session

In the cloud session, run `/tsk:pause-thread`. The pause records the task and what is
next, and pushes the ledger. It prints three lines:

```text
code: clean
code pushed: yes
ledger: pushed
```

Check each line. `code: dirty` or `code pushed: no` means the work is not on the remote,
and the terminal clone does not have it. Commit and push the code first. A `ledger:` line
other than `ledger: pushed` means the terminal cannot read the pause. Run
`tsk ledger push` in the cloud session.

A thread with a pause is paused. Any session resumes a paused thread, whichever robot
worked it.

## 2. Make a harness for the thread

Run `tsk harness new` in the clone. It makes a code worktree, a tmux session and a
`claude` process with a first prompt. On a terminal it asks in turn:

1. Pick up a paused thread? On yes, it lists the paused threads and you pick one.
2. If not, attach an existing robot? On yes, it lists the robots, free ones first.
3. Which mission? It lists the missions that are not done, and you pick one or a new
   mission.
4. Which branch? It lists suggestions, then you type a number, press Enter for the first,
   or type a branch name.

To answer from a script, pass the thread:

```bash
tsk harness new --thread <thread-id> --branch <branch>
```

`--thread <ID>` resumes that paused thread and skips the questions. `--robot <NAME>`,
`--new-robot`, `--mission <ID>` and `--new-mission` answer the robot and mission
questions. `--branch <NAME>` works on that branch. A new name is made from the default
branch of origin. `--detach` detaches HEAD at the default branch of origin, and the agent
names a branch later. `--no-claude` and `--no-attach` skip the `claude` start and the
tmux attach. `--name <WORD>` names the harness.

tsk refuses a branch that another worktree has checked out. The worktree is
`<root>/<repo name>/<harness name>`. `tsk harness list` shows each harness of the clone.

The first prompt of `claude` registers the robot, then resumes the thread. A held thread
is refused as in step 6.

To resume in a session you started yourself, continue with steps 3 and 4.

## 3. Find the thread in the terminal

Start Claude Code in the clone. Session start fetches the ledger. Then run:

```bash
tsk thread list
```

Each row is JSON. `state` is `paused` or `held`. `holder` is null for a paused thread.
`tsk thread next` prints the question round: the registered robots and their threads, then
the missions. A held thread is not offered there.

## 4. Resume the thread in the terminal

Run `/tsk:resume-thread <thread-id>`. The skill runs:

```bash
tsk thread resume <thread-id>
```

The command binds the terminal session to the thread and makes it the holder. It prints
the latest continuation state entry as JSON. The binding is a local write, and it goes out
with the next push. The skill then prints the mission, the task and where the thread
stands, and asks whether to continue.

To keep the robot of the cloud session, choose that robot in the question round. Or run:

```bash
tsk actor register "<robot name>" --kind robot
```

A name already registered joins that actor. A name registered with the other kind is
refused with exit 1.

## 5. What happens to the cloud session

The ledger records the terminal session as the holder. Nothing stops the cloud session:
it stays open, and its container keeps running. Two sessions on one thread write to the
same ledger. Stop work in the cloud session before you resume. Archive it in claude.ai/code
when you have no more use for it. tsk never archives a session.

## 6. When the cloud session did not pause

A cloud session that ended, crashed or was archived with no pause still holds the thread.
`tsk thread resume <thread-id>` exits 1 with `error: thread <id> is held by ...` and names
the holder and the time it took the thread. `/tsk:resume-thread` asks you whether to take
the thread over. Answer yes only when that session no longer runs. It then runs:

```bash
tsk thread resume <thread-id> --take-over
```

The terminal session becomes the holder. The warning about other actors stays in the
output. Work that the cloud session had not committed and pushed is lost to the terminal.
If you answer no, the skill offers `tsk thread skip`, `/tsk:start-thread` or a paused
thread.

## Failure cases

| Symptom | Cause | Action |
|---|---|---|
| `tsk thread list` shows no thread you expect | The terminal ledger is behind origin. | `tsk ledger fetch`. |
| `tsk ledger status` says to check that git can fetch the nexus | The machine has no access to the nexus repo. | Fix git access, then `tsk ledger fetch`. |
| `tsk ledger status` does not show the nexus | No nexus is attached. | `tsk nexus add <nexus url>`, then `tsk ledger fetch`. |
| `error: thread <id> is held by ...` | The cloud session did not pause. | Confirm it no longer runs, then `tsk thread resume <thread-id> --take-over`. |
| `tsk harness new` is an unknown command | `tsk` is older than 0.67.0. | `tsk self-update`. |
| `tsk harness new` refuses a branch | Another worktree has the branch checked out. | Pick another branch, or `--detach`. |
| `--take-over` is an unknown option | `tsk` is older than 0.65.0. | `tsk self-update`. |
| `warning` is non-empty in the resume output | Another actor worked the thread. | Read it before you continue. |
| `version_note` is set | An older `tsk` wrote the latest entry. | Treat a version or update step in the entry as possibly out of date. |
| The terminal lacks the cloud session's commits | The cloud code was not pushed. | Push from the cloud session, then `git fetch` in the clone. |

## Related

- [session-start.md](session-start.md): the terminal session start and busy robots.
- [missions-threads-and-continuation.md](missions-threads-and-continuation.md): pause,
  resume and take-over.
- [../design/session-and-harness-flows.md](../design/session-and-harness-flows.md): the
  flows of a thread between harness instances.
