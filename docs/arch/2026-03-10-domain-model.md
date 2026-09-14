# tsk — Domain Model & State Machines

*Written: 2026-03-10. Reflects decisions made through session 3.*

---

## Core concepts

### Thread

A **thread** is a stream of work — a feature, bug fix, investigation, or initiative. Threads
are the primary unit of context management in tsk. You can have many threads but only one is
**active** at a time.

Each thread has:

- A **directory** (`tsk/threads/{id}-{slug}/`) which is its working memory — notes, plans,
  decisions, links, and eventually tasks. This is where cognitive context lives between
  sessions.
- A **state** — whether it is being worked on, paused, or blocked on something external.
- A **priority** — how urgent it is relative to other threads.

Thread ids are stable monotonic integers, zero-padded to 4 digits (`0001`, `0002`, ...).
Threads can be addressed by id (`1`, `0001`) or slug (`fix-login`).

### Task

A **task** is a discrete unit of work within a thread — something concrete that can be
ticked off. Tasks live inside a thread and are stored in `tasks.json` in the thread directory.

Tasks have their own state machine (see below). A thread is both a context container and a
task list.

### Diversion

A **diversion** is when something comes up while working on one thread that needs recording
against a different thread — without switching context. The agent language for this is:

> "Diversion: add a task to thread 0004 — follow up with Alice about the API contract"

The `--thread` flag in task commands supports this explicitly. The active thread does not
change.

---

## Thread state machine

```
                create
                  │
                  ▼
              ┌────────┐
       ┌─────▶│ PAUSED │◀──────────────────────────┐
       │      └────────┘                            │
       │        │    ▲                              │
       │       wait  resume                         │ switch-to
       │        │    │                              │ (another)
       │        ▼    │                              │
       │      ┌─────────┐                      ┌────────┐
       │      │ WAITING │◀────── wait ──────────│ ACTIVE │
       │      └─────────┘                      └────────┘
       │                                            ▲
       └──────────────── switch-to ─────────────────┘
```

### States

| State | Meaning |
|-------|---------|
| `paused` | Not currently being worked on. Ready to pick up. |
| `active` | Currently being worked on. Only one thread can be active at a time. |
| `waiting` | Blocked on an external dependency. Carries an optional `reason` string. |

### Transitions

| Command | From | To | Notes |
|---------|------|----|-------|
| `thread create` | — | `paused` | New threads always start paused |
| `thread switch-to <id>` | any | `active` | Target becomes active. Previously active thread goes to `paused`. Waiting threads that are not the target keep their `waiting` state. |
| `thread wait <id> [reason]` | `paused` or `active` | `waiting` | Reason describes what is being waited on. Error if already waiting. |
| `thread resume <id> [note]` | `waiting` | `paused` | Always returns to `paused`. Use `switch-to` to make active again. Note is recorded in the event log. Error if not waiting. |

### Design decisions

**Why does resume always go to `paused` and not restore previous state?**
Active is a deliberate choice — it means "I am working on this right now." Resume means "the
blocker is gone." Getting back to active should be a conscious act via `switch-to`. Restoring
to active automatically would be surprising and could silently displace whatever is currently
active.

**Why does `waiting` carry its reason on the state variant rather than as a separate field?**
The reason is only meaningful when the thread is in the waiting state. Co-locating the data
with the state makes the model coherent and prevents a `waiting_reason` field from existing
on every thread in every state. Serde serialises this as a nested JSON object automatically:
```json
{ "state": { "waiting": { "reason": "waiting for PR review" } } }
```
while `active` and `paused` remain flat strings.

**Why do waiting threads keep their `waiting` state when `switch-to` activates another thread?**
When you switch away, other threads go to `paused` — but a waiting thread is not just
"not being worked on", it is actively blocked on something external. Collapsing waiting to
paused on every `switch-to` would destroy that information silently. The waiting state and
its reason persist until explicitly resolved with `resume`.

---

## Task state machine

```
         create
           │
           ▼
      ┌───────────┐    start    ┌─────────────┐
      │ not-started│────────────▶│ in-progress │
      └───────────┘             └─────────────┘
           │                         │    ▲
           │ cancel              block│    │ (unblock — TBD)
           │                         ▼    │
           ▼                    ┌─────────┐
      ┌───────────┐             │ blocked │
      │ cancelled │◀────────────└─────────┘
      └───────────┘   cancel         │
           ▲                         │ complete
           │ cancel                  ▼
           └─────────────────── ┌──────────┐
                                │   done   │
                                └──────────┘
```

### States

| State | Meaning |
|-------|---------|
| `not-started` | Created but not yet started |
| `in-progress` | Actively being worked on |
| `blocked` | Cannot progress — carries a `blocked_reason` string |
| `done` | Completed |
| `cancelled` | Abandoned — reachable from any state |

### Commands

| Command | Transition | Notes |
|---------|-----------|-------|
| `task create <desc>` | → `not-started` | Requires active thread (or `--thread`) |
| `task start <id>` | `not-started` → `in-progress` | |
| `task block <id> <reason>` | `in-progress` → `blocked` | Reason is required |
| `task complete <id>` | `in-progress` → `done` | |
| `task cancel <id>` | any → `cancelled` | |
| `task update <id> [flags]` | — | Updates metadata only, not state |

### Task fields

| Field | Type | Notes |
|-------|------|-------|
| `id` | String | `TSK-{thread_id}-{seq}` e.g. `TSK-0001-0003` |
| `description` | String | Free text |
| `state` | TaskState | See above |
| `due_by` | Option\<String\> | ISO 8601. Optional. |
| `seq` | u32 | Manual ordering integer. Not displayed. Used for sort. |

### Design decisions

**Why `seq` rather than sorting by `due_by`?**
`due_by` is useful for urgency but not all tasks have due dates. Manual sequencing (`seq`)
allows the user/agent to order tasks in the way that makes sense for the work, independently
of deadlines. Tasks are displayed sorted by `seq`.

**Why are task ids internal?**
`TSK-0001-0003` is precise but verbose. Task ids are used in commands but not intended to
be user-visible in lists or TUI views (where description + state is sufficient). The id
format is designed to be machine-readable and unambiguous across threads.

**Why no task priority?**
Priority for tasks is a hard problem — most systems model it poorly. We are leaving it out
deliberately until there is a clear, well-reasoned design. `due_by` and `seq` provide
sufficient ordering for now.

**Why no task dependencies (blocking relationships)?**
Task-to-task dependencies add significant complexity to the model and UI. The `blocked`
state with a free-text reason handles the common case (noting why something is stuck)
without requiring a dependency graph. Dependencies can be added later if there is a clear
need.

---

## Priority model

Thread priority has three levels:

| Value | Name | Meaning |
|-------|------|---------|
| `INC` | Incident | Something is on fire. Drop everything. |
| `PRIO` | Priority | Important. Should be worked on soon. |
| `BG` | Background | Low urgency. Pick up when nothing more pressing. |

Priority is set at thread creation and can be changed with `thread update --priority`.

In the TUI, threads are grouped by priority into sections:

- **Active** — the single active thread
- **Priority & Incidents** — paused/waiting threads with `PRIO` or `INC` priority, sorted
  incidents first
- **Background** — paused/waiting threads with `BG` priority

---

## Storage layout

```
tsk/
  event-log/
    events.ndjson          # append-only audit trail (NDJSON, one event per line)
  threads/
    index.json             # authoritative thread state (array of Thread)
    0001-fix-login/
      index.md             # human-readable context: notes, decisions, links
      tasks.json           # task list for this thread (array of Task)
    0002-update-deps/
      index.md
      tasks.json
```

Everything under `tsk/` is committed to source control. The event log is append-only and
provides a full audit trail of all state transitions.

---

## Event types

All state transitions append an event to `events.ndjson`:

| Event | Trigger |
|-------|---------|
| `ThreadCreated` | `thread create` |
| `ThreadSwitched` | `thread switch-to` |
| `ThreadUpdated` | `thread update` |
| `ThreadWaited` | `thread wait` |
| `ThreadResumed` | `thread resume` |
| `TaskCreated` | `task create` *(planned)* |
| `TaskStarted` | `task start` *(planned)* |
| `TaskBlocked` | `task block` *(planned)* |
| `TaskCompleted` | `task complete` *(planned)* |
| `TaskCancelled` | `task cancel` *(planned)* |
| `TaskUpdated` | `task update` *(planned)* |

---

## Open questions / deferred

- **Task priority** — needs a clear design before implementing. Most systems handle this
  poorly. Current approach (seq + due_by) is sufficient for now.
- **Task relationships** — blocking dependencies between tasks. Deferred. Free-text
  `blocked_reason` covers the common case.
- **Unblock command** — currently `task block` is one-way. An `unblock` or `resume`
  equivalent for tasks will be needed. Naming TBD.
- **Configuration file** — `tsk.toml` or `.tskrc` for per-project and per-user settings.
  First planned setting: `show_status_bar = true/false`.
- **Daemon push** — TUI currently polls the daemon every 500ms. The correct long-term
  solution is daemon-initiated push over the socket (see ADR 0006).
