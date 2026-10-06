# Orca integration: seams for plugging tsk in

Research note on where tsk could attach to Orca (`docs/kb/orchestration-ecosystem/orca.md`
covers what Orca is) as a second delivery surface alongside tsk's own tmux-based Mission
Control (`M-BOOT-06` in the tsk ledger), so a user chooses either surface rather than tsk
building its own in competition with a tool developers are already adopting. Raised by
Jim, 2026-10-03.

Sources: a direct, shallow clone of [stablyai/orca](https://github.com/stablyai/orca) at
`ddcc279` and later, read directly against the actual TypeScript source, not secondhand.
Every file path below is read, not inferred. Orca was not installed or run; nothing here
is confirmed against the running app.

## The four seams, by effort

### 1. The worktree comment: already renders tsk's state, no plugin needed

`orca worktree set --worktree active --comment "..."` sets a free-text field that
renders as markdown. Confirmed in the renderer, not only the CLI help text:
`src/renderer/src/components/sidebar/WorktreeCardMeta.tsx` renders the comment through
`CommentMarkdownAsync` inside a hover card, and
`src/renderer/src/components/sidebar/WorktreeCardMetaBadges.tsx` shows a sticky-note
badge in the worktree card's always-visible row whenever a comment is set
(`hasComment`). The badge sits in the left sidebar at all times; the comment's own text
renders on hover over the card.

tsk's thread continuation already writes an actor name, a mission link, a task in
progress and a what's-next summary at every pause
(`docs/domain/session-continuation-design.md`). Formatting that into the comment field,
on every pause or every turn, puts exactly that information in Orca's own sidebar
through a mechanism Orca ships for this purpose, not one built for tsk. No plugin, no
change on Orca's side, no new protocol.

What this does not give: a dedicated always-visible field. The badge is always on; the
content is hover-revealed. One comment per worktree, not separate structured fields for
actor, mission and task.

### 2. `orca.yaml` hooks: bind on worktree creation, and a tab for tsk's own TUI

`src/shared/orca-yaml-hook-types.ts` defines `OrcaHooks.scripts.setup`, which "runs
after worktree is created", trust-gated the same way Claude Code gates an untrusted
repo's own commands: a content-hash approval
(`PersistedTrustedOrcaHookEntry`), not a blanket yes. A tsk-managed repo's `orca.yaml`
could run a tsk thread-start or bind command the moment Orca creates a worktree.

`OrcaHooks.defaultTabs` is a list of `{ title?, color?, command? }` entries, opened as
ordinary, persistent terminals at worktree creation
(`src/main/runtime/runtime-local-worktree-terminal-startup.ts`, which calls
`createTerminal`, not a one-shot script runner). Checked directly: the list is capped at
`MAX_ORCA_YAML_COLLECTION_ENTRIES` (256, `src/shared/orca-yaml-file-limit.ts`), and
`normalizeDefaultTabs` (`src/shared/orca-yaml.ts`) treats `command` as a plain string
with no distinction between a short script and a long-running process. The one related
limit found, `TERMINAL_TAB_HOT_RETAIN_LIMIT = 6`
(`src/renderer/src/components/terminal-pane/terminal-hidden-view-parking.ts`), governs
which terminal panes stay mounted in the DOM when not focused; the underlying PTY and
process keep running regardless, so it does not bound how many background tabs can run
a long-lived TUI.

So a `defaultTabs` entry with `title: "tsk"` and `command: "tsk tui"` opens tsk's own TUI
as a tab in every new worktree, alongside the agent's own terminal, the editor and the
browser pane. It needs no new binding mechanism: `docs/domain/session-continuation-design.md`'s
CLI worktree binding already resolves a thread from
`$(git rev-parse --git-dir)/tsk-thread-id` inside whichever worktree it runs in, so the
TUI resolves the right thread and mission for that worktree on its own.

### 3. A tsk `SKILL.md`: reaches every agent Orca runs, not only Claude Code

Orca bundles its own skills (`orca-cli`, `orchestration`, `linear-tickets`, ...) as
ordinary `SKILL.md` files under `skills/`, and ships `orca skills share` / `orca skills
install` (`src/cli/specs/skills.ts`) to bundle and distribute any discovered skill
directory, not only Orca's own, behind an unlisted link. A tsk `SKILL.md` teaching an
agent tsk's mission, thread and ledger model, shared through that existing mechanism,
reaches Codex, Gemini, OpenCode, whichever agent a given team runs under Orca, not only
Claude Code. No change to Orca's own code.

### 4. The plugin system: real, but too narrow today for a live dashboard

`orca-plugin.json` (`src/shared/plugins/plugin-manifest.ts`) lets a plugin contribute a
sandboxed HTML panel rendered in the sidebar's activity bar, commands, and a
subscription to a closed set of three events: `worktree.created`, `worktree.removed`,
`agent.status.changed` (`PLUGIN_EVENT_NAMES`). The manifest's own comment calls the
whole system "EXPERIMENTAL: no compatibility promises until pluginApi v1 freezes."

The capability model (`src/shared/plugins/plugin-capabilities.ts`) is a closed set of
seven kinds: `workspace:read` (name, branch and terminal list of the focused worktree
only), `terminal:send` (type into a visible terminal), `notifications:show`, `storage`,
`secrets`, `events:subscribe`, `settings:own`. The file's own comment: "Scoped kinds
(net:fetch hosts, process:exec globs) arrive in later phases", i.e. network access and
process spawning do not exist yet in v0. No public plugin-authoring guide exists in the
repo; `docs/` holds internal audits only (`docs/audits/plugin-worker-output-retention`,
`docs/audits/plugin-uninstall-log-retirement`), not a third-party guide.

A tsk panel showing live mission and ledger state, built from the three subscribable
events plus the worktree name and branch, is possible today. It could not drive
worktree creation or the orchestration layer itself: those stay the CLI's business.
Worth re-checking once pluginApi v1 ships.

## A structural fact that matters beyond these four

`AgentType`, the type Orca's own status tracking keys on, is defined as
`WellKnownAgentType | (string & {})` (`src/shared/agent-status-types.ts`): an open
string, not a closed enum. Orca's status board is not confined by its type system to the
roughly twenty agents it ships native adapters for; anything reporting a status under an
arbitrary `agentType` string is representable. Separately, the plugin manifest's
`contributes.agents` field (`pluginAgentProfileContributionSchema`) declares only a
file path today; nothing found wires its content into the agent launcher yet, so adding
a wholly new launchable agent type through the plugin system specifically is not yet a
real seam, distinct from the open `AgentType` on the status side.

## Projects and repos

`Project.sourceRepoIds: string[]` (`src/shared/project-types.ts`): a Project is usually
one git repository, but can be several tied together as a cluster, and `kind?:
RepoKind` covers a plain folder workspace with no git at all. Not a strict one-to-one
with a repo.

## Recommendation, ranked

Seams 1 and 2 need nothing from Orca: a tsk-side convention (write the comment at every
pause) and a repo's own `orca.yaml`. Seam 3 is a packaging exercise, a `SKILL.md`,
distributed through a mechanism Orca already ships. Seam 4 is real but immature;
track it, do not build against it yet.

## Open, not checked here

- Orca was not installed or run. Everything above is read from source, not observed in
  the app.
- Whether the `setup` hook's trust approval re-prompts on every change to the script
  tsk would ship (a content-hash gate implies yes, not confirmed against a running
  instance).
- Whether more than one `defaultTabs` entry can each bind independently to a
  distinguishable part of tsk's state, versus all seeing the same worktree-level thread.
- `pluginAgentProfileContributionSchema`'s consuming code, if any exists outside what
  was searched here.

## Sources

- [stablyai/orca](https://github.com/stablyai/orca), cloned directly: `LICENSE`,
  `src/shared/project-types.ts`, `src/shared/project-group-types.ts`,
  `src/shared/orca-yaml.ts`, `src/shared/orca-yaml-hook-types.ts`,
  `src/shared/orca-yaml-file-limit.ts`, `src/shared/agent-status-types.ts`,
  `src/shared/native-chat-agent-profiles.ts`, `src/shared/plugins/plugin-manifest.ts`,
  `src/shared/plugins/plugin-capabilities.ts`,
  `src/shared/plugins/plugin-content-pack-contributions.ts`,
  `src/main/plugins/plugin-host-methods.ts`,
  `src/main/plugins/plugin-artifact-validation.ts`,
  `src/main/effective-hook-config.ts`,
  `src/main/runtime/runtime-local-worktree-terminal-startup.ts`,
  `src/cli/specs/agent-hooks.ts`, `src/cli/specs/skills.ts`,
  `src/renderer/src/components/sidebar/WorktreeCardMeta.tsx`,
  `src/renderer/src/components/sidebar/WorktreeCardMetaBadges.tsx`,
  `src/renderer/src/components/terminal-pane/terminal-hidden-view-parking.ts`.

## Related

- [orca.md](orca.md): what Orca is, its model, and its existing comparison against tsk
  and Mission Control.
- `docs/domain/session-continuation-design.md`: the CLI worktree binding (`tsk-thread-id`
  in the git directory) that a tsk TUI tab or a setup hook resolves against.
- `missions/operational/M-BOOT-06-mission-control.md`, in the tsk ledger: tsk's own
  tmux-based control surface, the alternative this integration sits beside.
- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's wider
  position against other systems in this space.
