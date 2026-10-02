# Claude Code mods

Status: announced in Claude Code 2.1.287, 2026-10-01, as "Claude Mods: plugins may now
modify deeper behavior". The API is early access and "may change between releases
without notice".

Sources: the Claude Code [changelog](https://code.claude.com/docs/en/changelog), the
[`mods` folder](https://github.com/anthropics/claude-code/tree/main/mods) of the
`anthropics/claude-code` repository, and two files that Claude Code 2.1.287 wrote into
this container when its `plugin-authoring` skill loaded: `reference.md` (the long form)
and `types/claude-code.d.ts` (the API declaration, 20,168 lines). The declaration file
is the authority for the build that wrote it. Quoted phrases were matched against those
files. Two commands were run in this container: `claude plugin validate` on a one-hook
test plugin, and `claude plugin details`. Nothing was loaded into a live session.

## What a mod is

A mod is a Claude Code plugin whose behaviour is a hooks module. The module exports
`register(on, options)`. `on(event, matcher?, hook)` adds a hook. Every hook is a function
`($, e, next)`:

| Argument | Meaning |
|---|---|
| `$` | The engine interface. Each call is one noun and one method, such as `$.fs.read` or `$.ui.toast`. |
| `e` | The event's input, a plain frozen value. |
| `next(e)` | Runs the plugins beneath, then the engine's own behaviour, and resolves to the event's result. |

A hook that returns without calling `next` answers for itself. A hook that calls
`next({ ...e, x })` rewrites what the rest of the chain sees, within what that event
allows. A hook that throws is skipped and the chain continues.

The module runs in an environment of its own, with no DOM and no Node. It has no
`require`, and a module that holds `import()` does not load. Everything outside the
module is reached through `$`. A plugin file is TypeScript or JavaScript and is an ES
module. JSX compiles against a global `h`.

The announcement is one line in the changelog. The details below come from the files in
this container.

## What a mod can hook

The declaration file names 103 dotted event names. Each is an event a hook receives and,
for most, the matching call on `$`. A plugin's own `$.fs.write(...)` "is a dispatch the
hooks above it see". The names fall into these groups.

| Group | Events | What a hook can do |
|---|---|---|
| Tool | `tool.call`, `tool.check`, `tool.describe`, `tool.list`, `tool.register` | Deny a call with `{ deny: reason }`, answer it with `{ result }`, rewrite its input, or act on its result. `tool.register` adds a tool the model calls, listed as `mcp__<plugin>__<name>`. A matcher such as `{ tool: "Bash" }` narrows the hook. |
| Prompt | `prompt.submit`, `prompt.compose`, `prompt.context`, `prompt.section`, `prompt.attachment`, `prompt.edit`, `prompt.fill`, `prompt.read`, `prompt.suggest` | Rewrite a submitted prompt. Add, replace, reorder or drop system prompt sections (`prompt.compose` resolves to `{ sections }`, each `{ id, text, scope }`). Change the context blocks the first user message holds (`prompt.context`). |
| Session | `session.start`, `session.end`, `session.append`, `session.send`, `session.receive`, `session.compact`, `session.messages`, `session.usage`, `session.cwd`, `session.model`, `session.repo` and others | Run once at start. Rewrite or append a conversation row. Redact a message sent to another agent. Read the transcript as `{ role, text, toolUses }` rows. |
| Turn | `turn.start`, `turn.step`, `turn.complete`, `turn.abort` | Show text beneath an answer on completion. `turn.step` and `process.spawn` stream and are async generators. |
| Interface | `ui.render`, `ui.open`, `ui.close`, `ui.status`, `ui.toast`, `ui.log`, `ui.press`, `ui.input`, `ui.select`, `ui.focus`, `ui.copy` and others | Draw panes, a band above the prompt, a status line entry and toasts. Replace what the engine draws for a component. Four surfaces: `terminal`, `desktop`, `vscode`, `mobile`. |
| Command and agent | `command.register`, `command.run`, `agent.register`, `agent.spawn`, `agent.offer`, `skill.prompt` | Add a slash command. Add an agent type, listed as `<plugin>:<name>`. Run a subagent. |
| Model | `model.complete`, `model.fork`, `model.classify` | One completion with no history. A tool-less question over the session's own transcript, served from the prompt cache. A classification of text against a list of labels. |
| Files, processes, network | `fs.read`, `fs.write`, `fs.list`, `fs.stat`, `process.run`, `process.spawn`, `http.fetch`, `mcp.call`, `mcp.connect` | Read and write files, run a host command by argv, fetch a URL, call an MCP server. |
| State and time | `state.get`, `state.set`, `store.get`, `store.set`, `clock.every`, `clock.after`, `clock.sleep`, `env.get` | `$.state` holds named values for the session. `$.store` keeps values across sessions. Timers run until cancelled or until the module reloads. |
| Admission | `plugin.register`, `engine.create`, `settings.read`, `config.*`, `telemetry.log`, `telemetry.mark` | Refuse another plugin with `{ refuse: reason }`. Add a noun to `$`. |
| Settings hooks | `classic.<Event>`, such as `classic.Stop`, `classic.SessionEnd`, `classic.PreToolUse` | Hook the events that `settings.json` command hooks use. `e` is what that hook receives on stdin. |

Three facts about timing:

- A hook runs inside one dispatch, with a time budget of its own. `next.signal` aborts when
  the dispatch is abandoned. Work that outlives a dispatch starts in a `session.start`
  hook and runs on `$.clock.every` or `$.clock.after`.
- `$.prompt.submit` queues a prompt that starts a turn once the session is idle, so
  background work can wake a quiet session.
- `session.end` fires on exit, `/clear`, resume, logout, signal and the end of a `-p` run.
  The whole chain shares "one short wall-clock bound", which `next.budget` reads.

## Tiers

Hooks nest in five tiers, outermost first. An earlier tier has more authority.

| Tier | Who provides it |
|---|---|
| `prepend` | Managed plugins an administrator lists, outermost. |
| `user` | Every plugin a person installs. |
| `append` | Managed plugins an administrator lists, after the user's. |
| `builtin` | Plugins bundled in the binary. |
| `core` | The engine's own behaviour, innermost. |

A user hook cannot skip a tier with more authority. The `sec-default` mod keeps an
organisation's classic hooks, prompt content, managed settings, tool policy and deny
rules beyond the control of installed plugins. A `plugin.register` hook sees each module's
tier, name, provenance and a scan of the events and `$` calls it uses, and returns
`{ refuse: reason }` to keep it out. Provenance is `<name>@<marketplace>` for an
installed plugin, `<name>@inline` for `--plugin-dir`, and `<name>@builtin` for a bundled
plugin.

## Packaging and loading

A mod is three files in one folder:

| File | Content |
|---|---|
| `.claude-plugin/plugin.json` | `name`, `version`, `description`. Optional `userConfig`, `dependencies` and `types`. |
| `hooks/hooks.json` | `{ "modules": ["./register.ts"] }`: one path. |
| `hooks/register.ts` or `.tsx` | `export const register: Register = (on, options) => { ... }`, with `Register` imported from `claude-code`. |

A mod that keeps values in `$.state` adds `types/index.d.ts`, its type contract, named in
`plugin.json`. A mod that adds a noun to `$` in `engine.create` ships that noun's types the
same way. Another plugin lists it under `dependencies`.

Ways to load one:

| Way | Behaviour |
|---|---|
| Hot reload | The `plugin-authoring` skill watches a `dev-mods` folder under `~/.claude`. The first file written there makes Claude Code ask the person "Enable hot reloading for this session?" Only the person can answer. On yes, the mod loads when the turn ends. Each later edit reloads it when its turn ends. The result can also be off, because nobody could be asked (as under `claude -p`), because of organisation policy, or because the workspace is untrusted. |
| `claude --plugin-dir <folder>` | Loads for that session. The folder is watched in an interactive session. |
| `CLAUDE_CODE_PLUGIN_DIRS` | The same folders, for a host that cannot pass a flag. |
| Skills folder | `~/.claude/skills/<name>` and the project's `.claude/skills/<name>` are watched and reloaded the same way. |

A reload runs `register` again and fires `session.start` again. `$.state` and `$.store`
values stay. The module's own variables start over. Options come from `pluginConfigs` in
settings, keyed by plugin name. The mods README adds: "hooks modules load only where
function hooks are enabled".

Commands:

- `claude plugin validate <folder>` reads the manifest and the module's source and lists
  what the module hooks and calls. Run here on a plugin with one `turn.complete` hook, it
  printed `hooks: turn.complete` and `calls: $.ui.status`, and passed with one warning
  about missing author information.
- `claude plugin test <folder>` runs the plugin's `*.test.ts` files against the engine.
  A test imports `test`, `expect` and `mock` from `claude-code/testing`. `mock` answers
  `$.clock`, `$.store` and `$.env` from memory. A UI test mounts a component on a named
  surface.
- `claude --debug` writes a line for every hook failure and every result the engine rejected. While a
  folder hot-reloads, the transcript shows one dim line for each.
- `tsc -p <folder>` type-checks a mod, using the declarations Claude Code lays in
  `.claude-plugin/types/`.

## Built-in mods

| Mod | What it does |
|---|---|
| `sec-default` | Keeps managed policy beyond the control of installed plugins. Outermost, on a machine with managed settings or for a Team or Enterprise organisation. |
| `diff` | `/diff`: uncommitted changes in a pane beside the transcript, refreshed as Claude edits files. |
| `telemetry` | Adds `$.telemetry` so a built-in plugin can record a first-party analytics row. Rejects installed plugins. Sends nothing where analytics are off. |
| `agents-md` | `AGENTS.md` as project instructions, set by the `instructionFiles` option: `claude-md`, `claude-md-or-agents-md` (default), `claude-md-and-agents-md` or `managed-only`. |
| You should know | Added in 2.1.287. A side agent watches the session and flags what the person or Claude might miss, above the prompt. Enabled with `/plugin enable cc-plugin-you-should-know@builtin`, for first-party sessions with telemetry on. |

The first four are published as source in the `mods` folder. In this container, on
2.1.287, `claude plugin details` reports "Hooks (0)" for You should know. It also reports
`agents-md` and `diff` as "not found" by that name. Function hooks are probably not
counted in the component inventory, but no source says so.

## Comparison with settings.json hooks

| Aspect | `settings.json` command hooks | Mods |
|---|---|---|
| Form | A shell command that reads JSON on stdin and writes a decision. | A function in a TypeScript module, with the engine's `$` in scope. |
| Events | Fixed lifecycle points: `SessionStart`, `Stop`, `PreToolUse` and the rest. | The same events as `classic.<Event>`, plus about 100 more. |
| Output | Allow, deny, block or add context. | Also rewrite a prompt, a system prompt section, a conversation row or a tool input, and draw interface elements. |
| State | A file or an environment variable. | `$.state` for the session and `$.store` across sessions. |
| Model access | None. | `$.model.complete`, `$.model.fork`, `$.model.classify` on the session's own client. |
| Authority | Managed settings hooks run first. | The five tiers. Managed plugins sit outside and inside the user tier. |
| Stability | Documented and versioned. | Early access. |

## Current use in swe and tsk

| Plugin | What it uses today |
|---|---|
| `swe` 0.10.1 (cached copy read here) | Three `PreToolUse` command hooks: on `Bash`, on `Artifact` and on the outbound-message MCP tools of Gmail, Google Drive and Slack. An output style. A lint script with options named `--transcript`, `--reply-file` and `--stop-hook-active`. |
| tsk (`.claude/settings.json`) | A `SessionStart` command hook that fetches `tsk/bootstrap`. A `Stop` command hook, `thread-binding-guard.sh`. |

## Mechanisms that could apply to swe and tsk

Not designed, not built and not tested. Each row states the mod mechanism and the
requirement it touches.

| Mechanism | Requirement it touches |
|---|---|
| `prompt.compose` appends a `session`-scoped system prompt section | swe: apply Software English without relying on an output style being selected. |
| `turn.complete` returns `{ text }` shown beneath the answer | swe: show a lint result under a reply. The transcript's record is not rewritten. |
| `session.append` rewrites a row before it is stored | swe: change the text of a row. The model, the transcript file and the next request all read the row as rewritten. |
| `model.classify(text, labels)` | swe: the inference tier of the linter, with no subprocess. |
| `ui.render` on `AbovePrompt` returns a tree | swe or tsk: a persistent band for lint state or thread state. |
| `tool.call` with `{ tool: "Bash" }` returns `{ deny }` | tsk: refuse a hand-run `git fetch origin tsk/bootstrap` that `CLAUDE.md` forbids. The `swe` Bash hook does the same for its own rules in shell. |
| `session.start` runs awaited before the first prompt | tsk: the bootstrap fetch and thread binding now in the `SessionStart` shell hook. |
| `session.end` fires on exit and `/clear`, under a short time bound | tsk: record a continuation entry. The bound limits what can run. |
| `$.store` and `$.state` | tsk: thread binding that survives a context reset. |
| `$.clock.every` plus `$.session.messages()` | tsk: detect a session that went idle and reset. |
| `$.prompt.submit` from a timer | tsk: wake a quiet session. |
| `$.tool.register` and `$.agent.register` | tsk: thread and mission commands as tools or agent types of a plugin. |

## Not documented or not tested

- Whether a mod installed from a marketplace loads function hooks in an ordinary session.
  Provenance `<name>@<marketplace>` and the statement that `*` "does not select" a telemetry
  event "for an installed plugin" imply it can. It was not tested.
- Whether a Claude Code cloud session can enable function hooks. Hot reloading needs a
  person to answer a prompt. The `plugin-authoring` skill loaded in this cloud session,
  and the answer was not requested.
- The meaning of "where function hooks are enabled" in the mods README. No setting by
  that name was found in the files read.
- How a mod is published to others. The mods README says its four mods "are not listed in
  this repository's marketplace".
- Whether `claude plugin details` counts function hooks.
- Whether the API stays stable. The files call it early access.
- Any cost of a mod in tokens or latency, apart from `claude plugin details` showing a
  projected token cost for plugins.

## Sources

- [Claude Code changelog](https://code.claude.com/docs/en/changelog), version 2.1.287,
  2026-10-01
- [anthropics/claude-code, `mods`](https://github.com/anthropics/claude-code/tree/main/mods)
  and its [README](https://github.com/anthropics/claude-code/blob/main/mods/README.md)
- `SKILL.md`, `reference.md` and `types/claude-code.d.ts` of the `plugin-authoring`
  skill, written by Claude Code 2.1.287 into this container
- `claude plugin validate`, `claude plugin details`, `claude --version`, run here

## Related

- [docs/index.md](../index.md): knowledge base index.
- [typesafe-jev-classifier.md](typesafe-jev-classifier.md): the classifier-in-the-loop
  pattern. `model.classify` is the mod mechanism nearest to it.
- [docs/domain/session-continuation-design.md](../domain/session-continuation-design.md):
  thread binding and continuation, the design a mod's session events would touch.
