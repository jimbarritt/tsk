# Mission: Environment and nexus setup

| Field | Value |
|---|---|
| ID | M-ENV |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | none |

## Objective

Kind: attainable

- A fresh repo on a machine or Claude cloud environment that has `tsk` installed gets the
  marketplace and the `tsk` plugin from one `tsk` command.
- A machine attaches and lists its nexus with `tsk nexus add <url>` and `tsk nexus list`,
  so an environment's setup script loads the nexus with no manual edit.
- A repo registers itself in the nexus with `tsk nexus register-repo`, and a session runs
  that command through a plugin skill once it is bootstrapped.
- A Claude cloud environment is created from a user guide and a copy-and-paste setup
  script, and its setup result is known to be cached across sessions.

## Purpose

Today each of these steps is a hand edit or a copied shell sequence: the plugin steps in
`ops/local/claude-session-start.sh`, `tsk config attach-nexus`, and `nexus.json` edited by
hand in the nexus repo (`docs/user-guide/new-nexus-on-a-clean-machine.md`, steps 4 and 7).
Raised by Jim on 2026-10-08, while writing the cloud environment user guide. M-BOOT-04
deferred the registration command from its T-10 list; this mission takes it.

## Intelligence

- `docs/user-guide/new-cloud-environment.md` on `main`: the environment steps, the setup
  script, and what was observed on 2026-10-08.
- `docs/user-guide/new-nexus-on-a-clean-machine.md` on `main`: the manual nexus and plugin
  steps these commands replace.
- `cli/src/config.rs`, `cli/src/ledger/nexus.rs`, `cli/src/ledger/location.rs` on `main`:
  the user config, the `nexus.json` model and the nexus checkout.
- `ops/local/claude-session-start.sh` on `main`: the plugin install and update sequence.
- `plugin/` on `main`: the plugin's skills and hooks.
- `docs/domain/ledger-layout.md` on `main`: the nexus URL normalisation and ledger refs.

## Decision authority

Jim decides the command names and when a release is published. The executing actor
decides the implementation within those names.

## Constraints

- All logic lives in the `tsk` binary. The skill calls it and holds no logic
  (`docs/adr/0011-logic-lives-in-the-binary-not-the-plugin.md`).
- Every command is idempotent: a second run with the same input changes nothing and
  exits 0.
- A command that writes to the nexus repo uses the machine's own git credentials.
- No code comments. British English. New commands have unit and end to end tests, and
  update the user guides, `docs/index.md` and the clap help text.

## Out of scope

- Moving an in-repo ledger worktree to the nexus.
- The `local` to `url` transition for a repo that gains a remote.
- More than one nexus per machine.
- Publishing a release. T-05 holds it for Jim.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | `tsk install-plugin claude-cli` | In a fresh repo, the command adds the `jimbarritt/claude-plugins` marketplace, installs and updates the `tsk` plugin at a chosen scope through the `claude` CLI, and exits 0 again when run twice | none | TODO |
| T-02 | `tsk nexus add <url>` and `tsk nexus list` | `add` records the nexus URL in the user config. `list` prints the attached nexus URL and the territories and repos its `nexus.json` indexes | none | TODO |
| T-03 | `tsk nexus register-repo` and the `register-repo` skill | Run in a repo, the command adds the repo's entry to `nexus.json` in the nexus, commits and pushes it. `/tsk:register-repo` runs it from a session | T-02 | TODO |
| T-04 | Cloud environment guide and setup script | `docs/user-guide/new-cloud-environment.md` holds a setup script that installs `tsk` and records its own run, and the cache behaviour is known | none | IN PROGRESS: the guide and script are on `main`. Setup runs as root, and a new session reuses the setup result (2026-10-08). Cache expiry and invalidation on edit are untested. The script records whether it installed cargo, not yet read |
| T-05 | Release | The version is bumped, `tsk-core` and `tsk-bin` are published, `plugin/tsk-version` is bumped to match, and the cloud setup script calls `tsk nexus add` | T-01, T-02, T-03 | TODO, Jim decides when |

**Essential task**: T-03. A new repo on a new machine reaches a working ledger with no
hand edit only when registration is a command.

## Decisions

- 2026-10-08: `tsk nexus list` lists the attached nexus and the repos it indexes, not a
  set of nexuses. A machine still holds one nexus. Jim's message named `list` and
  `add <url>` without saying what `list` shows. Reopen if Jim meant several nexuses.
- 2026-10-08: `tsk nexus add <url>` has the semantics of `tsk config attach-nexus`.
  `config attach-nexus` stays.
- 2026-10-08: the plugin command is `tsk install-plugin claude-cli`, with the target as
  an argument so other CLIs can follow.

## Open decisions

- Whether `install-plugin` also writes `extraKnownMarketplaces` and `enabledPlugins` into
  the repo's `.claude/settings.json`. The CLI route works in a cloud session and the
  settings route does not (`docs/kb/claude-code-plugin-packaging.md`).
