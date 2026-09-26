# Mission: Mission Control

| Field | Value |
|---|---|
| ID | M-BOOT-06 |
| Territory | agentic research |
| Assignee | the user |
| Blocked by | none |

## Objective

- One command, typed in a new tmux session, sets up this layout in that session:
  - Left: a list of Claude sessions. This list is the control list.
  - Right: the currently selected Claude session.
  - Bottom, across the full width: a terminal for anything else.
- The command starts a new Claude session. Each Claude session runs in its own tmux
  pane.
- Each Claude session takes a name. The default name is a good one, for example the
  name of the current git repo.
- The control list shows a status indicator per session, so the user can see when a
  session needs attention.
- The control list shows token consumption per session.
- A shortcut key on the control list launches nvim in the root directory, zoomed in. A
  zoom out, or a custom shortcut, hides nvim, keeps it running, and returns to the
  three panel layout.

The control list is called Mission Control. Settled by the user 2026-09-25. "Mission
command" was the alternative, rejected because `docs/domain/mission-model.md` in the
tsk repo uses it for the doctrine from *Auftragstaktik*. "Mission control" was rejected
for Nexus in `docs/domain/ubiquitous-language.md` for its operational-control sense,
which fits this list. In phase one a session is not tied to a mission. The user
accepts that.

## Purpose

Parent: M-BOOT. The aim: replicate the Claude app experience, native in the terminal,
using tmux.

This is a reconnaissance mission. It is built standalone from tsk, to explore what a
control list for Claude sessions needs. Later it folds back into tsk.

It is in M-BOOT's scope under the exception added to M-BOOT's constraints on
2026-09-25: tooling the user uses to run the bootstrap work itself.

## Decisions

- **Standalone first.** The tmux layout, the pane handling, and the Claude Code hooks
  that report status and tokens are a separate set of scripts, with a small Python list
  view. It follows the bootstrap ethos: build what is needed now, outside tsk, and move
  it into tsk once tsk has a place for it.
  Decided 2026-09-25.
- **Status indicator.** A permission prompt, waiting for input, a finished turn and an
  error all count as "needs attention", with one indicator for all of them. An empty
  circle means no attention needed. A full circle means the session needs attention.
  The little hand icon in the Claude app is the reference. "Doing work" as a pulsing
  circle is a later layer: tsk had that idea before, and getting it to work reliably
  was tricky. Decided 2026-09-25.
- **Token figure.** The control list shows the session's total tokens. Decided
  2026-09-25.
- **Session lifetime.** Phase one: sessions survive a tmux detach, which is
  standard tmux behaviour. Phase two: recovery after a tmux server restart or a machine
  restart. Mission Control rebuilds the layout and resumes each Claude session with
  `claude --resume`. Decided 2026-09-25.
- **Local only.** Phase one lists local Claude sessions only. Later: list cloud
  sessions too, and trigger a cloud session from Mission Control. Decided 2026-09-25.
- **Threads.** Phase one ignores tsk threads. Later, the list shows each session's
  thread. That depends on completing the bootstrap and moving the whole model into tsk
  itself, so that any repo can use tsk. Decided 2026-09-25.
- **nvim.** One nvim per Claude session, opened in that session's worktree. Decided
  2026-09-25.
- **Repo.** The scripts are kept in a new repo, `jimbarritt/tsk-mission-control`.
  Phase one needs no tsk knowledge: it uses tmux, Claude Code hooks and Claude Code's
  transcript files. When tsk shows these sessions later, tsk reads the scripts' status
  files, so the dependency goes from tsk to the scripts. Decided 2026-09-25.
  The repo is public. The user created it on 2026-09-25, empty. A cloud session attaches it with `add_repo`, access `push`:
  confirmed working 2026-09-25.
- **Command.** The command that sets up the layout is `tsk-mission-control`. Once the
  list moves into tsk, it becomes a flag: `tsk --mission-control`. Decided 2026-09-25.
- **Later, a view in the tsk TUI.** The list view moves into the tsk TUI once the tsk
  model has a place for a Claude session. The tmux layout and the hooks carry over
  unchanged.

## Open questions

1. Answered 2026-09-25, see Decisions: status indicator.
2. Answered 2026-09-25, see Decisions: token figure.
3. Answered 2026-09-25, see Decisions: session lifetime.
4. Answered 2026-09-25, see Decisions: local only.
5. Answered 2026-09-25, see Decisions: threads.
6. Answered 2026-09-25, see Decisions: nvim.
7. Answered 2026-09-25, see Decisions: repo.

## Intelligence

- The user's original statement of the idea, 2026-09-25, verbatim:

  > Ok so I have this idea. I want to create a tmux setup where I have the following
  > layout. On the left I want a list of Claude sessions , each running in its own tmux
  > pane. On the right will show the currently selected session. Stretched across the
  > bottom is a terminal window where I can just type whatever.
  >
  > This should all be within a single session.
  >
  > I want a command that launches this setup so my workflow is:
  >
  > Start a new tmux session.
  >
  > Type "Claude-session" or similar
  >
  > My tmux session configures itself, starts a new Claude session.
  >
  > I should be able to name each Claude session but it should come up with a good
  > default name, like the name of the current git repo.
  >
  > Each session will create its own worktree but I can do that manually.
  >
  > Ideally the index on the right will have some kind of status indicator so I know
  > when a session needs attention.
  >
  > Basically I want to replicate the Claude app experience but native in the terminal
  > using tmux.
  >
  > Also - there should be a shortcut key on the list that launches nvim in the root
  > directory and makes it zoomed in. Somehow when I zoom back out or I have a custome
  > keyboard shortcut that hides nvim but keeps it running and goes back to the three
  > panel layout.
  >
  > Ideally also I can see token consumption in the left panel the list.
  >
  > Maybe we need a special TUI for the control list.
  >
  > Maybe this is a view in the tsk TUI? I'm not sire how we would filter or represent
  > this in our model but it needs to be there.
  >
  > We could call the control list "Mission Control" or "mission command"

  The statement says "the index on the right" for the status indicator. The layout puts
  the list on the left. The objective reads it as the list.
- `docs/adr/0004-unified-tsk-binary.md` (in the tsk repo): the tsk TUI, the later home
  for the list view.
- `docs/kb/claude-code-mods.md` (in the tsk repo): Claude Code hooks, the source for
  status and token data.

## Decision authority

The user decides the open questions, the name of the control list, and the command name. All
three are settled, see Decisions.

The user approves the key bindings before T-05 and T-08 are built. The actor proposes
them.

The actor decides the rest of the implementation: the TUI library, the state file
format, the tmux mechanism for showing a session in the right pane, and how the hooks
are installed. The mission report records each choice and the reason for it.

## Constraints

- Standalone. No dependency on the tsk binary or the tsk daemon.
- Python for scripting.
- macOS only, for now. A cloud agent runs on Linux, so it tests there, and avoids anything that differs on macOS: GNU-only flags on `sed`,
  `date`, `stat` and `find`, and `inotify` for file watching. The user confirms
  macOS behaviour in T-09.
- Worktree creation per session is manual. The user does it.
- The agent that does the work runs in a session whose primary repo is
  `jimbarritt/tsk`, so it reads the missions and this plan from the ledger. It attaches
  `jimbarritt/tsk-mission-control` with `add_repo`, access `push`, and writes the
  product there.

## Out of scope

- tsk threads. Shown once the bootstrap completes and any repo can use tsk.
- Cloud sessions, listed or triggered. A later phase.
- Recovery after a tmux server restart or a machine restart, in phase one. It is phase
  two.
- The tsk TUI view. It follows once the tsk model has a place for a Claude session.
- Creating a worktree per session.

## Plan

Approved by the user 2026-09-25. Phase one only.

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Repo skeleton and install | `jimbarritt/tsk-mission-control` has a README, a Python project layout, and one install step that puts `tsk-mission-control` on `PATH`. The README lists the tmux, Python and Claude Code versions it needs | none | DONE |
| T-02 | Session state store | Each Claude session has one state file under an XDG state directory: name, tmux pane ID, worktree path, Claude session ID, status, total tokens. A module reads and writes it | T-01 | DONE |
| T-03 | Layout command | `tsk-mission-control`, run in a new tmux session, builds the three panes (list left, session right, terminal across the bottom) and starts the first Claude session, named after the current git repo unless a name is given | T-02 | DONE |
| T-04 | Mission Control list view | The left pane shows every Claude session in this tmux session by name. The list refreshes when a state file changes | T-02 | TODO |
| T-05 | New session and switching | From the list, one key starts a new Claude session with a name prompt that defaults to the git repo name. Selecting a session shows it in the right pane. Every other session keeps running in a pane out of view | T-03, T-04 | TODO |
| T-06 | Status hooks | Claude Code hooks write each session's status to its state file. A permission prompt, waiting for input, a finished turn and an error set "needs attention". Submitting a prompt clears it. The list shows an empty or a full circle. The hooks apply to Mission Control sessions only and leave the user's global Claude Code settings unchanged | T-04 | TODO |
| T-07 | Token totals | The list shows each session's total tokens, summed from the usage records in its Claude Code transcript file | T-06 | TODO |
| T-08 | nvim per session | One key on the list opens nvim in the selected session's worktree, zoomed. A second shortcut hides nvim, leaves it running, and returns to the three pane layout. Opening it again for the same session shows the same nvim | T-05 | TODO |
| T-09 | Confirm on macOS | The user installs it on macOS from the public repo and confirms each objective holds. What breaks is fixed and checked again. The mission report records what broke and each fix | T-01 to T-08 | TODO |

**Essential task:** T-09. The mission is done when T-09 is: every objective holds on macOS. The mission builds the thing; watching it in use afterwards is not part of it.

Notes for the actor:

- T-06: confirm which Claude Code hook event, if any, fires on an error before building
  the "needs attention" rule for it. If none does, record that in the mission report and
  continue with the other three states.
- T-06: the hook input carries `transcript_path`. Store it in the state file for T-07.
- Model: Sonnet 5 for execution, per M-BOOT's doctrine.
