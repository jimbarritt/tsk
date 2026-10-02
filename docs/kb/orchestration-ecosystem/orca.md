# Orca

Status: open source (MIT), version 1.4.214, latest commit 2026-10-02. A desktop app for
macOS, Windows and Linux, with iOS and Android companion apps. The site says it is "Backed
by Y Combinator" and © Stably AI. The licence file names Lovecast Inc. as the copyright
holder.

Sources: a shallow clone of [stablyai/orca](https://github.com/stablyai/orca) at commit
`ddcc279` (2026-10-02), and the product site [onorca.dev](https://www.onorca.dev/), fetched
as raw HTML. From the clone: `README.md`, `AGENTS.md`, `package.json`, `orca.yaml`, the
documentation in `docs/site/content/docs/` (worktrees, agents and sessions, hooks,
orchestration, checkpoints, hibernation, session history, usage tracking, notifications,
ways to run, automations, settings) and `skill-guides/orchestration.md`. Quoted phrases
were matched against those files. The repository holds 31,706 files, and most of the
source under `src/` was not read. Orca was not installed or run. Claims are Orca's own.

## What it is

Orca calls itself an "ADE", an agent development environment, and "The AI Orchestrator for
100x builders". It runs coding agents side by side, each in its own git worktree, in one
desktop app. The README says: "Run Codex, ClaudeCode, OpenCode or Pi side-by-side — each in
its own worktree, tracked in one place."

It is an Electron application (React, TypeScript, `node-pty` terminals, xterm). It is not a
terminal UI. It embeds terminals, a code editor, a Chromium browser and a diff viewer in
one window. It does not depend on tmux. The documentation mentions tmux only in a clipboard setting for
programs that run inside an Orca terminal.

## Model

| Concept | Meaning |
|---|---|
| Project | One git repository or a related cluster. The sidebar groups worktrees by project. |
| Worktree | A real `git worktree` with its own branch, files, agent terminals, editor tabs and browser tabs. Every task gets one. Orca "is worktree-native". |
| Agent session | "One CLI agent running in one terminal in one worktree". Orca tracks its lifecycle. |
| Workspace | A worktree, or a folder workspace that needs no Git. |
| Linked task | One GitHub, Linear, Jira or GitLab item linked to a workspace, shown on its card. |
| Run, Task, Dispatch | The orchestration layer, below. |

A worktree's lifecycle in the documentation: create, work, review, ship (commit, push, open
a pull request and wait on checks), then archive or delete. Creation runs in the background.
A repo's `orca.yaml` can list gitignored directories to share between worktrees, and a
`.worktreeinclude` file lists gitignored files to copy.

## Agent state

Agent tabs and worktree rows show one of five states:

| State | Indicator |
|---|---|
| Working | Spinner |
| Waiting on the person (a permission or a question) | Amber question mark. The sidebar counts these as "Needs You". |
| Done | Emerald check on the dashboard, emerald dot in the sidebar |
| Blocked, interrupted or failed | Red dot |
| Idle | Gray dot |

"State is detected from the terminal's OSC title sequence and agent hooks, which Claude
Code, Codex, and several other agents emit." Settings has "Agent status hooks", Orca-managed
hooks that report working, waiting and done. Orca installs them and removes them when the
setting is off. Hook endpoints are written to disk so a long-lived session keeps reporting
after Orca restarts.

An experimental Agent Dashboard shows a board across worktrees with four columns: Needs
You, Working, Done and Idle. A card shows the agent, the session name, the last message
and the age. Clicking a card focuses that agent's terminal. Nested subagents can appear as
children.

When an agent changes from working to idle, Orca sends a system notification, a sound and a
chip on the worktree. A header bell holds unread notifications. The macOS Dock icon shows
the unread count.

## Orchestration

Orchestration is experimental and is Orca's multi-agent layer. It records "who owns work,
which attempt is authoritative, and when supervised work has settled."

| Concept | Meaning |
|---|---|
| Run | A "durable namespace and home inbox". It never schedules or places workers. |
| Task | A work item with a spec, dependencies and a status: `pending`, `ready`, `dispatched`, `completed`, `failed` or `blocked`. |
| Dispatch | One attempt of a task on a terminal. It holds the lifecycle authority for completion and heartbeat messages. |
| Worker | A supervised agent started for a task with `orca orchestration worker-start`, in the current worktree or a new child worktree, on a chosen agent, model and effort. |
| Message | Inbox mail: `status`, `dispatch`, `worker_done`, `escalation`, `question`, `heartbeat` and others. Delivery is first in, first out, and a message is replayed until acknowledged. |
| Decision gate | A coordinator-owned question that blocks a task until it is resolved. |

A worker reports `worker_done` with an outcome of `succeeded` or `failed`, a subject, a body
and the files it modified. The orchestration skill states a coordinator's outcome as a
result, a next consumer, a definition of done and a safe failure. Its report must name,
"per Task, its outcome, the evidence behind it, and any unresolved blocker". On failure the
rule is to "preserve work and authority and report the state as unknown or `unverifiable`".

A worker with a blocking question uses an `ask` command and waits for the coordinator. A
live terminal "can still hold a dead or stuck agent", so liveness is reported in layers.
A task can run on another host with `--on`, and later commands address the dispatch.

A separate `orca-cli` skill covers a full ownership handoff without supervision: it creates
no Run, Task or Dispatch and does not monitor completion.

## Worktree checkpoints

Every worktree has a free-text comment that agents update with
`orca worktree set --worktree active --comment "..."`, plus an optional card status of
`todo`, `in-progress`, `in-review` or `completed`. The documentation recommends this "for
keeping human collaborators in the loop without forcing chat". The first line states what
just happened, where, and the next step. An agent should read the comment first "so you
don't clobber goals or constraints".

## Sessions, usage and remote work

- **Session history**: Orca scans the on-disk transcripts that supported agent CLIs leave
  behind and lists them. Resume opens a terminal in the same directory and runs the agent's
  resume command, such as `claude --resume <id>`.
- **Hibernation** (experimental, off by default): Orca stops an idle agent terminal after
  30 minutes by default and resumes the same session when the worktree reopens. It does so
  only when the agent is done, has a resumable session, and has no unsettled orchestration
  dispatch or live subagent.
- **Usage tracking**: Orca reads each agent's local usage state on disk for Claude Code,
  Codex, Gemini, OpenCode, Kimi Code and MiniMax. It shows usage against the plan, time to
  reset for 5-hour, daily and weekly windows, and a warning chip above 80%. The numbers
  are "only as fresh as the agent's own bookkeeping". A Stats view can show estimated cost
  from a local price table.
- **Remote work**: SSH hosts, a Remote Orca Server that owns projects, worktrees, terminals
  and agent processes, and a mobile companion that monitors and steers agents. Orca "does
  not sell managed VPS hosting".
- **Automations**: `orca automations create` runs a prompt on a schedule, with presets,
  cron or RRULE strings, a provider, and a repository or workspace target.
- **Orca CLI**: agents can control Orca with commands such as `orca worktree create`,
  `snapshot`, `click` and `fill`.

## Agents and permissions

Orca runs any CLI agent. The site claims 27 supported agents. It "launches every supported
agent with its full-autonomy permission flag pre-applied", for example
`--dangerously-skip-permissions` for Claude Code. The stated reason: "the worktree itself is
the sandbox". A user can edit each agent's launch arguments.

The enterprise page states that "no model in the middle": prompts and code go from the
agent straight to the configured provider, and Orca does not inspect or store them. It also
states that every agent change arrives in a worktree and a pull request. It lists AICPA SOC 2
as "Readiness".

## Not documented

- Any record of a mission, objective, plan or definition of done outside the orchestration
  layer. A Task has a spec and a status.
- How an objective is checked beyond the worker's own `worker_done` outcome.
- Any permission or authority model for what an agent may do, beyond the launch flags.
- Per-session token totals. Usage is shown per account and provider window.
- Any handover record between actors beyond the checkpoint comment and session history.
- Any model of product, deployed change or scale.
- The source under `src/`, beyond the files listed above.

## Comparison with the Mission Control task

Mission Control, M-BOOT-06, is a tmux layout with a control list of Claude sessions, built
standalone from tsk. Its plan marks tasks T-01 to T-08 done and T-09, confirming on macOS,
open. This table sets its objectives against Orca.

| Mission Control objective | Orca |
|---|---|
| A list of Claude sessions, the control list | A sidebar of projects and worktrees, and an experimental board across worktrees. |
| The selected session on the right | The selected worktree's tabs and split panes. |
| A terminal across the bottom | Terminal splits in each worktree. |
| One command starts a new session, named after the git repo | A create dialog and `orca worktree create`. The branch name derives from the typed name or the linked item. |
| A session per tmux pane | A session per terminal per worktree. |
| One status indicator for "needs attention": permission, waiting, finished turn, error | Five states. Waiting on the person and done are separate. Blocked and failed are one red dot. |
| Hooks write status to a state file per session | Orca-managed agent status hooks report to the running app, with the terminal title as a second signal. |
| A pulsing "doing work" circle is a later layer, because it was tricky | A spinner for working, shipped. |
| Token total per session, from transcript usage records | Usage per account and window from each agent's local state. Per-session totals are not documented. |
| nvim per session, in its worktree | A built-in code editor and file explorer in each worktree. |
| Sessions survive a tmux detach. Phase two rebuilds and runs `claude --resume` | The sessions are app terminals. Session history lists transcripts and resumes them. Scrollback survives a restart. |
| Local only in phase one. Cloud sessions later | SSH hosts, a Remote Orca Server and a mobile companion. |
| Worktree creation is manual | Worktree creation is the core operation. |
| Python scripts, tmux and Claude Code hooks | An Electron desktop app. |
| Tsk threads later | Linked GitHub, Linear, Jira or GitLab items. |

## Comparison with tsk

| Aspect | Orca | tsk |
|---|---|---|
| Unit | A worktree with a linked item and agent sessions. | A Mission, delegated with a briefing. |
| Task record | A Task has a spec, dependencies and one of six statuses, in the orchestration layer. | A Task in a mission's plan. Objectives are checkable states. |
| Delegation | A coordinator starts supervised workers. A Dispatch is one attempt. | A mission passes to a different actor with a briefing. The actor writes a report. |
| Completion | A worker sends `worker_done` with `succeeded` or `failed`, evidence and files modified. | A mission report: done, failed or blocked, with an account of what the briefing failed to give. |
| Open questions | A worker can ask the coordinator and wait. A decision gate blocks a task. | A mission report contains no open question. Decision authority is stated in the briefing. |
| Continuity | A free-text checkpoint comment per worktree, and transcript-based resume. | Thread continuation: an append-only entry at each pause. |
| State store | The app's own storage, plus git for the code. | A ledger on a git branch, to move to a nexus repo. |
| Approvals | Full-autonomy flags by default. The worktree is the sandbox. | Post would hold authority. Escalation to a post. Not designed. |
| Triggers | Scheduled automations by cron or RRULE. | Event-triggered operating context is named. Not built. |
| Substrate | Any CLI agent. Orca does not sit between the agent and the model. | The domain model names no substrate. Independence is untested. |
| Product, Delta, Scale | Not modelled. | Four dimensions. Not built. |

## Convergence and difference

Orca and tsk agree on two points:

- A delegated agent reports an outcome and an account of the work. Orca's worker sends
  `succeeded` or `failed` with evidence. A tsk mission report records done, failed or
  blocked.
- A person needs a view of every agent's state, and "needs attention" is the main signal.
  This is an objective of Mission Control.

Three more directions are shared, and tsk's design has none of them: a separate workspace
for each task, a link from a task to an issue tracker, and scheduled triggers. The ledger
holds an idea for a tracker link. Mission Control creates worktrees by hand.

They differ on five:

- Orca is an app that runs agents. tsk is a model of work and a ledger.
- Orca's orchestration is one layer in the app and is experimental. tsk's mission model is
  the core.
- Orca's workers may ask the coordinator a blocking question. A tsk report has none.
- Orca keeps a mutable checkpoint comment. tsk keeps an append-only log.
- Orca grants full autonomy and relies on the worktree. tsk's design would attach authority
  to a post, and OpenAPPA shows an enforcement layer for that.

## Sources

- [stablyai/orca](https://github.com/stablyai/orca), commit `ddcc279`: `README.md`,
  `AGENTS.md`, `package.json`, `orca.yaml`, `LICENSE`, `docs/site/content/docs/`,
  `skill-guides/orchestration.md`
- [Orca product site](https://www.onorca.dev/) and its
  [enterprise page](https://www.onorca.dev/enterprise)
- `mission-control` briefing, M-BOOT-06, in the tsk ledger

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position, and
  the Orca section that links here.
- [openappa.md](openappa.md): an enforcement layer for the permissions Orca leaves to the
  worktree.
- [cursor-projects.md](cursor-projects.md): a coordinator that delegates to subagents.
- [docs/adr/0004-unified-tsk-binary.md](../../adr/0004-unified-tsk-binary.md): the tsk TUI,
  the later home for the control list.
