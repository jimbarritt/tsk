# Mission: Harness

| Field | Value |
|---|---|
| ID | M-BOOT-02 |
| Territory | agentic research |
| Assignee | Jim |
| Blocked by | M-BOOT-01 |

## Objective

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

| ID | Task | Objective | Delegated to | Blocked by | Status |
|---|---|---|---|---|---|
| T-01 | Move the missions into the repository | M-BOOT and its breakout briefings readable from a fresh clone. A cloud session cannot read Jim's home directory | none | none | DONE |
| T-02 | Incorporate the mission briefing template into the harness | The harness points a session at `docs/domain/mission-briefing-template.md` and states the first behaviour: take ownership of the plan before any other action. No copy in `.claude/`: tsk is the only repo running this harness, so the docs directory can be relied on. Revisit if another repo installs it | none | none | TODO |
| T-03 | Define the run record format | Level 1 outcome: done, failed or blocked, with attempt count. Level 2: the actor's account of what it did, what the briefing failed to give it, and what it found wrong | none | none | TODO |
| T-04 | Define the thread state format | Records which tasks are done, which is in progress, and where to resume. Written at the end of every session. Readable by a different actor. Thread definition and its relationship to Mission are both settled, see Open decisions; the reverse-lookup design there is a dependency, not yet implemented | none | none | TODO |
| T-05 | Add the hook that writes thread state | Thread state written alongside the missions at session end | none | T-04 | TODO |
| T-06 | Add the `SessionEnd` hook for transcripts | Hook pushes the session transcript to `ksobr-transcripts`. Settled: the hook cannot attach the repo itself, since `add_repo` is an MCP tool call the agent makes and a bash hook has no path to MCP tools; attaching stays an instruction the agent follows, which lives in tsk's `CLAUDE.md` for now, as a stopgap | none | none | TODO |
| T-07 | Write `CLAUDE.md` | Points at `docs/` and the template. Holds ways of working | none | T-02 | TODO |
| T-08 | Configure `.claude/settings.json` | Hooks wired, permissions set, local session loads without error | none | T-05, T-06, T-07 | TODO |
| T-09 | Create the plugin marketplace repo | Harness and the language linter declared in `.claude/settings.json` and installed by a setup script | none | T-08 | TODO |
| T-10 | Configure the cloud environment | Network access, environment variables, and a setup script that installs the harness and the linter | none | T-09 | TODO |
| T-11 | Confirm GitHub repo access for cloud sessions | A test cloud session clones the tsk repo and reads a briefing | none | T-10 | TODO |
| T-12 | Configure `CLAUDE.md` with the Software English compact instructions | Agents in this repo write in Software English by default, in replies and in anything written into a file. Includes the one question at a time rule. Spec: https://github.com/jimbarritt/software-english. Overlaps T-07, which holds ways of working | none | none | DONE |
| T-13 | Build `/start-thread`, `/pause-thread` and `/resume-thread` | Design complete, recorded in `docs/domain/session-continuation-design.md` on `main` and this mission's `intel-index.md`. Delegated for implementation | M-BOOT-02-01 | none | DONE |
| T-14 | Thread binding resilience | Raised by Jim, 2026-09-17, as a fix for missed auto-resume after a pause and a `/clear`. Grew in scope during its own investigation: the real gap is that a session can be left unbound after `SessionStart` fires, in either direction, if other work distracts the agent before it acts on the hook's own instruction. Delegated, completed under M-BOOT-02-02 | M-BOOT-02-02 | none | DONE |
| T-15 | Fix the double fetch of `tsk/bootstrap` on `SessionStart` and `/resume-thread` | Noted in M-BOOT-02-02's handover, 2026-09-18: `claude-session-start.sh` fetched `tsk/bootstrap` twice per firing, since `thread_resolve_binding` always fetched even when the caller had just fetched the same ref itself. The same bug also hit `thread-resume.sh` (found live while resuming this thread). Fixed by giving `thread_resolve_binding` an optional already-fetched worktree path, threaded through `thread-resolve-binding.sh` and both call sites. Executed inline as an ad-hoc task, no delegation | none | none | DONE |
| T-16 | Install the `software-english-lint` plugin in this repo | `CLAUDE.md`'s Software English section already names the plugin as a backstop ("if installed"); it is not installed yet. Every reply and every changed document gets checked against the spec automatically, not only by the agent's own self-check. Raised by Jim, 2026-09-18. Overlaps T-09, which covers creating a plugin marketplace repo more generally; reconcile scope when picked up rather than duplicating the marketplace setup. On hold, 2026-09-19: installing today turns on all five of the plugin's hooks uniformly (Stop, Write/Edit, Bash, Artifact, MCP-send), with no per-hook toggle, and the Stop hook couples the chat-reply check to the only check that covers tracked markdown, so the two cannot be selected independently either. Feature request filed: https://github.com/jimbarritt/claude-plugins/issues/2. Held until the plugin supports narrower selection | none | none | ON HOLD |
| T-17 | Experiment with a `SessionEnd` hook that auto-triggers the pause/handover flow on `/clear` | Confirmed against the hooks reference (https://code.claude.com/docs/en/hooks.md): `SessionEnd` fires with `reason: "clear"` and a `transcript_path`, but only after `/clear` has already run, so it can react, not block. A command hook can call `append-handover.sh` for the deterministic fields (mission link, task ID, commit hashes) but not compose "what's next", which the handover schema makes agent-written by design (`docs/domain/session-continuation-design.md`); needs either a transcript-tail heuristic or a `type: "agent"`/`type: "prompt"` hook that re-invokes Claude on the transcript. This is a different, available-now mechanism from the `/goal`-driven trigger the design doc names as the intended later path ("Decided: pausing is manual for now, scoped to the supervised context", `intel-index.md`); reconcile the two rather than building both. Raised by Jim, 2026-09-19 | none | none | TODO |
| T-18 | Give a thread an autonomous-vs-supervised state | A thread records whether it is currently running autonomously or under supervision, so the harness can decide whether to pause and ask Jim a question or carry on unattended. Needs a further design question, not yet worked: how the agent infers that something is serious enough to escalate even while running autonomously, beyond the routine case of just asking. Jim's longer-term direction: escalation reaches Jim over a messaging app, since blocking a session nobody is watching achieves nothing. Raised by Jim, 2026-09-19, alongside T-17; capture only, no design done yet | none | none | TODO |
| T-19 | Add scheduled repo cleaning agent | Raised by Jim, 2026-09-19, from a live test against M-ADMIN-03 (dependency vulnerability watch). A Routine created with `create_trigger` fired a session with two access gaps, not one: (a) missing every `mcp__github__*` tool, found from outside by inspecting the fired session's own tool list (the tool also warned on creation that it stores no MCP connectors and had none to pass through), and (b), found from inside the fired session's own run and the more severe half, no git push credential for its own home repo at all, `jimbarritt/tsk` — every push, to `main` and to `tsk/bootstrap`, failed with a proxy 403 saying the repo was not in the session's authorized set, while clone and fetch worked throughout. Full account: [../../administrative/M-ADMIN-03/M-ADMIN-03-dependency-vulnerability-watch-report.md](../../administrative/M-ADMIN-03/M-ADMIN-03-dependency-vulnerability-watch-report.md). Root cause, from Anthropic's Routines docs: the web UI's routine creation has an explicit repository-selection step that provisions both GitHub API and git write access for the run; `create_trigger` has no equivalent field, and `add_repo` itself is absent from a fired session's tool list, so there is no in-session way to self-serve either gap. Needs either recreating the routine from claude.ai/code/routines directly, or a mechanism this harness can invoke programmatically that grants the same access. Update, 2026-09-19, after Jim edited the routine to attach the repo directly and re-fired it: attaching the repo through the web UI resolves both the git-push and the `mcp__github__*` tool-list halves of the gap, confirmed live (`git push --dry-run` succeeded, the token showed `admin`/`push`/`pull` all true, GitHub tools appeared). A third, narrower gap surfaced once those two closed: the GitHub App installation's token still lacks the `security_events` (or equivalent Dependabot alerts) scope, so `GET .../dependabot/alerts` returns 403 even with working push and other GitHub API access. This is a permission-grant gap on the GitHub App installation itself, not something `create_trigger` or the routine's repo attachment controls. Fix is either broadening that installation's permissions, or supplying a scoped personal access token as the environment's own credential so a run can call the Dependabot API directly regardless of the App's grant. Full account, including the addendum: [../../administrative/M-ADMIN-03/M-ADMIN-03-dependency-vulnerability-watch-report.md](../../administrative/M-ADMIN-03/M-ADMIN-03-dependency-vulnerability-watch-report.md). M-ADMIN-03 is the first mission blocked on this | none | none | TODO |

**Essential task**: T-11. Repo access denial is the most common cloud routine failure,
and nothing downstream works without it.

## Open decisions

- Where the missions live in the repository: a directory, or a git ref. T-01 decides.
- Run record location: settled, 2026-09-16. With the missions, not the transcripts
  repo: `missions/{id}/{id}-{slug}-report.md`, in a subdirectory created when the
  mission starts executing. See the Report location field note in
  `docs/domain/mission-briefing-template.md` on `main`. T-03 can build on this.
- Whether the definition of threads is right. Settled, 2026-09-16: Jim's reading held.
  Thread is unchanged in substance (still the execution sequence, still resumable
  possibly as a different actor) but Actor is now a defined term in its own right
  (`docs/domain/ubiquitous-language.md`), distinguished from a platform session by
  cardinality — a human holds many threads, an agent session is bound to one — and
  thread identity is now explicitly tsk's own, not borrowed from a session ID or a
  worktree, since neither is stable across every surface (confirmed: a cloud session's
  ID survives `/clear`, a CLI session's does not). T-04 can build on this rather than
  waiting on it. The thread-to-mission relationship is also settled, 2026-09-16: loose,
  not fixed. Usually one-to-one in practice, but a thread is scoped to an actor's
  continuity, not to a mission's, so a maintenance or coordination actor's thread can
  carry tasks across several missions over its life. Recorded in
  `docs/domain/ubiquitous-language.md`, Thread entry.
- Where thread state lives once it has a format: its own artefact, or the sections of
  `index.md` that hold it today. Deferred until T-04. Until then `index.md` keeps the
  summary of missions and tasks with their status.
- How a mission briefing reaches an individual agent session, so the session knows what
  it is working on. Raised by Jim, 2026-09-15. Not a state question: the ledger
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
  where to start. Raised by Jim, 2026-09-16, to explore. Superseded by a more precise
  design, 2026-09-16: a session registers itself against a thread, not directly against
  a mission, since that is the actual binding decided (session ID or worktree to thread
  ID) and mission is at most one loose step further. See "Design: reverse-lookup from
  binding to thread" in intel-index.md for the algorithm. Not yet implemented — the
  binding table it depends on doesn't exist yet, and neither does the thread ID minting
  scheme.


## Handover

Cross-session experiment concluded, 2026-09-16: whether a session ID survives `/clear`.
Result: it depends on the surface. A cloud session's ID and `worker_epoch` both survive
`/clear`; the CLI issues a new session ID on `/clear` instead. Neither is thread
identity in either case — that conclusion is now written into the domain model, not
just this mission. Don't re-run this experiment; read the findings instead.

Read, in order:

1. `docs/domain/ubiquitous-language.md` on `main` — Actor and Thread entries. Actor is a
   new term: cardinality (a human holds many threads, an agent session holds one) is
   what distinguishes it from a thread, and thread identity is tsk's own, never a
   platform session ID or a worktree path.
2. `docs/kb/agent-context-self-regulation-and-unattended-handoff.md` on `main` — context
   1 (supervised interactive), now split into cloud and CLI sub-contexts with the
   experiment's findings and the same resolution.
3. This mission's own intel-index.md, for the full experimental trail (four data
   points) and the fable follow-up on self-regulation mechanisms generally.

State of the mission: the thread-definition open decision is settled (see Open
decisions). T-04 (thread state format) is unblocked. T-13 (a scripted handover skill)
is still blocked: it needs the thread/binding design worked out, not just named, before
it can be automated. The remaining open question is whether a thread binds to a mission
or a mission to a thread — work that next.

Outgoing session, for reference only, not for another experiment:
`session_01WePrEonPkCV4kfJPK9D4Sy`, `worker_epoch` 34 at handover (one more than the
last recorded reading of 33, with a model switch back to Sonnet the only change in
between — consistent with, not yet proof of, "any model switch increments the epoch").
