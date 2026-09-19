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

A binding ties a running session, or a worktree, to a thread ID. Thread identity is
tsk's own, minted once, never a platform session ID or a path: neither is stable across
every channel.

| Channel | Binding mechanism |
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
  thread works on, not a bare mission ID.
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
| Mission briefing link | Agent (usually already known from thread state) | The briefing document the thread works on |
| Task ID | Agent (usually already known from thread state) | The task in progress when the thread paused |
| What's next | Agent | A short account of where things stand |
| Commit on `tsk/bootstrap` | Script | The commit `tsk/bootstrap` was at when the pause began, read before this entry is appended. It cannot be the commit the push creates: that commit holds this entry, so its hash is unknown here. |
| Commit on `main` | Script | The commit left on `main` at pause time |
| Timestamp | Script | When the entry was appended |
| Written by | Script | `urn:tsk:worktree:<name>` or `urn:tsk:cloudsession:<session-id>`, naming the binding that wrote this entry |

The commit and timestamp fields are deterministic: a script captures them, since the
same script already handles the git side of a pause. Only the mission link, task ID
and what's-next text involve judgement, and even the first two are usually already
known rather than freshly decided.

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

Writes one continuation state entry, via a script, `append-handover.sh`. The script
captures the commit hashes and the timestamp; the agent supplies the mission link, task
ID and what's-next text as arguments.

`/pause-thread` is invoked manually, in the supervised operating context: a human runs
it themselves before `/clear`.

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
  current session or worktree.
- Explicitly, naming an arbitrary thread ID. This is how a different actor takes over
  a thread that was not automatically bound to their session or worktree.

Multiple actors can be bound to the same thread simultaneously. Binding a session or
worktree to a thread that already has another binding elsewhere prints a warning but
proceeds.

### `/detach-thread`

Backed by `thread-detach.sh`. Removes only the current session's or worktree's own
binding: the cloud lookup entry, or the worktree marker file. The thread itself, its
continuation state, and any other actor's binding to it are untouched.

### `/stop-thread [<thread-id>]`

Backed by `thread-stop.sh`. Detaches the current binding, if it points at the target
thread, deletes the thread's directory (`index.md` and `continuation-state.jsonl`),
and purges every cloud-session lookup entry still pointing at it — a thread that no
longer exists cannot be a valid binding target for anyone. With no argument it
targets the thread currently bound; given an explicit thread ID, it targets that
thread instead, regardless of the current binding — how `/switch-thread` composes it
below.

A worktree marker in some other worktree that still names the stopped thread cannot
be reached or cleaned up from here: it is local metadata inside that other
worktree's own git directory, invisible to this script. It goes stale silently, the
same limitation worktree markers already carry generally (no separate lookup map
exists for them either — see Worktree binding above).

### `/switch-thread [<thread-id>]`

Composes the three commands above rather than duplicating their logic: detach from
the current thread, ask whether to stop (delete) it or leave it for someone to
resume later, then resume a different thread. Given an explicit thread ID, resumes
that thread directly. Given none, lists existing threads (`thread-list.sh`) and asks
which to resume.

## Resolution at session start

The `SessionStart` hook extends to:

1. Resolve the current binding (cloud session ID, or worktree marker).
2. If found, prompt the agent to run `/resume-thread <thread-id>` with the resolved ID.
3. If not found, prompt the agent to ask the human directly, as a structured question
   (`AskUserQuestion`: selectable options plus free text), not a plain message: no
   thread found, which mission do they want to work, with an initial inference offered
   as a candidate option. Once answered, the agent runs `/start-thread`.

The hook never creates a thread itself. Both branches end by naming a command and
letting the agent invoke it, not by the hook doing the loading or the asking itself.

## Binding persistence across a turn

The `SessionStart` hook above names the required action once, at the very start of a
session. Naming it once is not the same as it happening. A binding check that runs
once at session start can be skipped when conversation moves elsewhere, leaving the
session unbound for its entire duration.

**Mechanism.** A `Stop` hook, `ops/local/thread-binding-guard.sh`, runs on every turn,
not once at session start. It checks the current binding; if none is found, it blocks
the turn from completing (`{"decision": "block", "reason": "..."}`), and the agent
continues the same turn instead of handing control back. The reason text is the same
instruction the `SessionStart` hook gives on a miss (`thread_unbound_prompt_text` in
`thread-lib.sh`), so the two call sites cannot drift apart the way the push sequence
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

**Why the check does not fetch.** `thread_resolve_binding_local` (`thread-lib.sh`)
performs the same lookup as `thread_resolve_binding` but never calls
`fetch-bootstrap-ref.sh`. Refreshing over the network on every turn would be slow and
liable to fail transiently, and it is unnecessary here: a binding this session itself
wrote is already reflected in its own worktree checkout without being fetched again.

**Safety valve.** The guard does not track its own attempt count. Claude Code overrides
a `Stop` hook that blocks eight times in a row without progress, which is the actual
backstop; `AskUserQuestion` cannot fail to elicit a response, so reaching that cap would
mean the binding mechanism itself is broken, a case this script cannot repair by
trying a ninth time.

**Guarantee.** The mechanism ensures binding even when the first message is unrelated
to any mission. The `/clear` scenario is not covered by a live run: no tool call lets
an agent trigger `/clear` on itself, the same limitation M-BOOT-02-01's report
recorded. The guarantee holds by construction instead: the guard depends on no
conversational state, only on the worktree marker or the cloud lookup entry, neither
of which `/clear` changes, so its behaviour cannot differ before and after one.
