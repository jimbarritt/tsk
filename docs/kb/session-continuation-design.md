# Session continuation design

Reference for the mechanism that lets a session pick up a thread it or another actor
was working on, across a session boundary. Covers the supervised interactive operating
context only (see `docs/kb/agent-context-self-regulation-and-unattended-handoff.md` for
what that means); an automated trigger for pausing is future work, not covered here.

Depends on: `docs/domain/ubiquitous-language.md` (Actor, Thread, Thread continuation),
`docs/kb/session-creation-and-environments.md`,
`docs/kb/agent-context-self-regulation-and-unattended-handoff.md`.

## Contents

- [Binding a session to a thread](#binding-a-session-to-a-thread)
- [Thread identity and the threads directory](#thread-identity-and-the-threads-directory)
- [Continuation events](#continuation-events)
- [The three commands](#the-three-commands)
- [Resolution at session start](#resolution-at-session-start)
- [Deferred](#deferred)

## Binding a session to a thread

A binding ties a running session, or a worktree, to a thread ID. Thread identity is
tsk's own, minted once, never a platform session ID or a path: neither is stable across
every surface.

| Surface | Binding mechanism |
|---|---|
| Cloud session | `CLAUDE_CODE_REMOTE_SESSION_ID` looked up in `threads/lookup-by-cloud-session.json` on `tsk/bootstrap` |
| CLI worktree | A thread-ID marker file written inside the worktree's own git metadata |

### Cloud session binding

The environment variable `CLAUDE_CODE_REMOTE_SESSION_ID` (`cse_...`) is the platform
session ID, readable directly with no MCP call. Swapping the `cse_` prefix for
`session_` matches `get_session`'s `id` field exactly.

A second variable, `CLAUDE_CODE_SESSION_ID`, is a different UUID: a lower-level
container or instance identifier, not the platform session ID. A binding must use
`CLAUDE_CODE_REMOTE_SESSION_ID`, never the other one.

The lookup lives at `threads/lookup-by-cloud-session.json` on `tsk/bootstrap`, keyed on
the session ID, each entry:

```json
{ "thread_id": "<slug>", "registered_at": "<timestamp>" }
```

A shared, durable map is needed here because a cloud session's identity is a string
with no persistent local file store attached to it that survives the session's
container being reclaimed and reopened.

### CLI worktree binding

`git rev-parse --show-toplevel` is not a stable identifier: it returns the working
directory, which changes the moment a worktree is renamed or moved.

`git rev-parse --git-dir` and `--git-dir --git-common-dir` are equal in the main
worktree (`.git`) and diverge in a linked worktree: `--git-dir` returns
`<main-repo>/.git/worktrees/<name>`, `--git-common-dir` returns `<main-repo>/.git`.
`git-dir != git-common-dir` is the test for "this is a linked worktree."

`<name>`, the basename of `--git-dir`'s output, is set once at `git worktree add` time
and survives a rename or a `git worktree move`. It is not permanently unique, though:
removing a worktree frees its name, and a later, unrelated worktree created with the
same directory basename is assigned the same name. `git worktree list --porcelain`
does not expose `<name>` directly; it has to be derived from `--git-dir`.

Because of that recycling risk, the binding does not key on `<name>`. Instead, tsk
writes its own thread ID directly into a marker file:

```
$(git rev-parse --git-dir)/tsk-thread-id
```

One uniform rule, main worktree included: `--git-dir` already resolves to a distinct
path per worktree (`.git/worktrees/<name>` for a linked one, `.git` itself for the
main one), so the marker never collides even though the main worktree's `.git` is
otherwise shared object and ref storage that every linked worktree also reads via
`--git-common-dir`.

No separate lookup map exists for worktrees. The marker file is the whole binding: it
is colocated with the worktree, has no name to key on, and so has no recycling problem
to guard against. Two worktrees can each hold a marker pointing at the same thread with
no coordination needed, since each marker is its own file.

## Thread identity and the threads directory

A thread ID is a bare slug: 8 characters, lowercase base36, checked against existing
`threads/` entries for collision at mint time. Wrapped in a URN only if it ever needs
to leave the `threads/` namespace; the directory itself is the namespace, so a bare
slug is enough inside it.

Each thread has a directory, `threads/<slug>/`, holding:

- `index.md`: a stable pointer. At minimum, a link to the mission briefing document the
  thread is working, not a bare mission ID. Further contents are undecided; see
  Deferred.
- `continuation-state.jsonl`: the append-only log of continuation events. Named "thread
  continuation" in `docs/domain/ubiquitous-language.md`, qualified because the record
  already lives inside the thread's own directory.

## Continuation events

One JSON object per line, appended, never rewritten. Fields:

| Field | Written by | Description |
|---|---|---|
| Mission briefing link | Agent (usually already known from thread state) | The briefing document the thread is working |
| Task ID | Agent (usually already known from thread state) | The task in progress when the thread paused |
| What's next | Agent | A short account of where things stand |
| Commit on `tsk/bootstrap` | Script | The commit the push script left the branch at |
| Commit on `main` | Script | The commit left on `main` at pause time |
| Timestamp | Script | When the event was appended |
| Written by | Script | `urn:tsk:worktree:<name>` or `urn:tsk:cloudsession:<session-id>`, naming the binding that wrote this event |

The commit and timestamp fields are deterministic: a script captures them, since the
same script already handles the git side of a pause. Only the mission link, task ID
and what's-next text involve judgement, and even the first two are usually already
known rather than freshly decided.

Resuming a thread reads only the latest event by default. Earlier events stay in the
log and can be read directly, for example to notice a task stalling across several
pauses. The written-by field on every event also means the log doubles as the answer
to "which actors have touched this thread": no separate registry is needed.

## The three commands

### `/start-thread`

Backed by a deterministic script. Invoked explicitly, or via a natural-language
request naming a mission.

1. Resolve the current binding: check `CLAUDE_CODE_REMOTE_SESSION_ID` first; if absent,
   resolve the worktree marker.
2. If a thread already exists for that binding, this is a resume, not a start (see
   `/resume-thread`).
3. If none exists, mint a new thread ID, scaffold `threads/<slug>/`, and write the
   binding (the cloud lookup entry, or the worktree marker file).
4. The mission argument is resolved and validated twice: the agent resolves a
   natural-language mission reference to an actual mission ID and briefing link before
   running the script, and the script validates that reference independently rather
   than trusting the agent's resolution alone.

Everything in this path is scriptable. No agent judgement is needed once the mission
argument is resolved.

### `/pause-thread`

Writes one continuation event, via a script, `append-handover.sh`. The script captures
the commit hashes and the timestamp; the agent supplies the mission link, task ID and
what's-next text as arguments.

For now, `/pause-thread` is invoked manually, in the supervised operating context: a
human runs it themselves before `/clear`. The action set is expected to stay the same
once an automated trigger, likely driven by the goal verifier (`/goal`), is designed
for the unsupervised case; only the trigger changes.

### `/resume-thread <thread-id>`

The load counterpart to `/pause-thread`'s save. Loads the latest continuation event for
the given thread ID and presents it to the agent as a prompt. The agent replies with a
fixed-shape summary, then asks whether to continue with the thread or do something
else:

```
thread id: <id>
we are working on mission <mission title>
this is where we are at: <summary of the handover note>
```

Invoked two ways:

- Automatically, when the `SessionStart` hook finds an existing binding for the
  current session or worktree.
- Explicitly, naming an arbitrary thread ID. This is how a different actor takes over
  a thread that was not automatically bound to their session or worktree.

Take-over is additive, not exclusive: nothing prevents two actors from being bound to
the same thread. Binding a session or worktree to a thread that already has another
binding elsewhere prints a warning but proceeds. What running the same thread from two
actors at once actually does in practice is not yet explored.

## Resolution at session start

The `SessionStart` hook extends to:

1. Resolve the current binding (cloud session ID, or worktree marker).
2. If found, prompt the agent to run `/resume-thread <thread-id>` with the resolved ID.
3. If not found, prompt the agent to ask the human directly: no thread found, do they
   want to start one, and if so, for what mission. Once answered, the agent runs
   `/start-thread`.

The hook never creates a thread itself. Both branches end by naming a command and
letting the agent invoke it, not by the hook doing the loading or the asking itself.

## Deferred

Points raised during this design and explicitly set aside, not yet decided:

- The relationship between a thread and the mission's Plan.
- The thread state format proper (which tasks are done, which is in progress, as
  distinct from a single what's-next line in a continuation event).
- What running the same thread from two actors at once should actually do, beyond
  printing a warning.
- Whether "Continuation" as a term on its own, distinct from "Thread continuation",
  belongs in the ubiquitous language.
- Any content for `index.md` beyond the mission briefing link.
