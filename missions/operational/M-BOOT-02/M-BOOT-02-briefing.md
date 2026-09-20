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
| T-16 | Install the `software-english-lint` plugin in this repo | `CLAUDE.md`'s Software English section already names the plugin as a backstop ("if installed"); it is not installed yet. Every reply and every changed document gets checked against the spec automatically, not only by the agent's own self-check. Raised by Jim, 2026-09-18. `jimbarritt/claude-plugins` was already a valid marketplace (registered name `jimbarritt-claude-plugins`, from its own manifest), so T-09's marketplace-repo-creation scope did not apply here. Put on hold, 2026-09-19, over the coupling reported in https://github.com/jimbarritt/claude-plugins/issues/2: no per-hook toggle, and the Stop hook's chat-reply check and its tracked-markdown check were inseparable. Resolved same day: the plugin shipped per-hook config (`.claude/swe-lint.json`, project-level, overriding the plugin's own `config.json`), so Jim lifted the hold. Installed via `.claude/settings.json` (`extraKnownMarketplaces`/`enabledPlugins`) plus an idempotent `claude plugin install --scope project -y` in the `SessionStart` hook, since declaring `enabledPlugins` alone does not install the plugin; `stop.reply` turned off via `.claude/swe-lint.json`, keeping `stop.docs` (tracked-markdown check) on. Note for the record: under Claude Code, `stop.reply`'s check always skips regardless of this setting (the plugin's forced output style covers the reply instead) — Copilot CLI is where the setting actually bites. Pushed to `main`: `d8705d7` (install), `b010968` (config).

Corrected, 2026-09-19, same day: verified live on a local/CLI session only (`claude plugin list` showed it enabled at project scope, its skill files present on disk); a genuinely fresh cloud session tested after did not pick it up (no `/swe-*` commands). Root cause, confirmed against `docs/cloud-environments.md`, is structural, not a config error: a cloud environment's own setup script runs before Claude Code launches (so it cannot run `claude plugin install`, which needs Claude Code already up); the repo's own `SessionStart` hook runs after Claude Code has already decided the session's plugin set (so its install line is always one step too late for that session); and a cloud container's `~/.claude` does not persist across sessions (confirmed: plugin cache is listed under "does not carry over"), so unlike a local machine there is never a later session that benefits from an earlier one's install. No officially documented mechanism closes this gap today. One unverified, unbuilt possibility: the environment's filesystem-snapshot caching across sessions might let a setup script stage the plugin's files directly onto the cache path `claude plugin install` would have written, skipping the live command entirely, but that depends on an undocumented internal layout and was not attempted. Status downgraded from DONE: works for a local/CLI session, not for a cloud one.

Re-confirmed working in a cloud session, 2026-09-19, later the same day: session `cse_01CC9mGXaK6DHJNfpyKVwr9s` (`CLAUDE_CODE_REMOTE=true`) shows `claude plugin list` reporting `swe@jimbarritt-claude-plugins` enabled at project scope, `installed_plugins.json` recording the install one minute before the session's own thread registration, and its `/swe-*` skills and `swe:Software English` output style both live and selectable in the session. Same install mechanism as the downgrade above (`ops/local/claude-session-start.sh`'s idempotent `claude plugin install --scope project -y` at every `SessionStart`), opposite result. This contradicts the downgrade's structural argument that the hook always runs one step too late for its own session: here the plugin was available in the very session whose own `SessionStart` hook installed it. What differed between the two tests is not established; the downgrade's cloud test used a different, unrecorded session, so there is no side-by-side comparison to explain the discrepancy, only two data points pointing opposite ways with this one more recent. Status restored to DONE | none | none | DONE |
| T-17 | Experiment with a `SessionEnd` hook that auto-triggers the pause/handover flow on `/clear` | Confirmed against the hooks reference (https://code.claude.com/docs/en/hooks.md): `SessionEnd` fires with `reason: "clear"` and a `transcript_path`, but only after `/clear` has already run, so it can react, not block. A command hook can call `append-handover.sh` for the deterministic fields (mission link, task ID, commit hashes) but not compose "what's next", which the handover schema makes agent-written by design (`docs/domain/session-continuation-design.md`); needs either a transcript-tail heuristic or a `type: "agent"`/`type: "prompt"` hook that re-invokes Claude on the transcript. This is a different, available-now mechanism from the `/goal`-driven trigger the design doc names as the intended later path ("Decided: pausing is manual for now, scoped to the supervised context", `intel-index.md`); reconcile the two rather than building both. Raised by Jim, 2026-09-19 | none | none | TODO |
| T-18 | Give a thread an autonomous-vs-supervised state | A thread records whether it is currently running autonomously or under supervision, so the harness can decide whether to pause and ask Jim a question or carry on unattended. Needs a further design question, not yet worked: how the agent infers that something is serious enough to escalate even while running autonomously, beyond the routine case of just asking. Jim's longer-term direction: escalation reaches Jim over a messaging app, since blocking a session nobody is watching achieves nothing. Raised by Jim, 2026-09-19, alongside T-17; capture only, no design done yet | none | none | TODO |
| T-19 | Add scheduled repo cleaning agent | Raised by Jim, 2026-09-19, from a live test against M-ADMIN-03 (dependency vulnerability watch). A Routine created with `create_trigger` fired a session with two access gaps, not one: (a) missing every `mcp__github__*` tool, found from outside by inspecting the fired session's own tool list (the tool also warned on creation that it stores no MCP connectors and had none to pass through), and (b), found from inside the fired session's own run and the more severe half, no git push credential for its own home repo at all, `jimbarritt/tsk` — every push, to `main` and to `tsk/bootstrap`, failed with a proxy 403 saying the repo was not in the session's authorized set, while clone and fetch worked throughout. Full account: [../../administrative/M-ADMIN-03/M-ADMIN-03-dependency-vulnerability-watch-report.md](../../administrative/M-ADMIN-03/M-ADMIN-03-dependency-vulnerability-watch-report.md). Root cause, from Anthropic's Routines docs: the web UI's routine creation has an explicit repository-selection step that provisions both GitHub API and git write access for the run; `create_trigger` has no equivalent field, and `add_repo` itself is absent from a fired session's tool list, so there is no in-session way to self-serve either gap. Needs either recreating the routine from claude.ai/code/routines directly, or a mechanism this harness can invoke programmatically that grants the same access. Update, 2026-09-19, after Jim edited the routine to attach the repo directly and re-fired it: attaching the repo through the web UI resolves both the git-push and the `mcp__github__*` tool-list halves of the gap, confirmed live (`git push --dry-run` succeeded, the token showed `admin`/`push`/`pull` all true, GitHub tools appeared). A third, narrower gap surfaced once those two closed: the GitHub App installation's token still lacks the `security_events` (or equivalent Dependabot alerts) scope, so `GET .../dependabot/alerts` returns 403 even with working push and other GitHub API access. This is a permission-grant gap on the GitHub App installation itself, not something `create_trigger` or the routine's repo attachment controls. Fix is either broadening that installation's permissions, or supplying a scoped personal access token as the environment's own credential so a run can call the Dependabot API directly regardless of the App's grant. Full account, including the addendum: [../../administrative/M-ADMIN-03/M-ADMIN-03-dependency-vulnerability-watch-report.md](../../administrative/M-ADMIN-03/M-ADMIN-03-dependency-vulnerability-watch-report.md). M-ADMIN-03 is the first mission blocked on this. Confirmed directly, 2026-09-19, in an interactive cloud session: the environment's own API-credentials panel holds a classic PAT, `gh-admin-pat`, scoped to `api.github.com`, with Jim confirming the security-events (Dependabot alerts) box is ticked on it. A `GET .../dependabot/alerts` call still returned 403 "Resource not accessible by integration", so the response headers were inspected directly: `X-Oauth-Scopes` was empty (a classic PAT always lists its scopes there), `X-Oauth-Client-Id` was `Iv23liqTIFEtdIu6Vn1r` (a GitHub App client ID, not a personal account), `Github-Authentication-Token-Expiration` was about three hours out (App installation tokens expire on that timescale; classic PATs don't), and the rate limit was 15000/hour (App-installation scale, not a personal account's 5000). Conclusion: this session's `api.github.com` traffic authenticates as a GitHub App installation, not as `gh-admin-pat`. Ticking the scope on the PAT does not close this gap, because the PAT is not in the request path for these calls. The permission-grant gap is confirmed to sit on the GitHub App installation, not on any credential this harness can configure from inside a session. Idea raised by Jim, 2026-09-19, in response: sidestep the harness's own credential routing entirely by adding a GitHub Action, running under the repository's own Actions permissions rather than the Claude Code proxy, that reads Dependabot alerts and pushes them somewhere tsk can read — a deterministic script, not an agent call. Not yet designed or built.

Design settled, 2026-09-19, discussion with Jim. A GitHub Action in this repo, triggered on `dependabot_alert`, `code_scanning_alert` (CodeQL alerts fire here), `secret_scanning_alert` and `secret_scanning_alert_location`, running under the workflow's own `GITHUB_TOKEN` with `contents: write` only. No PAT, no Claude Code proxy in the path at all, and no `security-events: read` needed either: these trigger events carry the full alert object in the payload GitHub delivers, so nothing calls the security-events API. It checks out the repo (`actions/checkout`), runs `fetch-bootstrap-ref.sh`, appends one line to `external-events/queue.ndjson` in the worktree, then runs `push-bootstrap-ref.sh`, reusing the existing collision handling rather than duplicating it, per CLAUDE.md's rule against a second copy of that logic. Each line is an envelope: `source`, `event_type`, `action`, `received_at`, `repo`, and the raw GitHub event payload nested under `payload`. One shared file for every source for now, named "the external event queue"; splittable by source later if needed. The consumer is M-ADMIN-03: on each run it reads the queue, skips past `external-events/watermark.json`'s line count, processes what is new one event at a time, and writes the new count back. Each file has exactly one writer (the Action writes `queue.ndjson`, M-ADMIN-03 writes `watermark.json`), so the only conflict case left is two M-ADMIN-03 runs firing at once, already covered by `push-bootstrap-ref.sh`'s retry.

Producer side built and pushed, 2026-09-19: `.github/workflows/external-security-events.yml` and `ops/local/append-external-event.sh` on `main` (`abae0af`). Proven live end to end with a synthetic `dependabot_alert` payload run through the script directly (not yet through an actual GitHub-fired trigger): appended, committed and pushed to `external-events/queue.ndjson` on `tsk/bootstrap` correctly. That synthetic line is still sitting in the real queue, harmless since nothing consumes it yet. Remaining before this task is done: a real GitHub-triggered run (needs an actual alert or a manual workflow dispatch to prove the trigger wiring, not just the script), and the M-ADMIN-03 consumer side (reading the queue, the `external-events/watermark.json` cursor, processing loop) — neither built yet.

Trigger wiring corrected and proven live, 2026-09-20. `dependabot_alert`/`code_scanning_alert`/`secret_scanning_alert` turned out not to be valid GitHub Actions `on:` triggers at all (they are webhook event types, a different, smaller catalog); every run failed at parse time with zero jobs, always firing on ordinary pushes to `main` instead. Replaced with `schedule` (hourly) plus `workflow_dispatch`, calling the alert list APIs directly. First attempt used the workflow's own automatic `GITHUB_TOKEN` scoped `security-events: read`: confirmed live this covers code scanning alerts only, since Dependabot alerts and secret scanning alerts both reject an installation-type token outright per GitHub's own API docs for those two endpoints, independent of declared permissions. Fixed by having Jim create a GitHub Actions Environment, "Repo Admin", holding a personal access token as the secret `GH_PAT`; the job now declares that environment and the script uses `GH_PAT` throughout. Triggered manually and confirmed: queued exactly 12 open alerts, matching GitHub's own reported count, committed to `external-events/queue.ndjson` on `tsk/bootstrap` (`f7ea174`). Code on `main`: `68a8d1d`.

Read the queued alerts and fixed what was fixable, same session, 2026-09-20 (Jim: "I want to fix the actual errors"). All 12 are in `docs/slide-decks/overview-for-engineers/pnpm-lock.yaml`: 10 through `@xmldom/xmldom` transitively via `marp-cli`, 2 through `extract-zip` transitively via `puppeteer-core`. GitHub's own alert data names `0.9.12` as `first_patched_version` for all 10 xmldom alerts; added a `pnpm.overrides` entry pinning it, reinstalled, confirmed the deck still builds. The 2 `extract-zip` alerts carry `first_patched_version: null`: no fixed version exists upstream for `<=2.0.1`, confirmed by `pnpm audit` too, so both stay open, not silently skipped. Pushed to `main`: `d1996f1`. This also updates the 15-vulnerability estimate from M-ADMIN-03's earlier local-audit substitute (correct approach at the time, since the live API was blocked) to the actual live count, now 2.

Still remaining for this task: the M-ADMIN-03 consumer side (reading the queue, the watermark cursor, the per-event processing loop) — not built yet. The producer side (this Action) is proven end to end with real data.

Consumer side built, 2026-09-20 (Jim: daily Routine, checks the queue, makes fixes, a fresh report every run). `ops/local/read-new-external-events.sh` and `ops/local/advance-external-events-watermark.sh` on `main` (`e2cb46c`), tested end to end against the real queue (17 events, watermark advanced, re-read correctly showed zero new, backwards-move and bad-input both rejected). M-ADMIN-03's own briefing rewritten to match: reads the queue instead of calling Dependabot's API, per-alert-type fix scope added (Dependabot alerts with a published fix get fixed and pushed unattended; code scanning alerts triaged and written up, never auto-fixed, since the finding usually needs a real code change; secret scanning alerts never auto-remediated at all, since the actual fix is revoking or rotating a real credential, which only Jim can do). Ledger: `tsk/bootstrap` `22f94f4`.

Routine created: `trig_019AVSCvfqAWSVzeKV88p863`, daily `0 7 * * *` UTC, fresh session per firing, resumes the mission's standing thread `uu44oy4p` rather than asking which mission to work (the prompt names it directly, since nobody is present to answer). Carries the same warning T-19 already recorded above: no MCP connectors, no repo attached, so the fired session will hit the same access gap as the first M-ADMIN-03 Routine did. Needs the same fix Jim applied then: attach `jimbarritt/tsk` to it directly from claude.ai/code/routines. Not yet done, and not yet fired even once — this task stays TODO until a real firing is confirmed working end to end. | none | none | TODO |
| T-20 | Add `/detach-thread`, `/stop-thread`, and `/switch-thread` | Raised by Jim, 2026-09-19: detach removes the current session's or worktree's binding without deleting the thread; stop does that then deletes the thread (its directory, and every cloud-session lookup entry pointing at it — a worktree marker in some *other* worktree still naming it cannot be reached and goes stale silently); switch composes both, asks whether to stop the thread just left, then resumes a named or chosen thread. Design added to `docs/domain/session-continuation-design.md` (`main`), backed by `ops/local/thread-detach.sh`, `thread-stop.sh`, `thread-list.sh`, and three new skills, mirroring the existing `start`/`pause`/`resume` pattern. Verified end to end against an isolated test clone bound via its own worktree marker, not this session's real cloud binding: start, detach, resume, stop each behaved as designed, and the deleted thread confirmed absent from the pushed tree; the cloud-lookup purge and single-key removal filters were unit-tested against a throwaway copy of the lookup file's shape, not the live file. Pushed to `main`: `c22dd07`.

Fixed, 2026-09-19, same day: `thread_wt()` (`thread-lib.sh`) returned `$TSK_BOOTSTRAP_WT` directly whenever set and the directory existed, on the assumption it always names the current clone's own worktree. True for the `SessionStart` hook's own clone, false for a script run from a different clone while the variable is still exported from an earlier session: the two resolve to different clone IDs, so a thread script could scaffold a thread into one worktree while `push-bootstrap-ref.sh`, which never reads the variable, pushed against a different, nonexistent one. `thread_wt()` now always resolves through `bootstrap-wt-path.sh`, the same canonical path `push-bootstrap-ref.sh` and `fetch-bootstrap-ref.sh` already use, so the two cannot disagree. Verified against the same reproduction: `thread_wt()` now returns the same path as `bootstrap_wt_path()` even with a foreign `TSK_BOOTSTRAP_WT` set. Pushed to `main`: `f047228` | none | none | DONE |
| T-21 | Build a latency harness and find why replies feel slower | Raised by Jim, 2026-09-19/20. Three candidates proposed: the `swe` Stop hook's reply check, its `PostToolUse` check on `Write`\|`Edit`, and its output style raising reasoning tokens. Built `ops/local/run-latency-harness.py` (methodology in the adjacent `.md`): headless `claude -p`, one fresh session per run, reads `duration_ms`/`ttft_ms`/token counts/cost from the result JSON and per-hook `durationMs` from the session transcript, no stopwatch. Three conditions (plugin off; plugin on, output style stripped from a staged copy, since the style's `force-for-plugin: true` makes "plugin on, default style" otherwise unreachable through settings; plugin on intact) isolate the plugin's hooks from its style. Pilot (one repeat, short prompt) found the Stop hook costs ~2037 ms on every turn, including a plain chat reply with nothing to check, reproduced in a clean git repo with no working-tree changes. Narrowed further: the hook's own two sub-steps (the data-fetch script, the linter's `--diff --added-only`) each complete in under 100 ms run directly; wrapped in the hook's own backgrounded subshell and watchdog, the same work costs ~2037 ms, pointing at the hook's process orchestration rather than the check content. Filed upstream: https://github.com/jimbarritt/claude-plugins/issues/6. Output style's own latency cost not established at one repeat; every condition recorded zero thinking tokens at this prompt length. Pushed to `main`: `a9ff714` (harness) | none | none | TODO |

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
