# How a new Claude Code session starts with tsk

This page describes the first turn of a new Claude Code session in a repo that uses tsk:
in a cloud session on claude.ai/code, and in a terminal. By the end of the first turn the
session has a ledger, a robot and, when you choose one, a thread. The behaviour is from
tsk 0.57.0 to 0.63.0.

The steps below are the steps the agent runs. Your part is to answer one question round:
which robot this session is, and what to work on.

## Contents

- [The parts involved](#the-parts-involved)
- [A cloud session](#a-cloud-session)
- [A terminal session](#a-terminal-session)
- [A repo with no ledger](#a-repo-with-no-ledger)
- [The question round](#the-question-round)
- [Which robots are busy](#which-robots-are-busy)
- [The tmux chooser](#the-tmux-chooser)
- [Quick reference](#quick-reference)

## The parts involved

| Part | What it does at the start of a session |
|---|---|
| The plugin's `SessionStart` hook | Runs `tsk thread session-start`: fetches the ledger, exports `$TSK_LEDGER_WT`, and prints the context. With no thread binding, the context holds the robot question. |
| `tsk ledger status` | With no ledger store in the clone, prints the attached nexus, what it says about this repo, whether origin holds a ledger, and a `Next` line with the step to run. |
| `tsk thread next` | Prints `bound:<thread-id>` for a bound session. Otherwise prints the robot question. It reads the local ledger store and never fetches. |
| The plugin's `Stop` hook | After each turn of a session with no binding, prints the same robot question. |
| The plugin's `SessionEnd` hook | Outside a cloud session, records the end of the session on this machine. Then starts `tsk ledger push --background` and exits. |

Install the `tsk` and `swe` plugins once, at user scope:

```bash
claude plugin install tsk@jimbarritt-claude-plugins --scope user
claude plugin install swe@jimbarritt-claude-plugins --scope user
```

A cloud environment's setup script installs them for you. See
[new-cloud-environment.md](new-cloud-environment.md).

## A cloud session

This section is for a cloud environment whose setup script runs with
`--nexus <nexus url>`, and a repo whose ledger is in that nexus. For a repo whose ledger
is on its own origin, steps 1 to 4 do not apply: session start fetches the ledger, and
the session goes on at step 5.

### What `--nexus` sets up

The setup script `ops/cloud/setup-environment.sh` with `--nexus <nexus url>`:

1. Runs `tsk nexus add <nexus url>`, which writes the URL to the user tsk config.
2. Adds a section to `~/.claude/AGENTS.md`. Claude Code reads it in every session of the
   environment, through `~/.claude/CLAUDE.md`.
3. Adds three entries to `~/.claude/settings.json`, so that auto mode does not refuse the
   `add_repo` call:
   - `mcp__claude-code-remote__add_repo` in `permissions.allow`.
   - An `autoMode.environment` entry that names the nexus as a trusted repository.
   - An `autoMode.allow` entry that allows the agent to attach the nexus with `add_repo`
     and to fetch from it and push to it with tsk commands.

The section in `~/.claude/AGENTS.md` reads as follows, with your nexus URL in place of
`<nexus url>`:

```markdown
# The tsk nexus

This environment keeps tsk ledgers in the nexus repository <nexus url>. The owner of this
environment chose it with the `--nexus` option of the setup script, and authorises every
session in this environment to attach it with the `add_repo` tool.

On the first turn of a session, before any other work and whatever the first message
asks, run `tsk ledger status` and do what its `Next` line says, with no confirmation
first. Run it again after each action, until `Local` shows a ledger. If `add_repo` is
refused, report the refusal and stop. Do not clone the nexus after `add_repo`, and do not
run `register_repo_root` for it: tsk fetches the nexus itself, and the grant is all it
needs. The result of `add_repo` always says to clone the repository. That is expected
here: skip the clone and do not mention it. If `Next` names `tsk ledger init`, run the `/tsk:init-ledger` skill.

Once `Local` shows a ledger, run `tsk thread next` in the same turn and do what it says:
it asks which robot this session is, then which thread to resume. Do this before you
answer the first message.
```

The account's GitHub connection needs read and push access to the nexus repo.

### The first turn, step by step

1. **Session start fails to fetch the nexus.** This is expected. A new cloud session has
   one repository attached, the repo you opened, and the session's git proxy refuses any
   other repository. The context says so and gives the next step, with no `Warning:`
   line:

   ```
   This session cannot read the nexus <nexus url> yet: a cloud session reads only the
   repositories attached to it, so the fetch at session start failed, as it does in every
   new session. Attach it with the add_repo tool (owner <owner>, repo <repo>, access
   push). Do not clone it: tsk fetches the nexus itself. Then run `tsk ledger status` and
   do what its `Next` line says, before any other ledger command.
   ```

2. **The agent runs `tsk ledger status`.** With no ledger store in the clone, it prints
   four lines, for example:

   ```
   Local   no ledger in this clone
   Nexus   https://github.com/<you>/<nexus>: cannot be fetched: <reason>
   Origin  holds no ledger
   Next    attach the nexus to this cloud session with the add_repo tool (owner <you>, repo <nexus>, access push), do not clone it (tsk fetches it itself), then run tsk ledger fetch
   ```

3. **The agent attaches the nexus with the `add_repo` tool.** The `--nexus` settings
   pre-approve the call, so no permission prompt appears. The result of `add_repo` says
   to clone the repository. The agent does not clone it and does not run
   `register_repo_root` for it: `tsk` fetches the nexus itself, into its own copies under
   the state directory. The grant is all that `tsk` needs.

4. **The agent runs `tsk ledger status` again, then `tsk ledger fetch`.** Now the `Nexus`
   line names the repo's entry, `ledger in the nexus`, and `Next` says
   `run tsk ledger fetch`. The fetch makes the ledger store. A third `tsk ledger status`
   shows the ledger, with `refs branches (nexus)`.

5. **The agent runs `tsk thread next`.** With no binding and no robot, it prints every
   robot, the free robots first, then the busy robots. Each line gives the robot, `free`
   or `busy (holds <thread id> <mission>)`, and its threads, with `held by <robot>` on a
   thread that a session holds. Then it prints the missions that are not done. A robot is
   busy while it holds a thread: it started or resumed the thread after the thread's
   latest pause. The ledger alone says so, so a cloud session and a terminal give the
   same answer. A busy robot stays in the list, marked, and is not hidden (ADR 0032).

7. **The agent asks you one question round.** See [The question round](#the-question-round).

8. **The agent runs the commands for your answers, then sets the session title.** In a
   cloud session, `tsk actor register`, `/tsk:start-thread`, `/tsk:resume-thread` and
   `tsk thread skip` each print a line such as:

   ```
   Session title: call set_session_title once, with session_id session_<id> and title "M-ABC-NAME: Some Robot Name".
   ```

   The agent calls `set_session_title` with it. The line holds the `session_...` ID that
   `set_session_title` takes, so the agent types no ID.

Then the agent answers your first message.

### A session that is already bound

When the session already has a binding, `tsk thread next` prints `bound:<thread-id>` and
a line that names the thread and its mission. The agent goes on with that thread.

A second session start of the same session, such as the context reload after
`register_repo_root` for another repository, prints one line: the session is bound to the
thread, and not to run `/tsk:resume-thread`.

## A terminal session

In a terminal, the nexus is attached to the machine once with `tsk nexus add <nexus url>`,
and `git` on the machine fetches it. There is no `add_repo` step and no
`~/.claude/AGENTS.md` section.

1. **Session start fetches the ledger.** With no binding, its context holds the robot
   question, the same text that `tsk thread next` prints.
2. **If the fetch fails,** run `tsk ledger status`. With a nexus that cannot be fetched,
   `Next` says `check that git on this machine can fetch <nexus url>, then run tsk ledger
   fetch`. Fix the access, then run `tsk ledger fetch`.
3. **The agent asks you the question round** and runs the commands for your answers, as in
   a cloud session. There is no `Session title:` line and no `set_session_title` call in
   a terminal.

`tsk thread next` and `tsk actor list` accept `--open` and ignore it (since 0.65.0), so
an older skill still runs.

### The liveness files

Session start outside a cloud session writes a liveness file:

```
${XDG_STATE_HOME:-~/.local/state}/tsk/live/<session id>.json
```

The file holds the process ID and start time of the `claude` process above the hook, and
the time of the last turn. The `Stop` hook updates the time after each turn. The
`SessionEnd` hook runs `tsk session end`, which records the end. Nothing in the file goes
into the ledger. The robot choice does not read the file: a robot is busy while it holds
a thread, not while a session of it is open.

A held thread is not offered at session start. `tsk thread resume <thread-id>` of a held
thread exits 1 and names the holder. A holder that no longer runs (a session that ended,
crashed or was archived with no pause) still shows as the holder. The agent then asks you
whether to take the thread over, and on yes runs
`tsk thread resume <thread-id> --take-over`. See
[missions-threads-and-continuation.md](missions-threads-and-continuation.md).

## Which robots are busy

`tsk actor list` prints the columns name, kind, state, missions, uid and instances. The
state of a robot is one of:

| State | Meaning |
|---|---|
| `free` | The robot holds no thread. |
| `busy (holds <thread id> <mission>)` | The robot started or resumed the thread after the thread's latest pause. The mission is `no-mission` for a thread with none. |

A busy robot is still usable: a robot works in many sessions at once, and it takes a new
thread or no thread. A human has `-`. The missions column lists the missions of the
robot's threads: the held thread's mission first, then the others, latest first, at most
three and then `+N`. `-` means none. `--json` adds a `state` field and a `missions` array.
`--porcelain` prints name, kind, state, uid and instances, tab-separated.

A robot becomes free when it pauses the thread it holds: run `/tsk:pause-thread` or
`tsk thread pause`. A session that ended with no pause still holds its thread. Take the
thread over with `tsk thread resume <thread-id> --take-over`.

## The tmux chooser

The tmux session chooser (`prefix s`) prints its fields in columns (0.63.0): the tmux
session name in 18 characters, the state, the mission name in 24 characters, the robot in
32 characters, and the directory. The robot and the directory line up on every line. The
columns need tmux 3.2 or later.

A tmux config written by an earlier `tsk tmux install` has the old format. Run
`tsk tmux install` again to write the new one. See [tmux.md](tmux.md).

## Quick reference

| Situation | What to run |
|---|---|
| `tsk ledger status` shows `Next ... add_repo` | Attach the nexus with `add_repo`. Do not clone it. Then `tsk ledger fetch`. |
| `tsk ledger status` shows `Next ... check that git on this machine can fetch` | Fix git access to the named repository, then `tsk ledger fetch`. |
| `tsk ledger status` shows `Next ... tsk ledger init` | `/tsk:init-ledger`. |
| `tsk ledger status` shows `Next    run tsk ledger fetch` | `tsk ledger fetch`. |
| The ledger is fetched and the session has no binding | `tsk thread next`. |
| Which robots are free or busy | `tsk actor list`. |
| Free a busy robot | Pause the thread it holds: `/tsk:pause-thread` or `tsk thread pause`. |

Related guides:

- [new-cloud-environment.md](new-cloud-environment.md): the cloud environment and its setup
  script.
- [nexus-quickstart.md](nexus-quickstart.md): a nexus and a repo's ledger in it.
- [missions-threads-and-continuation.md](missions-threads-and-continuation.md): threads,
  pause and resume.
- [tmux.md](tmux.md): the tmux chooser and sidebar.
