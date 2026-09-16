# Session creation and environments

Reference for orchestrating agent sessions. Two dimensions: the mechanisms that start a
session, and the levers that determine what it knows once started.

## Contents

- [Triggering a session](#triggering-a-session)
- [Controlling context](#controlling-context)
- [Constraints](#constraints)
- [Appendix: every way a session starts](#appendix-every-way-a-session-starts)
- [Appendix: reusing a session, and resetting one](#appendix-reusing-a-session-and-resetting-one)
- [Appendix: environments and Cowork sessions](#appendix-environments-and-cowork-sessions)

## Triggering a session

Four mechanisms can be invoked programmatically. These are the ones available to an
orchestrator. The full list of ways a session starts is in the appendix.

| Mechanism | Invoked by | Creates a session | Cloud environment |
|---|---|---|---|
| Programmatic creation | `create_session` | Yes | Selected by ID, or inherited from the caller |
| Scheduled Routine | `create_trigger` | Yes per firing, or fires into a named existing session | Selected |
| Local subagent | The Agent tool | No: runs in-process inside the calling session | None of its own |
| Headless invocation | `claude -p`, shelled from a script | Yes: a separate `claude` process | None, unless also given `--cloud` |

The other paths are not orchestrator-invocable. A person starts a session from the local
CLI, or from claude.ai/code and the apps. An external event starts one through a GitHub
Action or a PR event, or delivers into an existing session through a PR-activity
subscription. An orchestrator can cause such an event, but it does not make the call.

## Controlling context

Three mechanisms determine what a session knows. They compose. None substitutes for
another.

| Mechanism | What it carries | Set by | Applies to |
|---|---|---|---|
| Initial prompt | The instruction the session starts from | The caller, at invocation | Every path except the two driven by an external event |
| Repository contents | `CLAUDE.md` and `.claude/`, chiefly the `SessionStart` hooks | What is committed, plus which repository and revision is checked out | Every path. The only mechanism that works without an authored prompt, because it reads out of the repository rather than out of the invocation |
| Environment | Container, repository and network access, environment variables | A person, in advance, through the web or the Desktop app | Cloud sessions only |

Permission mode is a fourth lever and is not a context mechanism. It constrains what a
session may do without approval, not what it knows. Values: `plan`, `default`,
`acceptEdits`, `bypassPermissions`, `dontAsk`.

Which levers each triggering mechanism exposes:

| | Initial prompt | Repository and revision | Environment |
|---|---|---|---|
| `create_session` | Set | Set via `source_url`, `source_revision` | Selected by ID, or inherited |
| `create_trigger` | Set | Set | Selected |
| Agent tool subagent | Set | Inherited from the calling session | None of its own |
| `claude -p` | Set | Whatever is checked out where the script runs | None, unless `--cloud` |

## Constraints

**An environment cannot be created programmatically.** Environments are created by a
person, through the web or the Desktop app. Nothing available to a running session
creates one. An orchestrator selects an existing environment by ID, or inherits the
calling session's.

**A session cannot set a durable environment variable for itself.** Shell state does not
carry between tool calls; only the working directory does. An `export` in one call is
gone by the next. Variables that persist across a whole session, or across every session
in an environment, are set in the environment's own configuration.

**A subagent has no environment of its own.** It runs inside the calling session's, and
inherits that session's loaded `CLAUDE.md` rather than loading its own. Whether
`SessionStart` hooks fire again for a subagent is unverified.

## Appendix: every way a session starts

1. Local interactive CLI. Run `claude` in the repository. The person types the first
   message.
2. claude.ai/code, and the Desktop and mobile apps. Same shape: a session against a
   repository, first message typed by the person.
3. A GitHub Action or PR event triggers a cloud session, for example a PR opening or a CI
   failure. The initial context comes from the event, not from an authored prompt.
4. `create_session`. One session spawns another, passing an explicit `prompt`, and
   optionally a source repository and revision.
5. A scheduled Routine, `create_trigger`. Fires on a cron schedule or once, either into an
   existing persistent session or spawning a fresh one per firing
   (`create_new_session_on_fire`). Carries a `prompt`.
6. A PR-activity subscription. No new session: webhook events keep arriving in an
   existing conversation.
7. A subagent, spawned in-process by the Agent tool.
8. `claude -p`, shelled from an external script. A separate `claude` process, so it loads
   `CLAUDE.md` and fires hooks as any session does, but it runs wherever the script runs.

Paths 3 and 6 are the two where nobody authors the initial prompt. Every other path
allows one.

A `SessionStart` hook reading state out of the repository is the one mechanism common to
paths 1 to 6 and path 8, because it does not depend on the prompt. Path 7 is unverified.
tsk uses this for mission state: `ops/local/claude-session-start.sh`, and the "Current
mission" line in `index.md` on `tsk/bootstrap`. Its limit: a single pointer does not say
which mission a session should pick up when more than one is unblocked.

## Appendix: reusing a session, and resetting one

A session can be addressed and given new work after it was created. Three mechanisms
exist:

- `create_trigger` takes `persistent_session_id`, which fires into a named existing
  session rather than spawning one.
- `claude -p "message" --cloud <session-id>` queues a message into an existing session,
  from any machine logged in to the same account.
- A session reports `cross_session_inbound` in `get_session` when it can receive messages
  from another session.

So a pool of long-lived workers, each addressable by session ID and handed a mission when
one is ready, is mechanically supported.

What is not supported is resetting one. `/clear` is unavailable in cloud sessions; the
documentation directs the reader to start a new session instead. `/compact` and
`/context` do work, but compaction summarises the conversation rather than discarding it,
so a worker carries its history into the next mission it is given. Context accumulates
across every mission that worker takes.

Session count itself is cheap. There is no separate compute charge for a cloud VM, and
rate limits are shared across the account and consumed by parallel work rather than by
the number of sessions in existence. An idle session has its VM reclaimed, and reopening
it provisions a fresh VM with the conversation history restored, so a pool of workers
buys stable addresses rather than warm containers. `archive_session` exists for managing
the resulting list.

The trade, then, is between a new session per mission, which starts with clean context
and leaves a list to archive, and a long-lived worker, which keeps a stable identity and
continuity at the cost of carrying every previous mission with it.

## Appendix: environments and Cowork sessions

An environment carries a tagged ID: `env_...`, or `ccpool_...` for a self-hosted pool. It
bundles a container, repository and network access, and configuration. If
`environment_id` is omitted on `create_session`, the new session inherits the calling
session's.

`get_session`, called with no `session_id`, describes the calling session. Its fields
include `session_id`, `environment_id`, `environment_kind` (for example
`anthropic_cloud`), and `origin`, the surface that started it. `list_environments`
returns the account's environments, each carrying a `kind`.

A Cowork session is a distinct kind. Per `create_session`'s tool description, one is
spawned only when `environment_id` resolves to the `remote_cowork` environment, whether
explicitly or by inheritance. That environment kind assembles the account's enabled
skills, plugins and a Cowork system prompt server-side, and ignores several of
`create_session`'s fields: `source_url`, `extra_allowed_tools`, `append_system_prompt`
and `environment_variables`.

So one session can create another cloud session by calling `create_session`. Whether the
result is an ordinary cloud session, visible in the normal sessions list on
claude.ai/code and the apps, or a Cowork session, depends entirely on which environment
it resolves to. When the calling session's environment is not `remote_cowork` and no
`environment_id` is given, the new session inherits that non-Cowork environment and is an
ordinary cloud session.
