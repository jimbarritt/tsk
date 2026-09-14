# State models

## Task state model

```
         create
           │
           ▼
      ┌───────────┐    start    ┌─────────────┐
      │ not-started│────────────▶│ in-progress │
      └───────────┘             └─────────────┘
           │                         │    ▲
           │ cancel              block│    │ (unblock?)
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

Commands: `task create`, `task start`, `task block`, `task complete`, `task cancel`, `task update`, `task list`.

Tasks are stored in `~/.tsk/threads/{id}-{slug}/tasks.json`, one file per thread.

Task fields: `id` (`TSK-{thread-id}-{seq}` e.g. `TSK-0001-0001`), `description`, `state`, `due_by` (ISO 8601, optional), `seq` (integer, for manual ordering).

All task commands default to the currently active thread. Use `--thread <id>` to target a different thread explicitly.

### Diversions

A **diversion** is when something comes up while you work on one thread that needs recording against a different thread, without switching context. The agent language for this is:

> "Diversion: add a task to thread 0004 — follow up with Alice about the API contract"

The `--thread` flag makes this explicit in the CLI:
```
tsk task create "follow up with Alice about the API contract" --thread 0004
```

The active thread does not change. You record the thought and get back to what you did before.

## Thread state model

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

- `create` → always starts **paused**
- `switch-to` → target becomes **active**; previously active thread becomes **paused**
- `wait` → marks a thread **waiting** (blocked on external dependency); works from active or paused
- `resume` → returns a waiting thread to **paused**; use `switch-to` to make it active again

## How it works

`tskd` is a headless daemon that owns all state. One `tskd` instance runs per user, not per-project. `tsk` is a thin client: in CLI mode it sends a JSON-RPC request over the Unix socket at `~/.tsk/tskd.sock` and exits; in TUI mode it watches `~/.tsk/threads/index.json` for changes and re-renders instantly. Multiple clients (CLI, TUI, agents) can talk to the daemon concurrently. See `doc/arch/` and `doc/adr/` for the full architecture.
