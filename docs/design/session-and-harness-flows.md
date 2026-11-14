# How sessions, robots and threads start and move

Status: design, 2026-11-14. The diagrams of session start, of moving a thread and of
thread states show built behaviour (tsk 0.57.0 to 0.63.0, ADR 0030 and ADR 0032). The
diagram of `tsk harness new` is a design by Jim. The `tsk harness new` command
does not exist yet, and nothing here says when it will.

This page draws the flows in one place. The steps in words are in
[session-start.md](../user-guide/session-start.md). The rules are in
[ADR 0032](../adr/0032-a-thread-has-one-open-session-and-a-robot-works-in-many.md) and
[ADR 0030](../adr/0030-an-actor-belongs-to-a-session-and-an-instance-is-the-continuity-key.md).

## Contents

- [The terms](#the-terms)
- [A cloud session starts](#a-cloud-session-starts)
- [A terminal session starts](#a-terminal-session-starts)
- [A thread moves between harness instances](#a-thread-moves-between-harness-instances)
- [The proposed tsk harness new command](#the-proposed-tsk-harness-new-command)
- [The states of a thread](#the-states-of-a-thread)
- [Open questions for tsk harness new](#open-questions-for-tsk-harness-new)

## The terms

- A harness instance is a place where a harness runs, such as a cloud session container or a
  code worktree with its terminal.
- A session is one conversation in an instance.
- A robot is an identity. It works in many sessions at once.
- A thread has at most one open session, its holder.

## A cloud session starts

A new cloud session has one repository attached, so the fetch of the nexus at session start
fails. That is expected. The agent attaches the nexus with `add_repo`, does not clone it, and
reads `tsk ledger status` to learn the next step. One question round then gives the robot
and the work.

```mermaid
flowchart TD
    A[SessionStart hook runs] --> B[Fetch of the nexus fails, as expected]
    B --> C[Agent calls add_repo for the nexus, no clone]
    C --> D[tsk ledger status]
    D --> E{What does Next say?}
    E -->|run tsk ledger fetch| F[tsk ledger fetch]
    E -->|names tsk ledger init| G["/tsk:init-ledger"]
    G --> D
    F --> D2[tsk ledger status shows a ledger]
    D2 --> H[tsk thread next]
    H --> I{Bound?}
    I -->|yes: bound:thread-id| Z[Continue the bound thread]
    I -->|no| J[list_sessions with mine true, limit 30]
    J --> K["tsk thread next --open id,id,..."]
    K --> L["One AskUserQuestion, two questions: which robot, which work"]
    L --> M{Robot answer}
    M -->|registered robot| N["tsk actor register name --kind robot"]
    M -->|construct a new robot| O["/tsk:construct-robot"]
    N --> P{Work answer}
    O --> P
    P -->|resume a paused thread| Q["/tsk:resume-thread id"]
    P -->|start a new thread| R["/tsk:start-thread mission"]
    P -->|no thread| S[tsk thread skip]
    Q --> T[set_session_title]
    R --> T
    S --> T
```

The title is `<mission>: <robot>`, or `NO-MISSION: <robot>` with no mission. The agent takes
the `session_...` ID from the `Session title:` line that the command prints.

## A terminal session starts

A terminal needs no `add_repo` and no `list_sessions`. The nexus is attached to the machine
once, and `git` on the machine fetches it. The robots in use come from this machine's
liveness files and its tmux panes.

```mermaid
flowchart TD
    A[SessionStart hook runs] --> B[Hook writes the liveness file]
    B --> C{Fetch of the ledger works?}
    C -->|no| D[tsk ledger status]
    D --> E{What does Next say?}
    E -->|check git access| F[Person fixes access]
    F --> G[tsk ledger fetch]
    E -->|names tsk ledger init| H["/tsk:init-ledger"]
    H --> G
    C -->|yes| I[tsk thread next]
    G --> I
    I --> J{Bound?}
    J -->|yes| Z[Continue the bound thread]
    J -->|no| K[Robots in use from liveness files and tmux panes]
    K --> L["One AskUserQuestion, two questions: which robot, which work"]
    L --> M{Robot answer}
    M -->|registered robot| N["tsk actor register name --kind robot"]
    M -->|construct a new robot| O["/tsk:construct-robot"]
    N --> P{Work answer}
    O --> P
    P -->|resume a paused thread| Q["/tsk:resume-thread id"]
    P -->|start a new thread| R["/tsk:start-thread mission"]
    P -->|no thread| S[tsk thread skip]
```

A terminal session sets no title.

## A thread moves between harness instances

A thread moves from a cloud session to a terminal, or the other way, through a pause. The
pause makes the thread paused, and any robot can resume a paused thread. If the old session
ended with no pause, the thread is still held by a session that no longer runs. The resume
refuses, and the person confirms a take-over.

```mermaid
flowchart TD
    A[Old session holds the thread] --> B{Does the old session pause?}
    B -->|yes: /tsk:pause-thread| C[Thread is paused]
    B -->|no: ended, crashed or archived| D[Thread is still held by a stale session]
    C --> E[Fresh session in the new harness instance]
    D --> E
    E --> F[Session start, as in the diagrams above]
    F --> G[Person chooses the same robot]
    G --> H[Person chooses to resume the thread]
    H --> I{Is the thread paused?}
    I -->|yes| J["/tsk:resume-thread id"]
    J --> K[New session is the holder, no take-over]
    I -->|no: held| L[tsk thread resume refuses]
    L --> M{Does the person confirm the take-over?}
    M -->|no| N[Choose another thread or no thread]
    M -->|yes| O["tsk thread resume id --take-over"]
    O --> P[New session is the holder, the resume warns]
```

The liveness checks say whether a holder is stale: the `claude` process and the tmux pane in
a terminal, `list_sessions` in the cloud. tsk never archives a session for the person.

## The proposed tsk harness new command

This is a design by Jim, 2026-11-14. The command does not exist. It runs in a terminal and
asks questions. Every branch ends in the same three steps: a code worktree, a tmux session,
and `claude` with a first prompt.

```mermaid
flowchart TD
    A[tsk harness new] --> B{"Pick up an existing paused thread? (y/n)"}
    B -->|yes| C["List paused threads: mission ID, robot name, thread ID"]
    C --> D[Person picks one]
    D --> E[Result: mission, robot and thread]
    B -->|no| F{"Attach an existing robot instance? (y/n)"}
    F -->|yes| G["List robots, as tsk actor list --robots"]
    G --> H[Person picks a robot]
    H --> I[Person picks a mission or a new mission]
    I --> J[Result: mission and robot]
    F -->|no| K[Construct a new robot]
    K --> L[Person picks a mission or a new mission]
    L --> M["Result: mission, and a robot that names itself with /tsk:construct-robot in the first turn"]
    E --> Q["Ask the branch name: suggestions, a typed name, or a detached HEAD"]
    J --> Q
    M --> Q
    Q --> N[Create a code worktree for the new harness instance]
    N --> O[Open a tmux session named after the mission code]
    O --> P["Start claude with a first prompt: register the robot, then resume or start the thread"]
```

The last question is the branch name. It lists suggestions in this order, and only those that
apply:

1. The ticket ID of the current task of the thread, such as `TEAM1-546`, when that task has
   one.
2. The mission code in lower case and the thread ID, such as `m-tsk-perf/25366ae1`, when a
   paused thread was picked.
3. The mission code and the harness name, such as `m-tsk-perf/kudu`, when a mission is known
   and no thread.
4. A detached HEAD. The agent names a branch later.

The person types a number, presses Enter for the first suggestion, or types a branch name.
`tsk` checks a typed name with `git check-ref-format --branch` and refuses a branch that
another worktree has checked out. A branch that exists and is free is checked out. A new name
is made from the default branch of origin. `--branch <name>` and `--detach` answer the
question, and the two flags exclude each other. With no terminal and no flag, the first
suggestion is used. The first prompt tells the agent the branch, and the repo's rule: rebase
on origin's main and push to main, or push the branch and open a pull request.

The tmux session takes the mission code as its name, as tsk names other tmux sessions (see
[tmux.md](../user-guide/tmux.md)). The first prompt differs by branch. The first branch
resumes the chosen thread. The other two start a new thread on the chosen mission. The new
robot of the third branch registers itself when `/tsk:construct-robot` runs.

## The states of a thread

A thread is paused or held. Both states are read from the ledger, so a cloud session and a
terminal session read the same answer.

```mermaid
flowchart LR
    P[Paused] -->|start or resume by a session| H[Held by that session]
    H -->|pause, stop or detach| P
    H -->|holder ends with no pause| S[Held by a stale session]
    S -->|resume refused, person confirms| T["tsk thread resume --take-over"]
    T --> H
```

A thread is held when a start or a resume follows its latest pause. A stale holder keeps the
thread held until a pause, or a take-over, changes it.

## Decisions on tsk harness new

Jim answered the open questions on 2026-11-14. The ledger holds them as decisions of
M-TSK-ENV, tasks T-30 and T-31, and question Q-05.

1. **A robot is busy while it holds a thread**, not while it has an open session. A thread
   is held when a start or a resume came after its latest pause. The ledger gives the same
   answer in every place, so a robot that pauses in the cloud is free to resume elsewhere
   while the cloud session stays open.
2. **The location of code worktrees is open (Q-05).** It is a configurable default for every
   worktree on the machine, under each repo's name.
3. **A branch per worktree applies only in a terminal on a shared machine.** A cloud harness
   keeps its own branch instructions. Whether work merges to main directly or through pull
   requests is a choice of each repo: trunk based, or a protected main.
4. **A harness is a code worktree, a tmux session and a `claude` process together.** One
   worktree can hold several branches and pull requests over time, so the worktree belongs
   to the tmux session and the harness instance, not to one branch. The commands:

| Command | What it does |
|---|---|
| `tsk harness new` | asks the questions above, creates the worktree and the tmux session, starts `claude` |
| `tsk harness list` | lists the harnesses, and which ones sleep |
| `tsk harness sleep` | pauses the thread, stops `claude` and the tmux session, keeps the worktree |
| `tsk harness wake` | starts the tmux session and `claude` again in the worktree, resuming the thread |
| `tsk harness delete` | stops `claude`, removes the worktree and kills the tmux session |

Sleep and wake give durable state: a set of harnesses can sleep, the machine can restart,
and the harnesses wake again. Removal in GitHub Copilot CLI is open: its agent cannot change
its own working directory.
