# Mission: Mission Control

| Field | Value |
|---|---|
| ID | M-BOOT-06 |
| Territory | agentic research |
| Assignee | Jim |
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
- The control list shows a status indicator per session, so Jim can see when a session
  needs attention.
- The control list shows token consumption per session.
- A shortcut key on the control list launches nvim in the root directory, zoomed in. A
  zoom out, or a custom shortcut, hides nvim, keeps it running, and returns to the
  three panel layout.
- Jim uses it at work.

The control list is called "Mission Control" or "mission command". Jim has not settled
which.

## Purpose

Parent: M-BOOT. Jim's aim: replicate the Claude app experience, native in the terminal,
using tmux. Jim needs this at work as soon as possible.

It is in M-BOOT's scope under the exception added to M-BOOT's constraints on
2026-09-25: tooling Jim uses to run the bootstrap work itself.

## Decisions

- **Standalone first.** The tmux layout, the pane handling, and the Claude Code hooks
  that report status and tokens are a separate set of scripts, with a small Python list
  view. It follows the bootstrap ethos: build what is needed now, outside tsk, and move
  it into tsk once tsk has a place for it. It also lets Jim use it at work without tsk.
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
  The repo is public, so Jim can clone it outside his GitHub user. Jim created it on
  2026-09-25, empty. A cloud session attaches it with `add_repo`, access `push`:
  confirmed working 2026-09-25.
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

- Jim's original statement of the idea, 2026-09-25, verbatim:

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

Jim decides the open questions, the name of the control list, and the command name.

## Constraints

- Standalone. No dependency on the tsk binary or the tsk daemon.
- Python for scripting.
- Worktree creation per session is manual. Jim does it.
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

To be drafted once the open questions are answered.
