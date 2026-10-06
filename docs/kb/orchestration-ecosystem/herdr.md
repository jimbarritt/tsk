# Herdr

Status: open source, Apache License 2.0, version 0.9.3, read 2026-10-06. Herdr, Inc.
raised a $6M seed led by Bessemer Venture Partners, with Y Combinator, announced
2026-09-08. The founder is Can Celik. The site counts 42,441 GitHub stars and 1,293,735
installs.

Sources: the [herdr.dev](https://herdr.dev/) home page, the
[compare](https://herdr.dev/compare/) page, the documentation pages
[Agents](https://herdr.dev/docs/agents/),
[Agent automation](https://herdr.dev/docs/agent-automation/),
[Session state and restore](https://herdr.dev/docs/session-state/),
[Connecting machines](https://herdr.dev/docs/connecting-machines/) and
[Integrations](https://herdr.dev/docs/integrations/), the
[plugins](https://herdr.dev/plugins/) page, the
[seed announcement](https://herdr.dev/blog/herdr-raised-a-seed/), and the
[README](https://github.com/herdrdev/herdr/blob/master/README.md) in `herdrdev/herdr`.
Every page was fetched as raw HTML or Markdown and read as text. Quoted phrases were
matched against that text. Claims are Herdr's own and were not run independently. The
source code was not read.

## What it is

Herdr calls itself "the runtime your coding agents live on". It is a terminal
multiplexer with panes, tabs and workspaces, built for coding agents. One Rust binary,
"no electron", for macOS, Linux and Windows. It runs inside the terminal a person
already uses.

Its compare page states the design in one line: "A server holds real terminals open on
the machine, the agents live in those, and every UI, ours included, is just a client
that attaches." It lists itself as a "runtime + clients", against tmux and zellij as
terminal multiplexers, cmux and Warp as terminal apps, Solo as a process dashboard, and
Conductor, Emdash and Superset as manager apps.

## Model

| Concept | Meaning |
|---|---|
| Server | A background process that owns the terminals. Clients attach and detach. Agents keep running with no client attached. |
| Client | The TUI, the CLI, or plain SSH. "The TUI is the first client, not the last." |
| Workspace, tab, pane | The layout. Creating a workspace creates its first tab and root pane. Pane IDs take the form `w1:p2`. |
| Agent | "the recognized process currently running inside a pane". A pane exists with or without one. |
| Agent name | An alias such as `reviewer` for the live agent in a pane. It is cleared when that agent exits. |
| Machine | A saved SSH host. Its workspaces and agents appear beside local ones in one window. |
| Session | A named server. A machine profile targets one remote session. |

## Agent state

Herdr marks each agent idle, working, blocked or done. Done means idle and not yet
viewed. A blocked agent makes its pane, tab and workspace show as blocked in the
sidebar. A working agent makes the workspace show as active.

| Source | Mechanism |
|---|---|
| Screen detection | For 23 agents Herdr supports, including Claude Code, Codex, Copilot CLI and Gemini CLI, Herdr finds the agent's process in a pane and reads the live bottom of the screen. Rules in a detection manifest per agent map that screen to a state. |
| Manifest updates | Herdr checks herdr.dev for updated manifests and applies them without a restart. A local override in `~/.config/herdr/agent-detection/<agent>.toml` always wins. |
| Integration reports | Some integrations report state to Herdr's socket, and those reports replace screen detection. |
| Agent reports | Five agents report their own state to Herdr, with nothing to install. |
| Blocked | Marked only when the screen matches a known approval, question or permission prompt. With no match, a known agent falls back to idle, and Codex to unknown. |
| Explain | `herdr agent explain <target>` prints the matched rule, the manifest source and version, and the evidence. |

The Claude Code integration, `herdr integration install claude`, writes
`hooks/herdr-agent-state.sh` and adds hook entries to `settings.json`. The hook reports
the Claude Code session identity to Herdr's socket on `SessionStart`. Claude Code's
state still comes from screen detection.

## Session state

| Case | Processes keep running | Layout returns | Agent conversation resumes |
|---|---|---|---|
| Detach and reattach | Yes | Yes | Yes, the process never stopped |
| Server restart | No | Yes | Only with native agent session restore |
| Update with `--handoff` | Best effort | Yes | Yes, if the handoff succeeds |

- **Snapshot restore**: after a server restart, Herdr restores workspaces, tabs, panes,
  working directories, layout and focus from `session.json`. Up to 48 layout snapshots
  are kept, at most one every 15 minutes.
- **Native agent session restore**: on by default. An integration reports each agent's
  session reference, and Herdr restarts the agent with its own resume command, for
  example `claude --resume <id>` or `codex resume <id>`.
- **Pane screen history**: off by default, "because pane output can include secrets,
  tokens, prompts, and command output".
- **Live handoff**: experimental. An old server passes its live panes to a new one, so
  processes keep running across an update.

## Agent automation

The CLI and a local socket API are one surface, for scripts and for agents. "One agent
can create work for other agents, inspect their state, and collect their results."

| Primitive | Responsibility |
|---|---|
| Layout | Create and organise workspaces, tabs and panes |
| Pane | A raw terminal: `pane run`, `pane send-text`, `pane send-keys`, `pane read`, `pane wait-output` |
| Agent | A recognised agent by name or pane: `agent start`, `agent prompt`, `agent send-keys`, `agent read`, `agent wait` |

- `agent start reviewer --kind codex --pane <id>` returns once Herdr detects the agent
  and marks it ready for input. 24 kinds are supported.
- `agent prompt --wait` submits a prompt and waits for the turn to settle. It refuses
  an agent that is already blocked, with `agent_blocked`, and sends no input.
- `agent wait --until blocked` waits for a state. Wait commands have no default timeout.
- A timeout "does not prove that no input was sent", so a caller reads the agent before
  it retries.
- Commands print JSON. IDs and agent names are scoped to one server.

## Machines and plugins

- `herdr machine add <host>` adds an SSH host. Herdr installs or starts a server there
  after asking. A lost connection shows the last state dimmed, and reconnects with a
  back-off up to two minutes. Herdr Cloud, to connect machines without SSH, is on a
  waitlist.
- A plugin is a public GitHub repository with the `herdr-plugin` topic and a
  `herdr-plugin.toml` manifest. `herdr plugin install` installs one. The index finds
  plugins by topic, and "Listings aren't reviewed by Herdr". The site counts 1,548
  community plugins.

## Compared with tmux

Herdr's docs address tmux users directly: "The prefix is ctrl+b, panes persist, detach
and reattach work the way you expect." Its one-line comparison: "tmux keeps terminals
alive; so does Herdr. The difference is Herdr knows which terminals are agents, what
state each one is in, and how to wait on them. tmux sees panes."

| Aspect | tmux | Herdr |
|---|---|---|
| Persistence | A server holds sessions across a client detach | The same, plus layout restore and agent resume after a server restart |
| Agent awareness | None. A pane is a pane | Detects agents and their state per pane, and rolls it up to tab and workspace |
| Waiting | A script polls `capture-pane` output | `agent wait --until <state>` and `pane wait-output` |
| Scripting | `tmux` commands with format strings | A CLI and a socket API that print JSON |
| Remote | Run tmux on the remote host over SSH | Saved SSH machines in one window, with a combined agent list |
| Input | Keyboard, with a prefix key | Keyboard with the `ctrl+b` prefix, and mouse first |
| Nesting | Not applicable | Herdr can run inside tmux. Herdr does not detect an agent inside a tmux session started in a Herdr pane |

## Comparison with the Mission Control task

| Mission Control objective | Herdr |
|---|---|
| A list of Claude sessions, the control list | The sidebar: workspaces and agents, across machines |
| The selected session on the right | The selected pane. `herdr agent attach <name>` attaches a terminal to one agent alone |
| A terminal across the bottom | Any pane split |
| One command starts a new session, named after the git repo | `herdr agent start <name> --kind claude --pane <id>` in an existing pane |
| A session per tmux pane | An agent per Herdr pane |
| One status indicator for "needs attention" | Four states: idle, working, blocked, done. Done stays visible until viewed |
| Hooks write status to a state file per session | Screen detection by manifest. The Claude Code hook reports the session ID only |
| A pulsing "doing work" circle is a later layer, because it was tricky | Working is detected and shown |
| Token total per session | Not documented. The home page mock shows Claude Code's own context line |
| nvim per session, zoomed in | Not documented as a feature. Any pane runs nvim |
| Sessions survive a tmux detach. Phase two runs `claude --resume` | Detach persistence, and native agent session restore runs `claude --resume <id>` |
| Local only in phase one | Local and SSH machines |
| Python scripts, tmux and Claude Code hooks | One Rust binary |

## Comparison with tsk

| Aspect | Herdr | tsk |
|---|---|---|
| Unit | A pane with an agent in it | A Mission, delegated with a briefing |
| State recorded | Layout, working directories and agent session references, in `session.json` on one machine | Missions, threads and continuation entries, in a ledger on a git branch |
| Continuity | The same agent process, or the agent's own resume command | Thread continuation: an append-only entry, read by any later actor |
| Actor | An agent name, cleared when the process exits | An actor that takes over a thread, with a binding to a session or code worktree |
| Delegation | One agent starts another, prompts it, and waits for its state | A mission passes to an actor with a briefing. The actor writes a report |
| Completion | A turn ends: the agent is idle or done | An objective is met, or a mission report states done, failed or blocked |
| Session start | The Claude Code hook sends the session ID to Herdr's socket | The `SessionStart` hook fetches the ledger and resolves the thread binding |
| Product, Delta, Scale | Not modelled | Four dimensions. Not built |

## Not documented

- How a herdr agent name or session relates to a task, an issue or a branch. The compare
  page says Herdr "pairs with" a worktree and diff review flow.
- Token or cost figures per agent.
- Whether the licence changed. One secondary source, not used above, describes Herdr as
  AGPL-3.0 with commercial licences. The README and the site footer state Apache 2.0.
