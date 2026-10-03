# Mission: Session names from Culture ship Minds

| Field | Value |
|---|---|
| ID | M-NAMES |
| Territory | agentic research |
| Assignee | unassigned |
| Blocked by | none |

Skeleton. The objective lines are drafted from the captured idea and wait on the open
decisions below.

## Idea, as captured

Raised by Jim, 2026-10-03:

> I have a new idea: I want us to write a plugin that can give our sessions or web
> sessions names based on the ai / ship names from Ian m banks so when they talk to each
> other they have identities.

## Objective

- A Claude Code session started with the plugin installed has a custom title that is a
  Culture ship name.
- A second session on the same machine sends a message to the first by that name, and the
  first receives it.
- A cloud session started with the plugin installed has a custom title that is a Culture
  ship name.
- Two live sessions do not hold the same ship name.

## Purpose

Parent mission: TBD. Related to M-BOOT-06, Mission Control, which names each Claude
session, and to cross-session messaging between sessions.

## Intelligence

- `docs/kb/session-names-and-culture-ship-names.md` in the tsk repo: how Culture ships are
  named, how Claude Code names, lists and addresses sessions, and what is not established.
- `docs/kb/claude-code-plugin-packaging.md` in the tsk repo: packaging a plugin and
  loading it in cloud sessions.
- `docs/kb/claude-code-mods.md` in the tsk repo: hooks and plugin tiers.
- [future-missions-tbd.md](../../future-missions-tbd.md), "Session names from Iain M.
  Banks ship Minds": the idea as first captured.

## Decision authority

Jim.

## Constraints

- A name works as a `SendMessage` target and an `@` mention.
- A name is at most 200 characters and does not start with `/`.

## Out of scope

- TBD.

## Plan

| ID | Task | Objective, in short | Delegated to | Blocked by | Status |
|---|---|---|---|---|---|
| T-01 | Ship name source | The facts on how Culture ships are named, and a sample list with sources, are in the KB | none | none | DONE |
| T-02 | Claude Code naming mechanisms | The KB states how a session is named, listed and addressed | none | none | DONE |
| T-03 | Cloud title from a plugin hook | A test shows whether a plugin SessionStart hook sets the title of a cloud session | none | none | TODO |
| T-04 | Hook names under Remote Control | A test shows whether a hook-set name is listed while a session is connected to Remote Control | none | none | TODO |
| T-05 | Quoting | A test shows whether `SendMessage` accepts a name with spaces, commas and an ellipsis | none | none | TODO |
| T-06 | Full name list | A complete list of ship names exists with a source for each | none | none | TODO |
| T-07 | Choice of name | How a session gets its name is decided | none | Open decisions | TODO |

**Essential task**: T-07. Without a rule for choosing a name there is nothing for the
plugin to apply.

## Open decisions

1. What the name belongs to. Decided 2026-10-03: the actor. Jim:

   > I think it's the actor but maybe we don't have a clean enough definition of what an
   > actor is.
   >
   > It's not a session because these are ephemeral.
   >
   > And like you say I don't think it's a thread because multiple actors can be working
   > on the same thread or picking up work.
   > So actor is a concept of something that runs either in iOS like here or in a terminal,
   > can span multiple sessions and has a position and possible certain skills and memory
   > if its own.

   The objective lines above name a session, and are not yet revised for this decision.
   The definition of an actor is open: see "Actor definition" in
   [future-missions-tbd.md](../../future-missions-tbd.md).
2. How a name is chosen: random from a list, drawn from a hash of something stable, or
   chosen by the session.
3. Which ships and Minds the list holds: all names, or a subset by class or tone.
4. Whether the plugin names only the main session, or also subagents and teammates.
5. Whether a name includes the class prefix (GSV, GCU, ROU) or only the name.
6. Where the plugin lives, given the plugin and binary order recorded under M-BOOT.
