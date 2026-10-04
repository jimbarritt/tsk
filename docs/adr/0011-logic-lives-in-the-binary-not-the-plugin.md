# 11. Logic lives in the binary, not the plugin

Date: 2026-10-04

## Status

Accepted.

## Context

tsk ships as two artefacts: the `tsk` binary, and the Claude Code plugin in `plugin/`,
which holds the hooks and skills that run the harness in a session. Before M-BOOT-04 the
harness logic lived in bash scripts under `ops/local/` and in the hooks that called them.
M-BOOT-04 moves that logic into the binary.

The same behaviour written twice, once in the binary and once in a hook or skill, drifts.
The binary has unit and end-to-end tests. A hook script and a skill have none. A
behaviour in the binary also works outside Claude Code, from a terminal or another
harness.

## Decision

All logic lives in the `tsk` binary. A hook, a skill or a script in the plugin holds the
minimum: it calls `tsk` and passes the result to Claude Code. It does not parse ledger
files, run git commands, or hold text that the binary also holds.

The exception is logic that belongs to the Claude Code harness itself, such as the hook
event wiring in `hooks/hooks.json`, the instructions a skill gives the agent, or the
mapping of a `tsk` exit code to a hook decision. That logic stays in the plugin.

`ops/local/` holds scripts local to the tsk repository. It is not distributed. A script
there that tsk needs to distribute moves into the binary or the plugin.

## Consequences

- A new harness behaviour starts as a `tsk` command with tests, and the hook or skill
  calls it.
- Text shown to a session, such as the prompt for a session with no thread binding,
  has one copy, in the binary. A hook prints it through a `tsk` command.
- A harness other than Claude Code reuses the binary and needs only its own thin
  adapter.
