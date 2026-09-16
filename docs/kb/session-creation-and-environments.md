# Session creation and environments

Gathered while working M-BOOT-02 (harness), investigating how a mission briefing gets
associated with a session. Facts collected 2026-09-16, from direct inspection of the
running session and the platform's own tools. Not yet a design: see
`missions/M-BOOT-02/M-BOOT-02-briefing.md` on `tsk/bootstrap` for the open decision this
feeds.

## Ways a new session gets created

1. Local interactive CLI. Run `claude` in the repo. First message is typed by the person.
2. claude.ai/code (web, desktop, mobile app). Same shape: a session against a repo, first
   message typed by the person.
3. A GitHub Action or PR event triggers a cloud session (for example: PR opened, CI
   failure). Initial context comes from the event, not from an authored prompt.
4. Programmatic session creation. One session calls `create_session` and spawns another,
   passing an explicit `prompt` string, and optionally a source repo and branch.
5. A scheduled Routine (`create_trigger`). Fires on a cron schedule or once, either into
   an existing persistent session or spawning a fresh session on each firing
   (`create_new_session_on_fire`). Also carries a `prompt` string.
6. An existing session kept alive on a PR-activity subscription. No new session created;
   it keeps receiving webhook events into the same conversation.

Only paths 1, 2, 4, and 5 let anyone author the initial prompt freely. Paths 3 and 6 do
not. The one thing the harness can rely on across all six paths is a `SessionStart` hook
reading state out of the repository itself, since that does not depend on the prompt at
all. This is what `tsk`'s own `SessionStart` hook already does for mission state: see
`ops/local/claude-session-start.sh` and the `index.md` "Current mission" line on
`tsk/bootstrap`. The gap: that is a single pointer, so it does not say which mission a
session should pick up when more than one is unblocked at once.

## Environment variables do not persist within a session

Tested directly: `export TSK_TEST_VAR=hello` in one Bash tool call, then read back empty
in the next Bash tool call. Shell state does not persist between tool calls in this
harness; only the working directory does. A session cannot set an environment variable
for itself that lasts beyond a single command.

Environment variables that persist across a whole session, or across every session in an
environment, are a different mechanism entirely: they are set in the environment's own
configuration (see `mcp__Claude_Code_Remote__list_environments` /
`create_session`'s `environment_id`), not by a running session acting on itself.

## Cloud sessions, "environments", and Cowork sessions

A "cloud session" (what the CLI calls a Claude Code Remote session) runs inside an
"environment": a tagged ID (`env_...`, or `ccpool_...` for a self-hosted pool) that
bundles a container, repo/network access, and configuration. `create_session` creates one
of these. If `environment_id` is omitted, the new session inherits the calling session's
environment.

This session's own facts, read directly from `get_session`:

- `session_id`: `session_01WePrEonPkCV4kfJPK9D4Sy`
- `environment_id`: `env_0173H2wsxugkZUm5Whrkmtv9`, named "Default", `environment_kind`:
  `anthropic_cloud`
- `origin`: `ios` — this exact session was opened from the iOS app

`list_environments` for this account currently returns two environments, "Test Cargo" and
"Default", both `kind: anthropic_cloud`. Neither is a Cowork environment.

A Cowork session is a distinct thing: per `create_session`'s own tool description, one is
spawned only "when [`environment_id`] resolves to the `remote_cowork` environment
(explicitly or via inheritance)" — a different environment kind that assembles the
account's enabled skills, plugins, and a Cowork-specific system prompt server-side, and
that ignores several of `create_session`'s fields (`source_url`, `extra_allowed_tools`,
`append_system_prompt`, `environment_variables`).

So: yes, one Claude session can create another cloud session, by calling
`create_session`. Whether the result is an ordinary Claude Code Remote session (what this
session itself is, visible in the same sessions list this session appears in, in
claude.ai/code and in the iOS app) or a Cowork session depends entirely on which
environment it resolves to. Today, this account has no `remote_cowork`-kind environment
in its `list_environments` output, so a `create_session` call made from here, with no
`environment_id` given, inherits this session's own `anthropic_cloud` environment and
produces another ordinary cloud session, not a Cowork session.
