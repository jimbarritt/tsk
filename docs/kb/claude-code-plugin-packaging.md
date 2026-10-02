# Claude Code plugin packaging and distribution

Status: reference for packaging a plugin and hosting it in a marketplace. Read against
Claude Code 2.1.287, 2026-10-02.

Sources: the Claude Code documentation pages listed under Sources, fetched as raw HTML
and read as text, and two observations in this cloud container: `~/.claude/plugins/` and
`ops/local/claude-session-start.sh`. A statement from the documentation is marked as
such. Nothing was published or installed for this note.

## What a plugin holds

A plugin is a folder with a `.claude-plugin/plugin.json` manifest. Each component has a
default location.

| Component | Location |
|---|---|
| Skills | `skills/<name>/SKILL.md` |
| Commands | `commands/*.md` |
| Agents | `agents/*.md` |
| Hooks | `hooks/hooks.json` |
| MCP servers | `.mcp.json` |
| LSP servers | `.lsp.json` |
| Monitors | `monitors/monitors.json` |
| Output styles | `output-styles/*.md` |
| Themes, workflows | `themes/`, `workflows/` |
| Executables | `bin/` |
| Settings | `settings.json`. Only `agent` and `subagentStatusLine` take effect. |

A mod is a plugin whose `hooks/hooks.json` names a TypeScript or JavaScript module. See
[claude-code-mods.md](claude-code-mods.md). A plugin can hold a mod, skills and an MCP
server together.

A `CLAUDE.md` at the plugin root is not loaded as context. `claude plugin validate` warns
when it finds one. Instructions that load into context go in a skill.

The manifest `dependencies` field lists other plugins that must be enabled. Claude Code
installs and enables them with the plugin. A dependency on a plugin in another
marketplace needs that marketplace named in `allowCrossMarketplaceDependenciesOn`.

## Binaries

The manifest reference has no field for a system prerequisite or a binary. A plugin gets
a binary in one of these ways.

| Mechanism | Behaviour |
|---|---|
| `bin/` | Files here are on the Bash tool's `PATH` while the plugin is enabled, so Claude runs them as bare commands. The binary is part of the plugin. claude.ai and Cowork do not install a plugin that has this directory. |
| `${CLAUDE_PLUGIN_DATA}` | `~/.claude/plugins/data/<id>/`. Created on first reference. Kept across plugin updates. Deleted on uninstall from the last place the plugin is installed. The documentation names its use as "installed dependencies such as node_modules, generated code, and caches". Hook commands and MCP stdio servers receive it as an environment variable. Commands that Claude runs through the Bash tool do not. |
| `${CLAUDE_PLUGIN_ROOT}` | The installed version's directory. It changes on update, so state is not written there. |
| MCP stdio server | `command`, `args` and `env` accept `${CLAUDE_PLUGIN_ROOT}` and `${CLAUDE_PLUGIN_DATA}`. The command can be any program on the machine. |
| `npm` source | Claude Code fetches a package with the user's npm client. Install scripts never run, and dependencies install only from a supported lockfile, also without scripts. |
| `archive` source | A zip over HTTPS, up to 256 MiB, with an optional `sha256` pin. |
| `command` source | A command on the user's machine prints the plugin directory. Claude Code shows the user the command before it runs. An administrator can turn command sources off. |

A hook that downloads a binary into `${CLAUDE_PLUGIN_DATA}` follows from the variable's
stated use. The documentation does not describe it as a pattern for binaries.

## Marketplace

A marketplace is a directory, usually a git repository, with `.claude-plugin/marketplace.json`.

| Field | Meaning |
|---|---|
| `name` | Required. Forms the part after `@` in every plugin ID. |
| `owner` | Required. `name` is required. |
| `plugins` | Required. One entry per plugin. |

An entry needs `name` and `source`. It also accepts the `plugin.json` fields. The plugin
ID is `<entry name>@<marketplace name>`. Each user registers one marketplace per name.

Plugin source types:

| Type | Fields | Use |
|---|---|---|
| Relative path | the string | A directory inside the marketplace repository. |
| `github` | `repo`, `ref`, `sha` | A GitHub repository. The repository root is the plugin root. |
| `url` | `url`, `ref`, `sha` | Any git repository. |
| `git-subdir` | `url`, `path`, `ref`, `sha` | One subdirectory of a repository, fetched with a sparse checkout. |
| `npm` | `package`, `version`, `registry` | An npm package. |
| `archive` | `url`, `sha256` | A zip. |
| `command` | `command`, `timeout`, `mode` | A directory a local command prints. |

`ref` is a branch or tag. `sha` is a full 40-character commit. When both are set, Claude
Code checks out `sha`.

A relative path resolves only when Claude Code has the marketplace's files. It does not
resolve when users add the marketplace as a bare `marketplace.json` URL.

Reserved marketplace names include `npm`, `pip`, `uv`, `cargo`, `github`, `gh`, the
official Anthropic names, and any name that starts with `claudeai-`.

## Versions and updates

| Case | Behaviour |
|---|---|
| `version` set in `plugin.json` | It wins over the entry's `version`. Users keep the cached copy until the string changes, however many commits are pushed. |
| `version` omitted everywhere | For `github`, `url` and `git-subdir` sources the version is the source's commit SHA, shortened to 12 characters. Users track commits. |
| `version` in both places | `plugin.json` is used and `claude plugin validate` warns. |
| Pin | `ref` and `sha` on the entry hold users on one commit. |
| Release channels | Two marketplaces with different `name` values, whose entries point at different refs. Claude Code has no channel concept. |
| Auto-update | Off by default for every marketplace except the official ones and those added from claude.ai. A user or an administrator turns it on. `marketplace.json` has no field for it. |
| Without auto-update | Users run `/plugin marketplace update <name>`, or `claude plugin update <plugin>@<name>`. |
| Rename | `name` is the identifier. Changing it breaks installs. A top-level `renames` map migrates users. `displayName` changes the label only. |

A session keeps the plugin versions it loaded. An update applies at the next session, or
at `/reload-plugins`.

## Where plugins load

| Place | Behaviour |
|---|---|
| Terminal, desktop app, VS Code | Plugins install from a marketplace at user, project or local scope. Mods run in all of these. Panes and bands draw in the terminal and the desktop app only. |
| Cloud session, per the documentation | "doesn't load the plugins you installed on your own machine or the ones your repository's .claude/settings.json turns on". A cloud session does not add the marketplaces a repository lists under `extraKnownMarketplaces`. That needs a workspace trust dialog, and a cloud session never shows one. |
| Cloud session, observed here | `swe@jimbarritt-claude-plugins` 0.16.4 is installed at project scope for `/home/user/tsk` and enabled. `known_marketplaces.json` lists the `jimbarritt/claude-plugins` GitHub marketplace. |

The observation follows from `ops/local/claude-session-start.sh`, which runs at session
start and calls, in order, `claude plugin marketplace add jimbarritt/claude-plugins`,
`claude plugin install swe@jimbarritt-claude-plugins --scope project -y`,
`claude plugin marketplace update` and `claude plugin update`. The script's own comment
states that the `extraKnownMarketplaces` and `enabledPlugins` settings "only declare
intent". The `install` call does not upgrade an installed plugin, so the `update` call is
a separate step. The command-line route works in a cloud session. The settings route does
not, per the documentation.

## The jimbarritt marketplace today

`jimbarritt/claude-plugins` has `.claude-plugin/marketplace.json` with `name`
`jimbarritt-claude-plugins`, owner Jim Barritt, and one entry: `swe` with source
`./swe`, a relative path. The cache in this container holds `swe` versions 0.10.0 to
0.16.4.

## Source in one repository, entry in another

An entry can point at a repository other than the marketplace's own. Two forms fit
a plugin whose source is in the `tsk` repository.

| Form | Entry | Requirement |
|---|---|---|
| `github` | `{ "source": "github", "repo": "jimbarritt/tsk", "ref": "main" }` | The `tsk` repository root is the plugin root, with its own `.claude-plugin/plugin.json`. |
| `git-subdir` | `{ "source": "git-subdir", "url": "https://github.com/jimbarritt/tsk", "path": "plugin" }` | The plugin is in a subdirectory. Claude Code checks out that directory only. |

With `version` omitted, each commit to the ref is a new version, and the marketplace
entry needs no edit for a release. With `ref` and `sha` set, a release is an edit to the
entry. The entry then pins the commit.

A marketplace name change, such as a move to another organisation's marketplace, changes
every plugin ID. Users add the new marketplace and install again. Cloud sessions start
from `ops/local/claude-session-start.sh`, so the names in that script change too.

## Not documented or not tested

- A prerequisite field for a binary. None was found in the manifest reference.
- A binary download from a hook. It follows from the `${CLAUDE_PLUGIN_DATA}` description
  and was not run.
- Whether `bin/` works in a cloud session. The documentation excludes only claude.ai and
  Cowork.
- The `git-subdir` and `github` forms against the `tsk` repository. Nothing was published.
- Why the documentation says a cloud session does not load the repository's plugins while
  this session loaded one. The explanation above is inferred from the script.
- Windows behaviour of `bin/`.
- The `ubiqtek.ai` host. It was not researched.

## Sources

- [Plugin manifest reference](https://code.claude.com/docs/en/plugins-reference)
- [Marketplace reference](https://code.claude.com/docs/en/plugins/marketplace-reference)
- [Host and maintain a marketplace](https://code.claude.com/docs/en/plugins/host-marketplace)
- [Plugin loading reference](https://code.claude.com/docs/en/plugins/loading)
- [Install and manage plugins](https://code.claude.com/docs/en/discover-plugins)
- [Mods overview](https://code.claude.com/docs/en/plugins/mods/overview)
- [jimbarritt/claude-plugins](https://github.com/jimbarritt/claude-plugins),
  `.claude-plugin/marketplace.json`
- This container: `~/.claude/plugins/installed_plugins.json`,
  `~/.claude/plugins/known_marketplaces.json`, `claude plugin list --json`, and
  `ops/local/claude-session-start.sh`

## Related

- [claude-code-mods.md](claude-code-mods.md): the function-hooks API a plugin can hold.
- [docs/user-guide/installation.md](../user-guide/installation.md): how the `tsk` and
  `tskd` binaries install today, with `cargo install`.
