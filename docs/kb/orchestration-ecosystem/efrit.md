# Efrit

Status: open source, Apache License 2.0, version 0.4.1, read 2026-10-10. Repository
[steveyegge/efrit](https://github.com/steveyegge/efrit). First commit 2025-07-23, last
commit in the clone 2026-07-04. 632 commits, 620 of them by Steve Yegge, 527 in the 12
months to 2026-10-10. 86 Emacs Lisp files, about 30,000 lines, and a TypeScript MCP server.

Raised from Yegge's post of 2026-10-08 about Rex, which says he runs "Emacs with 20+
agents". Two replies of his in the same thread, read on 2026-10-10, place Efrit in that
setup: "eat buffers work well for me so far", and "Rex will also replace my tmux layer
when it gets more stable. It's definitely buggy right now. Then all that's left are Emacs
and agents and Rex." So the agents run in eat terminal buffers inside Emacs, which is not
Efrit's own buffer, and his stack today is Rex, then tmux, then Emacs, then agents. Efrit
may be one of the agents. See [Yegge's Emacs stack](#yegges-emacs-stack).

Sources: a full clone of the repository. Files read: `README.md`, `ARCHITECTURE.md`,
`SECURITY.md`, `CHANGELOG.md`, `docs/CHANNEL.md`, `docs/CLAUDE_CODE_MIGRATION.md`,
`plans/ROADMAP.md`, `mcp/README.md`, `bin/efrit-tui-init.el`, the headers of the session
and budget modules, the shell allowlist in `lisp/interfaces/efrit-do-handlers.el`, and
`.beads/issues.jsonl`. Efrit was not installed or run. Claims are the repository's own.

## What it is

"An AI coding agent that brings Claude's intelligence directly into Emacs." It is written
in Emacs Lisp, calls the Anthropic API directly, and needs Emacs 28.1 or later and an API
key. It has two roles:

1. **An agent inside Emacs.** Three interfaces: `efrit-chat` (multi-turn, read-only
   context), `efrit-do` (a command run in the background, with a progress buffer) and
   `efrit-agent` (a structured session buffer with status, a TODO list and expandable
   tool calls).
2. **A channel for other agents to drive a live Emacs.** `bin/efrit`, described below.

## The Pure Executor principle

`ARCHITECTURE.md` states one rule, and `plans/ROADMAP.md` repeats it as the only
contribution rule: "ZERO CLIENT-SIDE INTELLIGENCE: Efrit is a pure executor that
delegates ALL cognitive computation to Claude."

| Efrit does | Efrit never does |
|---|---|
| Gathers context: buffer text, file lists, warnings | Pattern recognition on errors or file formats |
| Runs the tools the model selects | Task-specific logic or pre-written fixes |
| Relays results and errors unchanged | Choosing the next tool or the workflow |
| Validates syntax and filters for security | Deciding when to continue or stop |
| Persists session state and logs | Classifying the user's intent |

The cycle: the user's query and the tool schema go to the API, the model returns tool
calls, Efrit runs them and returns the raw results, and this repeats until the model
calls `session_complete`.

## Tools

35 or more, in categories: code execution (`eval_sexp`, `shell_exec`), file editing,
codebase exploration, version control, task management (`todo_write`, `session_complete`,
`request_user_input`), safety (`confirm_action`, `checkpoint`, `restore_checkpoint`,
`show_diff_preview`), diagnostics, external (`web_search`, `fetch_url`), and issue
tracking: `beads_ready`, `beads_create`, `beads_update`, `beads_close`, `beads_list`.

## Sessions

- **Storage.** A session is one JSON file, `~/.emacs.d/efrit/sessions/{id}.json`, holding
  `version`, `id`, `created`, `last_activity`, `project`, `title`, the whole
  `conversation`, the `api_messages`, and `metadata` with turn count, total tokens used
  and status.
- **Controls.** In the agent buffer: `C-c C-p` pauses a session, `C-c C-r` resumes a
  paused one, `C-c C-h` browses history, and `C-c C-q` quits the buffer while the session
  continues.
- **Context management.** A work log with eviction and compression, a token budget with
  least-recently-used eviction, and a circuit breaker "prevents infinite loops".
- **Interruption.** `C-g` stops a running command. Commands queue while one runs.

## The channel: how an agent drives Emacs

`docs/CHANNEL.md`: "A playwright-style channel into a live Emacs for external agents. One
bash command per interaction; every action returns a perception."

```
agent → bin/efrit → emacsclient --eval → efrit-channel.el → JSON back
```

| Aspect | Behaviour |
|---|---|
| Commands | `bin/efrit ping`, `eval '<elisp>'`, `snapshot`, `reload`, `stop` |
| Every eval returns | A JSON envelope: `ok`, `value`, `error`, `messages`, and a `snapshot` of the selected buffer, the text around point with `<\|point\|>` marking the cursor, window layout, buffer list and echo area |
| Errors | Data: `{"ok":false,"error":{"type":"wrong-type-argument",...}}`, including reader errors on unbalanced input |
| Escaping | Payloads cross the `emacsclient` boundary base64-encoded in both directions |
| Timeouts | "Fail loudly": a busy Emacs past `-t SECONDS` returns `{"error":{"type":"timeout"}}` with exit 1, "never a hang" |
| Sockets | The default socket (a running Emacs), then an `efrit` daemon, auto-started if nothing is reachable. `-s NAME` pins one |
| Bounds | Printed values 20k characters, snapshot content 8k, messages 4k, 15 buffers, 60 lines around point |
| Standalone | `efrit-channel.el` "requires nothing from the rest of efrit" and loads into any Emacs 28 or later |

**A design lesson the document records.** "The 2024-era efrit stack failed because
external agents drove Emacs blind: requests crossed multiple async hops (MCP server → file
queue → file watcher → inner LLM loop) and nothing returned the resulting editor state.
Models rationally fell back to `emacs --batch`." The channel "collapses this to a single
synchronous hop". The README still describes the file-based JSON queue for AI-to-AI use,
and the `mcp/` server is built on it.

**Interactive testing.** `bin/efrit-tui` runs `emacs -nw` in a detached, fixed-size tmux
session with a named server socket, so one Emacs is reachable by keystrokes through tmux
and by `bin/efrit` for ground truth: `keys`, `type`, `wait-for <regex> <seconds>`,
`screen`, `eval`. Efrit's own test rig is a tmux session.

## Efrit and beads

Efrit's own backlog is in `.beads/`: 170 issues, 167 closed and 3 open (52 bugs, 86 tasks,
18 features, 8 epics, 6 chores). The roadmap's method is dogfooding: "Use Efrit to build
Efrit", file an issue for every limitation, and work `bd ready --json`. The agent has
beads tools of its own. Yegge also built beads (see `beads-as-backing-store-analysis.md`).

## Security model

`SECURITY.md`: "Efrit trusts Claude (via Anthropic's API) to execute arbitrary Elisp code
in your Emacs." It calls this "fundamentally 'remote code execution'". Each session sends
the username, home directory, current buffer details, git-tracked file names, recent files
and shell output to the API. API keys are not sent. Shell commands pass an allowlist,
`efrit-do-allowed-shell-commands`, of about 90 names including `git`, `ssh`, `scp`,
`rsync` and `docker`. `"*"` as the sole element switches the allowlist off. `tsk` and `bd`
are not on it.

## Compared with tsk

| tsk term | Efrit |
|---|---|
| Surface | Emacs: buffers, key chords and a header line. A person watches and steers an agent buffer |
| Harness | Efrit is its own: it runs the agent loop and calls the API. It is not a harness a Claude Code plugin attaches to |
| Thread continuation | A session file holds the whole conversation and every API message. It resumes in the same Emacs. A tsk continuation entry holds three fields: the mission, the task and what is next. Another machine and another actor read it |
| Mission, objective | None. A session ends when the model calls `session_complete`. No end state is checked |
| Actor | None. A session has an id and a project path |
| Plan | A TODO list the model writes into the buffer |
| Issue tracking | Beads tools in the agent, and a beads backlog for the project |
| Agent-facing interface | One command, a JSON envelope with state returned. The `tsk` binary follows the same pattern |
| Delegation | Another agent drives the Emacs through the channel. No briefing or report |

Three items intersect tsk:

- **The channel's contract is a model for a `tsk` command**: one command per
  interaction, errors as data, a bounded result, a timeout that fails loudly, and the
  current state returned with every answer.
- **The failure of the multi-hop design.** tsk has an external event queue in the ledger.
  A request that crosses several asynchronous hops and returns no state is the failure the
  channel document names.
- **A tmux-driven test rig for an agent surface.** `efrit-tui` is the pattern that
  `tmux send-keys` and `capture-pane` give Mission Control: keystrokes in, rendered screen
  out.

## Yegge's Emacs stack

From two replies in the thread of his 2026-10-08 post about Rex, read on 2026-10-10 as a
screenshot:

- **Terminals inside Emacs are eat buffers.** "eat buffers work well for me so far." eat
  (Emulate A Terminal) is an Emacs package that runs a terminal emulator in a buffer, so
  a terminal program such as Claude Code runs inside Emacs. That description of eat is
  from general knowledge, not from a page read.
- **tmux is still in the stack, and Rex is to replace it.** "Rex will also replace my
  tmux layer when it gets more stable. It's definitely buggy right now. Then all that's
  left are Emacs and agents and Rex."

So the nesting today is Rex, then tmux, then an Emacs daemon reached through
`emacsclient` (the tab title in his screenshot), then agents in eat buffers. The target
is Rex, then Emacs, then agents. Emacs is the top-level host of the agents, with
terminals inside it. Efrit, whose agent surface is a buffer with no terminal, is at most
one of those agents.

## Not established

- Which agents run in his eat buffers, and whether Efrit is among them. The replies
  name eat and Rex, not the agents.
- Any dotfiles. No public repository of his Emacs configuration was found. His profile
  page could not be read from the sandbox, so the list of his repositories is not seen.
- How the project is used by anyone but its author: 620 of 632 commits are his.
- What changed after 2026-07-04, the date of the last commit in the clone.
