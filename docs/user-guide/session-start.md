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
- [Which robots are in use](#which-robots-are-in-use)
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

5. **The agent runs `tsk thread next`.** In a cloud session (`CLAUDE_CODE_REMOTE=true`),
   with no binding and no robot, it prints one step: call the `list_sessions` tool of the
   `claude-code-remote` server, with `mine: true` and `limit: 30`, take the ID of every
   session that is not archived, and run `tsk thread next --open <id>,<id>,...`.

6. **The agent runs `tsk thread next --open <ids>`.** It prints the robots that no open
   session uses, each with its threads, then the missions that are not done. A robot
   whose registrations or sessions name one of the open sessions is not offered, and the
   text names it as taken. A session ID matches its harness instance: `session_X`,
   `cse_X` and `cloud:cse_X` are one session.

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

### The open-session limit

`list_sessions` returns the 30 newest sessions. A session that is open but older than
those does not mark its robot as taken, so that robot can be offered.

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

`tsk thread next` in a terminal takes no `--open`. It reads the facts of this machine
instead.

### How a terminal session counts as open

Session start outside a cloud session writes a liveness file:

```
${XDG_STATE_HOME:-~/.local/state}/tsk/live/<session id>.json
```

The file holds the process ID and start time of the `claude` process above the hook, found
through the parent processes with `ps`, and the time of the last turn. The `Stop` hook
updates the time after each turn. The `SessionEnd` hook runs `tsk session end`, which
records the end. Nothing in the file goes into the ledger.

A terminal session is open when it has no recorded end and one of these is true:

- Its `claude` process still runs, with the same start time as the file records.
- No process is recorded, and its last turn is within 7 days. Set `TSK_ROBOT_IDLE_DAYS` to
  change the limit.

A session whose recorded process no longer runs is not open. So a crash frees its robot.

### When a robot counts as taken in a terminal

A robot is taken, and not offered, when one of these is true:

- A session open on this machine is registered as the robot.
- A tmux pane shows the robot in `@tsk_actor` and runs `claude`.

Sessions on other machines are not checked.

### Free a robot

- Quit the Claude Code session. The `SessionEnd` hook records the end, and the robot is
  free at once.
- Before you quit, run `/tsk:pause-thread` when the next session needs a hand-off note.
  Quitting records no continuation state.

## A repo with no ledger

When no ledger exists for the repo, in the nexus or on origin, `tsk ledger status` prints
a `Next` line that names `tsk ledger init`. With a nexus attached:

```
Next    ask the person where the new ledger goes, then run tsk ledger init --nexus (in the nexus) or tsk ledger init --repo (in this repo)
```

The agent then runs the `/tsk:init-ledger` skill:

1. It asks where the ledger goes: in the nexus (recommended), or in this repo. A ledger in
   this repo is on its `origin`, so it is public when the repo is public.
2. It proposes a mission prefix of 3 or 4 capital letters, for you to change.
3. It runs one of:

   ```bash
   tsk ledger init --nexus --prefix <PREFIX>
   tsk ledger init --repo --prefix <PREFIX>
   ```

   With `--nexus`, the command first adds the repo to `nexus.json` and checks the prefix
   against the other repos. Add `--territory <id>` for the first repo of a new nexus,
   which creates `nexus.json` and the territory, and when the nexus has more than one
   territory. The command creates the ledger and pushes it.

With a nexus attached, `tsk` never creates a ledger on origin by itself. Every ledger
command except `tsk ledger status` and `tsk ledger init` stops and names
`tsk ledger init`. Before 0.57.0, `tsk ledger fetch` created an empty ledger on origin in
this case, and a push made it public.

## The question round

With no binding and no robot for the session, the agent asks two questions in one
`AskUserQuestion` call. Each question also takes free text.

1. **Which robot this session is.** Up to three free robots, the one with the latest
   thread first, and "Construct a new robot".
2. **What to work on.** The latest thread of each robot offered, labelled
   `<robot>: resume <thread id> <mission>`, then missions to start a new thread on, and
   "No thread". Four options at most.

The agent runs one command for each answer:

| Answer | Command |
|---|---|
| A robot from the list | `tsk actor register "<name>" --kind robot` |
| Construct a new robot | `/tsk:construct-robot` |
| A thread | `/tsk:resume-thread <thread-id>` |
| A mission | `/tsk:start-thread <mission-id>` |
| No thread | `tsk thread skip` |

If the thread you choose belongs to a robot other than the one you choose, the agent asks
again which thread.

With every registered robot taken, there is one choice for the robot: the agent runs
`/tsk:construct-robot`, then `/tsk:start-thread <mission-id>`.

A session that already has a robot but no binding gets one question: which of that
robot's threads to resume, or a mission for a new thread.

When the agent answers your first message without this round, the `Stop` hook prints the
same question after the turn.

### The session title

The title is `<mission>: <robot>`, for example `M-ABC-NAME: Some Robot Name`. A session
whose thread has no mission, or that has no thread, is titled `NO-MISSION: <robot>`.
`tsk thread title` prints the title of the current session.

## Which robots are in use

`tsk actor list` prints the columns name, kind, state, uid and instances (0.63.0). The
state of a robot is one of:

| State | Meaning |
|---|---|
| `free` | No open session uses the robot. |
| `taken (tmux pane)` | A tmux pane on this machine shows the robot and runs `claude`. |
| `taken (session <id>)` | The open session `<id>` is registered as the robot. |
| `unknown` | The state was not checked. |

A human has `-`. `--json` adds a `state` field.

In a terminal, the state comes from this machine's liveness files and tmux panes. In a
cloud session, pass the open sessions as `tsk thread next` takes them:

```bash
tsk actor list --open <id>,<id>,...
```

Without `--open`, a cloud session shows every robot as `unknown`.

A robot stays taken while any cloud session it ran in is not archived, idle sessions
included. To free a robot of a cloud session, archive that session. Run
`/tsk:pause-thread` first when the next session needs a hand-off note.

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
| The ledger is fetched and the session has no binding | `tsk thread next`. In a cloud session, `list_sessions`, then `tsk thread next --open <ids>`. |
| Which robots are free | `tsk actor list`, or `tsk actor list --open <ids>` in a cloud session. |
| Free a robot | Quit the terminal session, or archive the cloud session. Pause first for a hand-off note. |

Related guides:

- [new-cloud-environment.md](new-cloud-environment.md): the cloud environment and its setup
  script.
- [nexus-quickstart.md](nexus-quickstart.md): a nexus and a repo's ledger in it.
- [missions-threads-and-continuation.md](missions-threads-and-continuation.md): threads,
  pause and resume.
- [tmux.md](tmux.md): the tmux chooser and sidebar.
