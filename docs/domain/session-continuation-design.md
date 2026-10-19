# Session continuation design

Reference for the mechanism that lets a session pick up a thread it or another actor
worked on, across a session boundary. Covers the supervised interactive operating
context only: a human runs the session directly and can act on a prompt immediately,
as opposed to an unattended or autonomous run (see
`docs/kb/agent-context-self-regulation-and-unattended-handoff.md` for the full
distinction).

Depends on: `docs/domain/ubiquitous-language.md` (Actor, Thread, Thread continuation),
`docs/kb/session-creation-and-environments.md`,
`docs/kb/agent-context-self-regulation-and-unattended-handoff.md`.

## Contents

- [Binding a session to a thread](#binding-a-session-to-a-thread)
- [Thread identity and the threads directory](#thread-identity-and-the-threads-directory)
- [Continuation state entries](#continuation-state-entries)
- [The commands](#the-commands)
- [Resolution at session start](#resolution-at-session-start)
- [Binding persistence across a turn](#binding-persistence-across-a-turn)

## Binding a session to a thread

A binding ties a running session, or a code worktree, to a thread ID. Thread identity is
tsk's own, minted once, never a platform session ID or a path: neither is stable across
every channel.

| Channel | Binding mechanism |
|---|---|
| Cloud session | A row of the `bindings` table, keyed on `CLAUDE_CODE_REMOTE_SESSION_ID` |
| Code worktree | A row of the same table, keyed on the clone ID and the worktree name |

Every binding is one row of the ledger's `bindings` table: the actor key, its kind
(`cloud` or `worktree`), the thread ID and the registration time. An actor has one row, so
binding it to another thread replaces the row. A thread can have any number of rows. The
table has no foreign key to `threads`: `tsk thread stop` deletes the thread and every row
that names it, of either kind.

`tsk thread list` shows the actors of each thread in `actors`, an array of
`{"actor", "kind"}`. `tsk thread binding` prints `cloud:<thread-id>` or
`worktree:<thread-id>` for the row of the current actor.

### Cloud session binding

The environment variable `CLAUDE_CODE_REMOTE_SESSION_ID` (`cse_...`) is the platform
session ID, readable directly with no MCP call. Swapping the `cse_` prefix for
`session_` matches `get_session`'s `id` field exactly.

A second variable, `CLAUDE_CODE_SESSION_ID`, is a different UUID: a lower-level
container or instance identifier, not the platform session ID. A binding must use
`CLAUDE_CODE_REMOTE_SESSION_ID`, never the other one.

The actor key of a cloud session is the session ID. A shared, durable table is needed
here because a cloud session's identity is a string with no persistent local file store
attached to it that survives the session's container being reclaimed and reopened.

### Code worktree binding

`git rev-parse --show-toplevel` is not a stable identifier: it returns the working
directory, which changes the moment a code worktree is renamed or moved.

`git rev-parse --git-dir` and `--git-dir --git-common-dir` are equal in the main
worktree (`.git`) and diverge in a linked worktree: `--git-dir` returns
`<main-repo>/.git/worktrees/<name>`, `--git-common-dir` returns `<main-repo>/.git`.
`git-dir != git-common-dir` is the test for "this is a linked worktree."

`<name>`, the basename of `--git-dir`'s output, is set once at `git worktree add` time
and survives a rename or a `git worktree move`. It is not permanently unique, though:
removing a code worktree frees its name, and a later, unrelated code worktree created with the
same directory basename is assigned the same name. `git worktree list --porcelain`
does not expose `<name>` directly; it has to be derived from `--git-dir`.

The name is unique only within one clone, and the ledger is shared by every clone. So the
actor key of a code worktree is `<clone-id>/<worktree-name>`. The clone ID is the one in
`.git/tsk-clone-id`, minted once per clone from the clone directory's name and a random
suffix, and kept when the directory moves. The worktree name is `main` for the main
worktree and the basename of `--git-dir` for a linked one. Two clones never share a key. A
recycled worktree name in one clone reuses the key, and so the old row: the new
worktree's first bind replaces it.

Decided 2026-10-16, built by M-TSK-SCHEMA T-11. Before that, the binding of a code
worktree was a marker file, `$(git rev-parse --git-dir)/tsk-thread-id`, with the thread ID in
it. The first command that resolves a worktree binding, finds the file and finds no row
for the actor key writes the row from the file and removes the file. Nothing writes the file
again. A bind or a detach of a worktree is a ledger write, and the command pushes it with any pending local records and documents, as a cloud session's does. A failed push does not fail a worktree command: the binding is valid in the local store, so the command prints `note: the binding is saved locally; run tsk ledger push when origin is reachable` and exits 0. A worktree command also goes on when its first refresh fails and a local store exists. A cloud command still fails on either.

The Stop hook check and `tsk thread binding --no-fetch` read the local ledger store only
and never fetch, so they work offline. They read one table with one query, and they read
a marker file that is still there without migrating it.

`tsk thread skip` stays a file in the git directory, `tsk-thread-skip`. It is local to
the clone and never in the ledger.

## Thread identity and the threads directory

A thread ID is a bare slug: 8 characters, lowercase base36, checked against existing
`threads/` entries for collision at mint time. Wrapped in a URN only if it ever needs
to leave the `threads/` namespace; the directory itself is the namespace, so a bare
slug is enough inside it.

Each thread has a directory, `threads/<slug>/`, holding the following. In the Dolt ledger
the directory is a row of the `threads` table: `mission_id`, and `briefing_path` only for a
mission with no record.

- `index.md` (file ledger only): a stable pointer. It names the mission by ID
  (`Mission: <id>`). A thread for a mission with no record also links the mission
  briefing document. A thread made before the ID was stored has the link only, and the
  reader derives the ID from the link's file name. The Dolt ledger holds the mission ID
  in the `threads` row.
- `continuation-state.jsonl`: an append only store of continuation contexts that were
  passed between thread pause and resume. Named "thread continuation" in
  `docs/domain/ubiquitous-language.md`, qualified because the record already lives
  inside the thread's own directory. It is a state store that keeps its history, not a
  log: what it holds is the state a resume reads, and earlier entries are kept rather
  than being the point of the file.

## Continuation state entries

One JSON object per line, appended, never rewritten. Fields:

| Field | Written by | Description |
|---|---|---|
| Mission ID | Binary | The mission the thread works, from the thread |
| Mission briefing link | Binary | The briefing document the thread works on. Written only for a thread whose mission has no record |
| Task ID | Agent (usually already known from thread state) | The task in progress when the thread paused. When the mission has a record, the binary checks it against the mission's tasks |
| What's next | Agent | A short account of where things stand |
| Git state | Binary | A nested `git` object. `git.ledger.commit` is the commit the ledger branch was at when the pause began, read before this entry is appended. It cannot be the commit the push creates: that commit holds this entry, so its hash is unknown here. `git.code.ref` is the full ref of the branch the code worktree is on at pause time. `git.code.commit` is the commit at its `HEAD`. |
| Timestamp | Script | When the entry was appended |
| Written by | Script | The `bindings` actor key of the actor that wrote this entry: the session ID in a cloud session, otherwise `<clone-id>/<worktree-name>` (`main` for a main worktree). An entry written by 0.16.0 or earlier holds `urn:tsk:worktree:<name>` or `urn:tsk:cloudsession:<session-id>`, which `thread resume` treats as an unknown writer |

The commit and timestamp fields are deterministic: a script captures them, since the
same script already handles the git side of a pause. The mission comes from the thread.
Only the task ID and what's-next text involve judgement, and the task ID is usually
already known rather than freshly decided.

Resuming a thread reads the latest entry by default, selected by timestamp. Earlier
entries stay in the store and can be read directly, for example to notice a task
stalling across several pauses. A later entry supersedes an earlier one by being later;
nothing needs to say so. The written-by field on every entry also means the store
answers "which actors touched this thread": no separate registry is needed.

## The commands

### `/start-thread`

Backed by a deterministic script. Invoked explicitly, or via a natural-language
request naming a mission.

1. Resolve the current binding: check `CLAUDE_CODE_REMOTE_SESSION_ID` first; if absent,
   use the actor key of the code worktree. Both are rows of `bindings`.
2. If a thread already exists for that binding, this is a resume, not a start (see
   `/resume-thread`).
3. If none exists, mint a new thread ID, create the thread with its mission (or with no
   mission, when no mission argument is given), and write the binding (the `bindings` row of
   the session or the code worktree). Starting clears a `tsk thread skip`.
4. The mission argument is resolved and validated twice: the agent resolves a
   natural-language mission reference to an actual mission ID before running the
   command, and `tsk thread start <mission-id>` validates that ID independently rather
   than trusting the agent's resolution alone. The mission must have a record, or, in a
   ledger not yet moved to records, a briefing document. The thread stores the briefing
   path only in the second case.
5. `tsk thread start` with no mission argument mints a thread with no mission. Use it for
   continuity without a mission. The thread has no tasks: tasks belong to missions, and a
   thread that needs ad hoc tasks takes an administrative mission such as `M-ADMIN-02`.
   `tsk thread set mission <mission-id> [<thread-id>]` attaches a mission later or changes
   it, and `tsk thread set mission --none [<thread-id>]` removes it. Entries already
   written keep the mission they were written with. The session start context for a bound
   thread with no mission says so, gives the thread's latest what's next (or `none yet`),
   and names `tsk thread set mission <mission-id>`.

Everything in this path is scriptable. No agent judgement is needed once the mission
argument is resolved.

### `/pause-thread`

Writes one continuation state entry, through `tsk thread pause`. The command captures
the commit hashes, the branch and the timestamp, and takes the mission from the thread;
the agent supplies the task ID and what's-next text as arguments:
`tsk thread pause <thread-id> <task-id> <whats-next>`. The task ID can be empty. When the
mission has a record, a task ID that is not one of its tasks stops the pause. A thread with
no mission has no tasks: its task ID must be empty, and any other value stops the pause.
The entry records the mission current at the pause, `null` for a thread with no mission.

The pause rule: `HEAD` of the code worktree must be reachable from some branch on origin,
not only from origin's default branch, because work happens on feature branches and in
other code worktrees. A detached `HEAD` stops the pause, because there is no branch to
record in `git.code.ref`. The command takes its context from the directory it runs in,
so it needs no record of branch switches: each entry is a snapshot at pause.

A session that moves into another code worktree resolves the binding of that code
worktree, not the binding it started with. No mechanism carries a binding across code
worktrees yet.

`/pause-thread` is invoked manually, in the supervised operating context: a human runs
it themselves before `/clear`.

### Working without a thread

`tsk thread skip` records, in a file in the clone's git directory, that this session or
code worktree works without a thread. The Stop hook check passes in silence while the file
exists. The record is local and is never in the ledger. `tsk thread start` and
`tsk thread resume` remove it. The Stop hook check also passes when the ledger holds no
missions, because there is nothing to bind to.

### `/resume-thread <thread-id>`

The load counterpart to `/pause-thread`'s save. Loads the latest continuation state entry for
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
  current session or code worktree.
- Explicitly, naming an arbitrary thread ID. This is how a different actor takes over
  a thread that was not automatically bound to their session or code worktree.

Multiple actors can be bound to the same thread simultaneously. Binding a session or
code worktree to a thread that already has another binding elsewhere prints a warning but
proceeds.

### `/detach-thread`

Backed by `tsk thread detach`. Removes only the current session's or code worktree's own
binding: its `bindings` row. The thread itself, its
continuation state, and any other actor's binding to it are untouched.

### `/stop-thread [<thread-id>]`

Backed by `tsk thread stop`. Detaches the current binding, if it points at the target
thread, deletes the thread's directory (`index.md` and `continuation-state.jsonl`),
and deletes every `bindings` row still pointing at it, of either kind. A thread that no
longer exists cannot be a valid binding target for anyone. With no argument it
targets the thread currently bound; given an explicit thread ID, it targets that
thread instead, regardless of the current binding — how `/switch-thread` composes it
below.

A marker file from before the `bindings` table, in some other code worktree, that still
names the stopped thread cannot be reached or cleaned up from here: it is local metadata
inside that other code worktree's own git directory, invisible to this script. It goes
stale silently, and its first resolve there writes a row for a thread that no longer
exists. Every other binding is a row, so stopping the thread clears it.

### `/switch-thread [<thread-id>]`

Composes the three commands above rather than duplicating their logic: detach from
the current thread, ask whether to stop (delete) it or leave it for someone to
resume later, then resume a different thread. Given an explicit thread ID, resumes
that thread directly. Given none, lists existing threads (`tsk thread list`) and asks
which to resume.

## Resolution at session start

The `SessionStart` hook runs `tsk thread session-start`, which does the tsk-specific
work and prints the context message. It extends the ledger fetch with:

1. Resolve the current binding (the `bindings` row of the cloud session ID, or of the code worktree).
2. If found, prompt the agent to run `/tsk:resume-thread <thread-id>` with the resolved ID.
3. If not found, prompt the agent to ask the human directly, as a structured question
   (`AskUserQuestion`: selectable options plus free text), not a plain message: no
   thread found, which mission do they want to work, with an initial inference offered
   as a candidate option. The candidates come from `tsk mission list`. Once answered, the
   agent runs `/tsk:start-thread`. A person who wants no thread gets `tsk thread skip`.
4. If the ledger holds mission records and the session is unbound, the context also says
   `Run `tsk mission list` to see the missions.` It does not name `index.md`, which the
   record model retires. A bound thread gets its mission and open tasks instead.
5. If the plugin that loaded the hook is older than the plugin version the binary
   expects, the context says so and names the update command. The hook passes its own
   version with `--plugin-version`.

The hook never creates a thread itself. Both branches end by naming a command and
letting the agent invoke it, not by the hook doing the loading or the asking itself.

## Binding persistence across a turn

The `SessionStart` hook above names the required action once, at the very start of a
session. Naming it once is not the same as it happening. A binding check that runs
once at session start can be skipped when conversation moves elsewhere, leaving the
session unbound for its entire duration.

**Mechanism.** A `Stop` hook, which calls `tsk thread guard`, runs on every turn,
not once at session start. It checks the current binding; if none is found, it blocks
the turn from completing (`{"decision": "block", "reason": "..."}`), and the agent
continues the same turn instead of handing control back. The reason text is the same
instruction `tsk thread session-start` gives on a miss (one constant in the binary), so the two call sites cannot drift apart the way the push sequence
once did (see `CLAUDE.md`). This holds even when the human's first message after a
`SessionStart` firing, or after a `/clear`, is unrelated to any mission: the agent may
answer it, but the turn cannot end until a thread is bound, so the binding instruction
cannot be silently dropped as a casualty of that answer.

**Why a `Stop` hook and not `/goal`.** `/goal`'s evaluator is a fast model judging
whether a condition holds from the transcript alone; whether a thread is bound is a
plain boolean fact, checkable by a deterministic script with no judgement involved. Using
`/goal` for it would also occupy the one-goal-per-session slot a mission's own
verification work might later want, and `/goal` is documented as built for the
unsupervised context, not this design's supervised-interactive scope.

**Why the check does not fetch.** `tsk thread guard` resolves the binding
from the existing ledger store, with one query, and never runs `tsk ledger fetch`.
Refreshing over the network on every turn would be slow and liable to fail transiently,
and it is unnecessary here: a binding this session itself wrote is already in its own
ledger store without being fetched again.

**Safety valve.** The guard does not track its own attempt count. Claude Code overrides
a `Stop` hook that blocks eight times in a row without progress, which is the actual
backstop; `AskUserQuestion` cannot fail to elicit a response, so reaching that cap would
mean the binding mechanism itself is broken, a case this script cannot repair by
trying a ninth time.

**Guarantee.** The mechanism ensures binding even when the first message is unrelated
to any mission. The `/clear` scenario is not covered by a live run: no tool call lets
an agent trigger `/clear` on itself, the same limitation M-BOOT-02-01's report
recorded. The guarantee holds by construction instead: the guard depends on no
conversational state, only on the actor's `bindings` row, which `/clear` does not change, so its behaviour cannot differ before and after one.
