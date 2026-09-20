# Claude Code mods

Research note on "mods", a harness-level extension mechanism in Claude Code. Raised by
Jim, 2026-09-20, from
[github.com/anthropics/claude-code/tree/main/mods](https://github.com/anthropics/claude-code/tree/main/mods)
and a post naming Thariq Shihipar, a Claude Code engineer at Anthropic, at
[simonwillison.net/2026/Sep/18/thariq-shihipar](https://simonwillison.net/2026/Sep/18/thariq-shihipar/).
The Simon Willison post is blocked by this sandbox's egress proxy and not read
directly; everything below comes from the GitHub repository itself, read directly, and
from this container's own installed Claude Code.

## What a mod is

A mod is a Claude Code plugin whose behaviour is a hooks module with one entry point,
`register(on, options)`, that attaches functions of the shape `($, e, next)` to engine
events. This is a different surface from the `SessionStart`/`Stop` style hooks tsk's own
`CLAUDE.md` and `.claude/hooks/` already use: those fire at fixed lifecycle points, a mod
attaches to whichever engine events it names.

A mod is not a marketplace plugin. It ships inside the Claude Code binary itself, is not
listed in any marketplace, and is written against an early-access hooks API that can
change release to release without notice. Anthropic's own four mods are visible on
GitHub as source; a session cannot see the equivalent for any other Claude Code
installation's exact release the way it can query its own.

## The four built-in mods

| Mod | What it does |
|---|---|
| `sec-default` | Isolates an organisation's managed hooks, settings and tool policy from user-installed plugins, on managed machines and Team/Enterprise orgs |
| `diff` | The `/diff` command: a pane beside the transcript showing uncommitted changes, file by file, updating live as the session edits and runs things |
| `telemetry` | Adds `$.telemetry`, so a plugin can write first-party analytics rows through the engine; respects the account's analytics setting |
| `agents-md` | `AGENTS.md` as project instructions, alongside or instead of `CLAUDE.md` |

Each mod is a complete plugin: `.claude-plugin/plugin.json`, `hooks/hooks.json` naming
the module, TypeScript under `hooks/` typed against the engine's own type contracts, and
a `tests/` folder runnable with `claude plugin test mods/<name>`.

## `agents-md`, confirmed live in this container

This container runs Claude Code 2.1.278. `claude plugin details agents-md` returns the
mod directly:

```
agents-md
  Description: AGENTS.md as project instructions: by default loaded where the project
  has no CLAUDE.md; by its instructionFiles option, loaded beside CLAUDE.md, left out,
  or with the project instructions dropped
  Source: agents-md@builtin
```

Four loading modes, set as `pluginConfigs."agents-md@builtin".options.instructionFiles`
in settings, or via the deprecated `projectInstructions` key it still honours:

| Mode | Behaviour |
|---|---|
| `claude-md` | Only `CLAUDE.md` loads. The mod adds nothing. |
| `claude-md-or-agents-md` (default) | `AGENTS.md` loads only where no `CLAUDE.md` exists from root to working directory. Where one does, the mod defers entirely. |
| `claude-md-and-agents-md` | Every `AGENTS.md` up and down the tree loads alongside `CLAUDE.md`, framed the same way. Duplicate files, by path then content, load once. |
| `managed-only` | Project instruction files, checked-in or private, are dropped. Only the organisation's managed `CLAUDE.md` and engine memory remain. |

A secondary source, not verified against Anthropic's own release notes, dates the
default fallback to Claude Code 2.1.277. Consistent with this container's 2.1.278
carrying it live, not contradicted by anything checked directly.

## Why this belongs in tsk's own knowledge base

Two separate reasons, not one.

**It settles the `AGENTS.md` question directly.** tsk's `CLAUDE.md` could adopt
`claude-md-and-agents-md` today, in this exact container, with no wait on availability.
Whether to is a separate decision, tracked as its own idea rather than settled here.

**The hooks surface itself is the more consequential find.** tsk's `SessionStart` hook
fires once, at fixed points in a session's life. A mod attaches to whichever engine
events it names, inside the session, not only at its boundaries. If that API opens to
plugin authors beyond Anthropic's own four, it is a materially different substrate for
tsk's own hooks, thread binding, continuation, the fetch-path guard, than the
`SessionStart`/`Stop` surface tsk is built on today.

## Open, not settled here

- Whether the mods hooks API opens to plugin authors outside Anthropic, and when. The
  four mods on GitHub are all Anthropic's own; nothing found confirms or rules out
  third-party mod authorship.
- The `agents-md` mod's component inventory (`claude plugin details`) reports 0 hooks
  and ~0 always-on token cost, despite being a hooks-based mod by its own README. Not
  explained by anything read so far. Possibly `plugin details` does not count function
  hooks the way it counts a marketplace plugin's declared hooks, tools or skills; not
  confirmed.
- The Simon Willison post naming Thariq Shihipar was not read. What he said, and why
  Willison thought it worth a post, is unknown here.

## Sources

- [github.com/anthropics/claude-code/tree/main/mods](https://github.com/anthropics/claude-code/tree/main/mods),
  read directly.
- [github.com/anthropics/claude-code/tree/main/mods/agents-md](https://github.com/anthropics/claude-code/tree/main/mods/agents-md),
  read directly, for the loading-mode detail.
- This container: `claude --version` (2.1.278), `claude plugin details agents-md`, both
  run directly, 2026-09-20.
- [simonwillison.net/2026/Sep/18/thariq-shihipar](https://simonwillison.net/2026/Sep/18/thariq-shihipar/),
  blocked by this sandbox's egress proxy, not read.
