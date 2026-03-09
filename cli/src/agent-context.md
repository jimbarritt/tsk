# tsk — agent context

tsk is a tool for tracking parallel work threads. It is designed to work alongside both
autonomous agents and humans, keeping cognitive context clear across context switches.

## Core concepts

A **thread** is a stream of work — a feature, bug fix, investigation, or task. You can have
many threads but only one is **active** at a time. When you switch to a thread, all others
are paused.

Each thread has a **directory** (`tsk/threads/{id}-{slug}/`) for storing context: notes,
plans, decisions, links — anything needed to resume this thread quickly without losing
cognitive continuity. This directory is the thread's working memory.

Thread ids are stable zero-padded integers (0001, 0002, ...). Reference a thread by id
(`1` or `0001`) or slug (`fix-login`).

### Priorities

- `INC` — incident: something is on fire, drop everything
- `PRIO` — priority: important, should be worked on soon
- `BG` — background: low urgency, pick up when nothing more pressing

### Commands

```
tsk thread create <slug> <priority> <description>   create a new thread (starts paused)
tsk thread switch-to <id-or-slug>                   activate a thread (pauses all others)
tsk thread list                                     list all threads as JSON
tsk context                                         print this context
tsk                                                 launch the live TUI (press q to quit)
```

### Response formats

**`tsk thread create`** returns the created thread:
```json
{
  "id": 1,
  "slug": "fix-login",
  "priority": "PRIO",
  "state": "paused",
  "description": "Fix the login bug",
  "dir": "/your/project/tsk/threads/0001-fix-login"
}
```

The `dir` field is the absolute path to the thread's context directory. This is where you
should read and write context files. An `index.md` is pre-created in this directory.

**`tsk thread list`** returns all threads:
```json
{
  "threads": [
    { "id": 1, "slug": "fix-login", "priority": "PRIO", "state": "active", "description": "Fix the login bug" },
    { "id": 2, "slug": "update-deps", "priority": "BG", "state": "paused", "description": "Update dependencies" }
  ]
}
```

**`tsk thread switch-to`** returns the newly activated thread, same shape as `thread create`
including the `dir` field.

---

## Mode A — Autonomous agent

Use this mode when the agent is doing independent work and managing its own threads.

**You are the worker.** You decide what threads to create, when to switch, and what context
to store. tsk is your working memory across tasks.

### Responsibilities

- Create a thread for each distinct stream of work you undertake
- When activating a thread, read the files in its directory to restore context before starting
- Write notes, plans, and decisions to files in the active thread's directory as you work
- Switch threads when you decide to context-switch, or when interrupted by higher priority work
- Keep descriptions concise — they appear in the TUI at a glance

### Suggested workflow

1. `tsk thread list` — check current state before starting
2. `tsk thread switch-to <id>` — activate the thread you are working on
3. Read `tsk/threads/{id}-{slug}/` to restore context
4. Do the work; write context files as you go
5. When done or interrupted, update your context files before switching away

---

## Mode B — Human-assisted

Use this mode when the agent is helping a human manage their work threads.

**The human is the worker.** They decide what to work on and when to switch. You help them
organise their context, execute tsk commands on request, and keep their thread directories
useful. You are a co-pilot, not the pilot.

### Responsibilities

- Execute tsk commands when asked by the human
- Help the human write useful context into their active thread's directory — summaries,
  decisions made, next steps, links to relevant code or docs
- When the human switches context, offer to capture a summary of where they left off
- Suggest thread switches when you notice priority conflicts, but do not switch unilaterally
- Help the human keep descriptions and context files up to date

### Suggested workflow

1. At session start: `tsk context` to understand the current state
2. Ask the human which thread they are working on, or suggest based on current state
3. As the human works, help them document decisions and progress in the thread directory
4. When the human is interrupted: help them capture context before switching away
5. When resuming: read the thread directory together and summarise where they left off

---

## Current state
