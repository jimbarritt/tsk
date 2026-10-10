# 13. tsk works in any harness, and Mission Control stays a separate product

Date: 2026-10-06

## Status

Accepted. Replaces three M-BOOT-06 decisions of 2026-09-25: that Mission Control folds
back into tsk, that its command becomes `tsk --mission-control`, and that tsk reads
Mission Control's status files.

## Context

A harness here is the tooling a person runs agent sessions in: Claude Code itself, a
terminal multiplexer such as tmux or Herdr, an agent app such as Orca, or Mission
Control. tsk ships as the `tsk` binary and a Claude Code plugin. ADR 0011 puts all logic
in the binary, and gives each harness a thin adapter that calls it.

M-BOOT-06 builds Mission Control in `jimbarritt/tsk-mission-control`: a tmux layout with
a list of Claude sessions, a status indicator per session, token totals and an nvim per
session. Its briefing planned three later steps. Mission Control folds back into tsk.
Its command becomes the flag `tsk --mission-control`. Its list becomes a view in the tsk
TUI. The same briefing said tsk would read Mission Control's status files to show its
sessions, so tsk would depend on Mission Control.

Two products ship the same layer. Herdr (`docs/kb/orchestration-ecosystem/herdr.md`) is
a terminal multiplexer that tracks each agent's state and resumes agents after a restart.
Orca (`docs/kb/orchestration-ecosystem/orca.md`) is a desktop app with a session list,
agent state and resume. Neither models a mission, an objective, a ledger or thread
continuation. `docs/kb/orchestration-ecosystem/orca-integration.md` already treats Orca
as a delivery surface beside Mission Control, chosen by the person.

## Decision

1. tsk works in whatever harness a person uses. tsk depends on no harness. Each harness
   reaches tsk through an adapter that calls the `tsk` binary.
2. Mission Control stays a separate product, in `jimbarritt/tsk-mission-control`. It is
   one harness among several. It does not move into the tsk binary or the tsk TUI.
3. Mission Control depends on tsk, and tsk does not depend on Mission Control. Mission
   Control calls `tsk` commands and reads their output. tsk reads no Mission Control
   file. A session's status indicator and token total belong to Mission Control.

## Consequences

- A new harness needs an adapter and no change to tsk. The Claude Code plugin is the
  first adapter. `orca-integration.md` lists the places an Orca adapter attaches.
- When Mission Control needs data from tsk, such as the thread bound to a session or a
  code worktree, tsk provides it as a `tsk` command with tests. `tsk thread binding`
  already prints a binding.
- Mission Control runs without tsk installed. With tsk installed, it also shows tsk
  data, such as each session's thread.
- The flag `tsk --mission-control` is not built. The tsk TUI holds no Mission Control
  view.
- tsk competes with no harness. Its scope is the mission model, the ledger and thread
  continuation.

## Addendum, 2026-10-10

The research of 2026-10-05 to 2026-10-10 read five more surfaces: tmux, Herdr, Rex, Orca
and Efrit. Each is where a human sees and steers agent work, and none models a mission, an
objective, a ledger or thread continuation. They differ in openness and maturity: tmux
is open and nineteen years old, and Rex is a closed beta. That strengthens decision 1
and decision 3. The comparison is in `tsk-market-position-analysis.md`, under
"Surfaces: the core of tsk stays independent". Jim adopted OSC 7501, the Program Status
Protocol, on 2026-10-09 as the way `tsk` reports status to any surface that reads it.
