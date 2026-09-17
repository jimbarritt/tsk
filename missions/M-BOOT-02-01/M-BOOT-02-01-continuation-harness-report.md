# Report: M-BOOT-02-01, continuation harness

| Field | Value |
|---|---|
| Mission | M-BOOT-02-01 |
| Outcome | Done |
| Attempts | 1 |
| Actor | Cloud session `cse_0159M5Ma5prKJWGeHn7bzkNU`, Sonnet 5 then Opus 5 |
| Date | 2026-09-17 |
| Supervision | Supervised interactive. Jim intervened four times, and each intervention changed the outcome |

## Outcome

Done. All ten plan tasks complete, T-10 added on take-up. Every objective line in the
briefing is met, with one qualification on the last one, recorded under Proof below.

Built on `main`:

- `ops/local/thread-lib.sh`: binding resolution, thread ID minting, the written-by URN
- `ops/local/thread-mint-id.sh`, `thread-scaffold.sh`, `thread-resolve-binding.sh`
- `ops/local/thread-start.sh`, `thread-append-handover.sh`, `thread-resume.sh`
- `.claude/skills/{start,pause,resume}-thread/SKILL.md`
- `ops/local/claude-session-start.sh` extended to resolve the binding and name the
  command to run next

Built on `tsk/bootstrap`:

- `threads/`, with `lookup-by-cloud-session.json` as the cloud binding map

Changed as a consequence, not as a specified task:

- The bootstrap worktree moved out of `.git/` to an XDG state path, keyed per clone
  (ADR 0009)
- `ops/local/bootstrap-wt-lib.sh` and `bootstrap-wt-path.sh` added;
  `fetch-bootstrap-ref.sh` and `push-bootstrap-ref.sh` rewritten onto them
- `docs/user-guide/missions-threads-and-continuation.md` added
- `CLAUDE.md` tightened twice

## Proof

The full cycle was run end to end twice: once against the original scripts, once again
after the worktree relocation. Start minted and bound a thread, pause appended a
continuation event, resume read back the mission link and the what's-next text
unchanged. The `SessionStart` hook resolved the binding and named `/resume-thread` with
the right ID. Take-over was proven from a second git worktree standing in as a
different actor: the warning named the prior actor, and the bind proceeded.

**Qualification.** The objective says "start a thread, pause it, `/clear`, resume it."
No `/clear` was performed. An agent cannot invoke `/clear` on itself, as
`docs/kb/agent-context-self-regulation-and-unattended-handoff.md` already records: there
is no tool call for it. What was proven instead is that the mechanism carries no
conversational state. Resume reads three files and an environment variable, so a fresh
session reaches the same result as this one. That is the substance of the objective. It
is not the letter of it, and a human running the literal sequence would close the gap in
a minute.

Both test threads were removed afterwards and `threads/` holds only the empty lookup
file. That cleanup is T-10, which the briefing did not specify.

## What the briefing failed to give me

**The commit-on-`tsk/bootstrap` field is not implementable as written.** The design doc
names it "the commit the push script left the branch at". Read literally that is
self-referential: the event line recording the commit is part of the commit being
recorded, and no commit hash can describe a tree containing itself. I took it as the
branch's HEAD immediately before the event is appended, which matches how commit-on-main
is captured, and recorded the reasoning in the plan rather than treating it as blocking.
This is the one place I resolved an ambiguity by judgement rather than by asking. Worth
Jim confirming, since the design doc is his to change.

**No guidance on proving the cycle without `/clear`.** The objective assumes a capability
the actor does not have. The briefing's own Intelligence section points at the document
that says so. Two sections of the same briefing disagree.

**No guidance on where test data goes.** T-09 exercises scripts whose only real target is
the live `tsk/bootstrap` branch. Nothing said whether test threads were acceptable there,
or who removes them. I added T-10 to close it.

## What I found wrong, and got wrong

Four defects, all mine except the second. The fourth was found after this report was
first written, and closed as T-11.

**1. I duplicated the push sequence instead of calling the script.** The first drafts of
`thread-start.sh`, `thread-append-handover.sh` and `thread-resume.sh` each inlined
`git add` / `git commit` / `git fetch` / `git push` against `tsk/bootstrap` rather than
calling `push-bootstrap-ref.sh`. `CLAUDE.md` already prohibited this, in words that fit
exactly ("even when a hand-run command looks equivalent to what the script does"), and I
read that instruction and still wrote three copies of the sequence. I had qualified the
ref in full, so the collision hazard was not live, but the drift hazard was. Jim caught
it. `CLAUDE.md` now states that the rule binds scripts, not only commands typed directly.

**2. `fetch-bootstrap-ref.sh` destroyed uncommitted work, and had since it was written.**
It ran `git reset --hard` unconditionally. Any caller reaching for it to resolve the
worktree path silently lost whatever was uncommitted in that worktree. It discarded my
first attempt at the plan update, which is how it was found. Path resolution is now a
separate pure script, `bootstrap-wt-path.sh`, and the fetch refuses to reset over a dirty
worktree. This one predates the mission and was latent in the original design.

**3. I defended a correct mechanism instead of hearing the real complaint.** Jim stopped
work twice believing an agent was editing a git ref. Both times I was editing an ordinary
working-tree file in a linked worktree, both times I verified it and said so, and both
times I was answering the wrong question. The mechanism was fine. The path was not: a
location under `.git/` obliges everyone who reads it to re-derive that distinction, and
in a supervised context that cost is paid by the supervisor, repeatedly. Being right
about the mechanism is not the same as the design being right.

**4. The ledger fetch resets over a commit that was never pushed.** T-11.

`push-bootstrap-ref.sh` commits, then pushes. A worker restart can end a turn between
those two steps. What it leaves is a commit the worktree holds and origin does not, in a
worktree that is clean, so defect 2's dirty-tree guard does not fire and the next
`reset --hard` orphans the commit. The `SessionStart` hook runs that fetch at every
session start, so the loss runs unattended, before any agent is placed to notice it.

Both scripts changed. `push-bootstrap-ref.sh` is idempotent: it commits only when
something is staged, and pushes a commit an earlier run left behind (`e3ec479`). The
fetch now leaves the worktree as it is in that state, and the hook reports the pending
commit in `additionalContext` (`57b5320`). Refusing at session start and pushing from
the fetch were both weighed: refusing blocks the session until someone clears it by
hand, and pushing would make a read path write to origin.

The part worth keeping is Jim's addition to the message. It tells the agent the commit
may be its own work from a turn it holds no record of making, and not to assume another
actor made it. The agent that finds the commit is usually the agent that made it. I
assumed the opposite earlier the same day: I found a commit I had no record of, decided
a second session had made it, and reported a shared-worktree hazard between sessions
that does not exist. The reflog settled it in one command, which I should have run
before reasoning. Left unsaid in the hook's message, that same reading invites an agent
to discard real work.

## The worktree relocation

This was not in the plan. It came out of Jim's third intervention, and it is the most
useful thing the mission produced.

The trigger was a question I had answered badly. I told Jim the permission prompt he saw
was for `push-bootstrap-ref.sh` rather than for the edit. It was for the edit. When he
pressed on why updating a plan needs approval at all, the pattern in the session was
already visible: every `git commit` and `git push` ran unprompted, and `Edit` calls
against the `.git/` path escalated to a human. The most probable mechanism is Claude
Code's auto-mode permission classifier scoring a write whose path contains `/.git/` as
dangerous. That remains inference, since the diagnostics log records no per-call
verdicts, but the correlation held all session and the path is the obvious discriminator.

That turns a cosmetic objection into an operational one. A design that needs a human to
approve every mission-state update cannot run unattended, which is what M-BOOT-03 is for.
The location was costing two things at once: a supervisor's attention, twice in one
session, and the possibility of unattended operation.

The move: `${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/bootstrap`. Still a
linked worktree, so no second object store. State home rather than cache home, because an
in-progress mission edit sitting in that worktree is not regenerable and `~/.cache`
carries a contract that its contents may be deleted at any time. Per clone rather than per
machine, because a linked worktree belongs to one clone. Keyed by a marker minted into
`.git/tsk-clone-id`, because a hash of the repository path changes when the directory is
renamed. Reasoning recorded in ADR 0009.

**The general lesson, which is worth more than the fix.** Two independent readers, one
human and one classifier, drew the same wrong conclusion from the same path. When that
happens the path is the problem, whatever the mechanism underneath it does. I spent three
exchanges establishing that I was not doing what it looked like I was doing, and none of
them made the next reader's job easier. Changing the path made all of it unnecessary.

**A second lesson, about this report's own genre.** The briefing asks for what the
briefing failed to give me and what I found wrong in it. Three of the four most valuable
findings this session came from Jim interrupting, not from the plan. The plan's value was
in holding the shape of the work; the corrections came from supervision. For M-BOOT-03,
which targets an unattended run, that is the gap to design against: none of these four
interventions would have happened, and the first three defects would all have shipped.

## For M-BOOT-02 and what follows

- T-13 is complete. The three commands exist, are wired to the `SessionStart` hook, and
  are proven.
- **Confirm or correct the commit-on-`tsk/bootstrap` reading.** Jim's call, per Decision
  authority.
- **The `Edit` tool was retested against the new path, 2026-09-17, and the premise
  holds.** Direct `Edit` calls at the state-home location run with no prompt. The same
  edit, to the same file, was refused twice at the old `.git/` path earlier that day.
  Before and after on one file in one session, so the path is confirmed as the
  discriminator rather than inferred from correlation.
- **T-04, the thread state format, is still open and this mission deliberately did not
  encroach on it.** A continuation event carries one what's-next line. Which tasks are
  done, and which is in progress, still has no home.
- **Deferred points in the design doc stay deferred**, as the briefing instructed: the
  thread-to-Plan relationship, what two actors on one thread should actually do, and
  whether "Continuation" earns a place in the ubiquitous language on its own.
- **Naming.** `docs/domain/ubiquitous-language.md` calls the mission data store a "data
  ref" in the Nexus entry. ADR 0008 moved it onto a branch, so that term is now wrong
  wherever it appears. Jim asked for candidate terms for the two stores at the end of
  this session; the outcome of that belongs in the ubiquitous language, not here.
