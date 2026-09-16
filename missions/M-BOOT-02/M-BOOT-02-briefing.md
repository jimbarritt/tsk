# Mission: Harness

| Field | Value |
|---|---|
| ID | M-BOOT-02 |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | M-BOOT-01 |

## Objective

Kind: attainable

- A local Claude Code session in the tsk repo loads the harness without error.
- A test cloud session clones the tsk repo, loads the harness, and reads a briefing from
  the repository.
- The `SessionEnd` hook pushes that session's transcript to the transcripts repo.
- The session writes thread state at its end, recording which tasks are done and where
  to resume.

## Purpose

Parent: M-BOOT, bootstrap tsk self hosting. An agent executes inside the harness. The
harness is the envelope. The mission briefing sits inside it.

## Intelligence

See [intel-index.md](intel-index.md).

## Decision authority

Jim decides the harness structure, the run record format, and what belongs to ksobr
rather than tsk.

## Constraints

- Hooks must be bash. Cloud virtual machines are Linux. PowerShell hooks do not run.
- Files must use LF line endings. A bash script with CRLF fails on Linux.
- No secrets in the repo. The cloud environment has no secrets store, and environment
  variables are visible to anyone who can edit the environment.
- Ways of working belong in the harness, not in a mission. That is ksobr domain.
- `CLAUDE.md` points at `docs/`. It does not restate the design.

## Out of scope

- Building ksobr beyond what this bootstrap needs.
- Resuming from thread state. M-BOOT-03 proves that.
- Level 3 reflection, a separate scoring pass. Deferred until there are enough runs to
  know what to score.

## Plan

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Move the missions into the repository | M-BOOT and its breakout briefings readable from a fresh clone. A cloud session cannot read Jim's home directory | none | DONE |
| T-02 | Incorporate the mission briefing template into the harness | The harness points a session at `docs/domain/mission-briefing-template.md` and states the first behaviour: take ownership of the plan before any other action. No copy in `.claude/`: tsk is the only repo running this harness, so the docs directory can be relied on. Revisit if another repo installs it | none | TODO |
| T-03 | Define the run record format | Level 1 outcome: done, failed or blocked, with attempt count. Level 2: the actor's account of what it did, what the briefing failed to give it, and what it found wrong | none | TODO |
| T-04 | Define the thread state format | Records which tasks are done, which is in progress, and where to resume. Written at the end of every session. Readable by a different actor. Open first: whether the definition of threads is right, see Open decisions | none | TODO |
| T-05 | Add the hook that writes thread state | Thread state written alongside the missions at session end | T-04 | TODO |
| T-06 | Add the `SessionEnd` hook for transcripts | Hook pushes the session transcript to `ksobr-transcripts`. Settled: the hook cannot attach the repo itself, since `add_repo` is an MCP tool call the agent makes and a bash hook has no path to MCP tools; attaching stays an instruction the agent follows, which lives in tsk's `CLAUDE.md` for now, as a stopgap | none | TODO |
| T-07 | Write `CLAUDE.md` | Points at `docs/` and the template. Holds ways of working | T-02 | TODO |
| T-08 | Configure `.claude/settings.json` | Hooks wired, permissions set, local session loads without error | T-05, T-06, T-07 | TODO |
| T-09 | Create the plugin marketplace repo | Harness and the language linter declared in `.claude/settings.json` and installed by a setup script | T-08 | TODO |
| T-10 | Configure the cloud environment | Network access, environment variables, and a setup script that installs the harness and the linter | T-09 | TODO |
| T-11 | Confirm GitHub repo access for cloud sessions | A test cloud session clones the tsk repo and reads a briefing | T-10 | TODO |
| T-12 | Configure `CLAUDE.md` with the Software English compact instructions | Agents in this repo write in Software English by default, in replies and in anything written into a file. Includes the one question at a time rule. Spec: https://github.com/jimbarritt/software-english. Overlaps T-07, which holds ways of working | none | TODO |

**Essential task**: T-11. Repo access denial is the most common cloud routine failure,
and nothing downstream works without it.

## Open decisions

- Where the missions live in the repository: a directory, or a git ref. T-01 decides.
- Run record location: with the missions, or the transcripts repo. T-03 needs it.
- Whether the definition of threads is right. Jim's current reading: a thread is more
  about the actors than about overall work status, which is not what the ubiquitous
  language entry says today. Settle this before T-04 defines a format on top of it.
- Where thread state lives once it has a format: its own artefact, or the sections of
  `index.md` that hold it today. Deferred until T-04. Until then `index.md` keeps the
  summary of missions and tasks with their status.
- How a mission briefing reaches an individual agent session, so the session knows what
  it is working on. Raised by Jim, 2026-09-15. Not a state question: the data ref
  (`tsk/bootstrap`, or its successor) has no concept of a session, so this cannot be
  answered by anything held in state. Likely candidates: the session's initial prompt
  names the mission, or a `SessionStart` hook reads a pointer from somewhere and injects
  the briefing as context. Undecided, and the next thing to work.

  The mechanics are now established and written up:
  `docs/kb/session-creation-and-environments.md` in the tsk repo on `main`
  (https://github.com/jimbarritt/tsk/blob/main/docs/kb/session-creation-and-environments.md).
  It covers which mechanisms start a session and which an orchestrator can invoke, the
  three levers that set what a session knows (initial prompt, repository contents,
  environment), and the limits of reusing a long-lived session. Read it before designing
  anything, rather than re-deriving it.

  Jim has ideas on the design and wants to discuss them first. Do not start implementing.

- **Handoff**, as a concept to follow up on. Named by Jim, 2026-09-16, after a session
  ended by writing the next step and its groundwork down for whoever picks the work up
  next. Whether it earns a place in the ubiquitous language is open, and so is its
  relationship to thread state: T-04 already describes recording which tasks are done,
  which is in progress and where to resume, written at the end of every session and
  readable by a different actor, which is close to the same thing under another name.
  Resolve the overlap rather than defining both.

- `index.md` is the wrong home for what is next. Noted by Jim, 2026-09-16. Recording the
  next step there means every session has to edit the index to say where it got to, which
  makes a navigational file carry state that changes on every run. This sharpens the
  earlier open decision above about where thread state lives: that one asks whether the
  index keeps those sections, this one says it should not. To be solved in the next
  session.

- Sessions registering themselves against the mission, as a mechanism for working out
  where to start. Raised by Jim, 2026-09-16, to explore. A candidate answer to the open
  decision above on how a briefing reaches a session, and possibly to handoff too: rather
  than a session reading a single written-down pointer, a session could announce itself
  against the mission it is working, so the mechanism for finding where to start is
  registration rather than a note left behind. Not shaped further than this. Explore
  alongside handoff rather than instead of it.

## Handover

In progress: researching how to build a "continuation harness" — an agent session that
can hand off to its own successor before running out of context, rather than a human
restarting it cold. Pick this up in the next session.

Read, in order: `docs/kb/session-creation-and-environments.md` and
`docs/kb/agent-context-self-regulation-and-unattended-handoff.md`, both in the tsk repo
on `main`, then this mission's own intel-index.md for the mission-specific thread (the
working hypothesis, checked and refuted in part, and the confirmed findings on `/goal`
and `get_session`). Jim has ideas on the design and wants to discuss them before
anything is built.

As of 2026-09-16, the self-regulation doc now opens with four operating contexts —
supervised interactive, unsupervised autonomous, orchestrator spawning workers,
event-triggered — that frame the whole document. These are a separate axis from the
four mechanisms (compaction, context awareness, agent-directed persistence, session
inspection): a mechanism applies differently, or not at all, depending on which context
a session is running in. Read the operating contexts first; they're at the top of the
doc for a reason.

Context 1, supervised interactive, is the least developed and the most immediately
useful to get right: it's the context this harness itself runs in, and the problem it
names — a session restart gives a new session ID, so continuity across that boundary
needs an identifier that outlives the ID — was demonstrated live during this document's
own writing, not hypothetically. Jim considers this the one to nail next.
