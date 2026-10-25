# Report: M-BOOT-02-02, thread binding resilience

| Field | Value |
|---|---|
| Mission | M-BOOT-02-02 |
| Outcome | Done |
| Attempts | 1 |
| Actor | Cloud session `cse_01WePrEonPkCV4kfJPK9D4Sy`, Sonnet 5 |
| Date | 2026-09-18 |
| Supervision | Supervised interactive. Jim confirmed the mechanism choice once, at the one point the briefing named as his decision |

## Outcome

Done. All five plan tasks complete, plus two implied tasks (correcting the design doc,
this report) added on take-up. Every objective line in the briefing is met.

Built on `main`:

- `ops/local/thread-binding-guard.sh`: the `Stop` hook backing script. Runs every turn;
  blocks completion with `{"decision": "block", "reason": "..."}` when no thread is
  bound.
- `.claude/hooks/thread-binding-guard.sh`: thin wrapper, same pattern as
  `session-start.sh`.
- `.claude/settings.json`: wires the guard to `hooks.Stop`.
- `ops/local/thread-lib.sh`: added `thread_resolve_binding_at` (the lookup body,
  factored out so it has one copy), `thread_resolve_binding_local` (the same lookup
  without fetching, for a check that runs every turn), and
  `thread_unbound_prompt_text` (the not-found instruction, now shared by
  `claude-session-start.sh` and the new guard instead of living inline in one script).
- `ops/local/claude-session-start.sh`: sources `thread-lib.sh`, uses the shared prompt
  text instead of its own inline string.
- `docs/domain/session-continuation-design.md`: corrected. See T-06 below.

No changes were needed to `/start-thread`, `/pause-thread`, `/resume-thread`, or their
skills: the gap was entirely in what ran between `SessionStart` firings, not in what
those three commands do once invoked.

## Proof

**Fresh session, proven live.** A real cloud session was created against this repo's
`main` (`session_01YEawKwWrouzvBNcPxXxiiV`), seeded with an immediate, unrelated first
message: "What's 17 times 23?" `list_sessions` on that session afterwards showed it
blocked on `AskUserQuestion` with exactly the structured question the objective
requires: "Jim, no thread is bound for this session. Which mission should I work?",
three selectable options, `M-BOOT-02-02` itself correctly inferred and billed first
("Directly closes the gap this session just hit..."). No plain-text answer-only
response was found anywhere in its state; the guard's block was the reason the question
got asked at all. The session made no repository or binding changes (confirmed by
re-fetching `tsk/bootstrap` and finding no lookup entry for its session ID), so nothing
needed cleaning up before it was archived.

**`/clear`, proven by construction, not live.** No tool call lets an agent trigger
`/clear` on itself, the exact limitation M-BOOT-02-01's report recorded for the same
reason. Live proof was not possible from inside a single session. The guard's own
construction closes the gap regardless: `thread_resolve_binding_local` depends on no
conversational state, only on the worktree marker file or the cloud lookup entry keyed
on `CLAUDE_CODE_REMOTE_SESSION_ID`, and `/clear` changes neither of those (a cloud
session's ID survives `/clear` unchanged, and a CLI worktree's marker file is untouched
regardless of session ID). The guard's behaviour is therefore identical before and
after a `/clear`, by the same reasoning M-BOOT-02-01 used to accept that a fresh session
and a `/clear` reach the same result for a mechanism with no conversational state.

**Repeated distraction beyond one turn.** Not separately proven. The objective and the
live test both cover one turn of distraction. Whether the guard still closes the gap
after several turns of unrelated conversation was not tested; the mechanism's own
construction (it re-checks the same fact every turn, forever, until it holds) makes
this very likely to generalise, but "very likely" is not "proven."

## What the briefing failed to give me

**No criterion for what counts as a reproduction (T-01).** The objective says
"confirmed to leave the session unbound," but not confirmed how: by literal repro, by
citing the 2026-09-17/18 incident already on record, or both. I did both: a subagent
simulation (a fresh agent given the hook's real `additionalContext` plus an immediate
unrelated question) that showed the instruction survive but only as a bolted-on
afterthought, not a priority, plus the historical incident on record for the full drop.
Different evidence would have satisfied a differently-worded T-01 equally well; the
briefing left me to decide what "confirmed" required.

**No anticipation that proving T-05 live means proving it on a session I cannot fully
drive.** The objective's proof criterion assumes a tester who can carry a scenario
through to a bound thread. `AskUserQuestion` is answered by a human, by design; I could
create a real cloud session and inspect its state, but not click through its pending
permission approval from inside this session, so the live proof stops at "correctly
asked," not "correctly bound." That stopping point is not a gap in the mechanism. It is
the mechanism working as intended: the point where a human decides. The briefing did
not flag this limit on what a session testing its own resilience mechanism can prove
about itself.

## What I found wrong, and got wrong

None of mine. One thing worth naming rather than fixing: `claude-session-start.sh`
fetches `tsk/bootstrap` once directly, then `thread-resolve-binding.sh` fetches it
again inside `thread_resolve_binding`'s call to `thread_refresh_wt`. Two fetches per
`SessionStart` firing where one would do. Left alone: `SessionStart` fires once per
session, so the redundancy costs one extra network round trip at a point that already
tolerates it, and touching `thread-resolve-binding.sh`'s freshness contract was not
implicated by this mission's own gap.

## For M-BOOT-02 and what follows

- T-14's delegation is closed. The gap it named, a session left unbound for its whole
  duration once distracted, cannot recur under the new guard: the check that used to
  run once now runs every turn until it holds.
- **T-03 and T-04 turned out to be one implementation unit, not two.** The briefing
  split "build the structured prompt" from "implement the persistence mechanism," but
  the prompt text is a single shared function (`thread_unbound_prompt_text`) consumed
  by both the existing `SessionStart` hook and the new `Stop` hook guard; there was no
  seam to build one without the other. Worth noting for how finely a future briefing's
  plan can divide work that a `grep` for the actual call sites would show is one piece.
- **T-04, the thread state format, is still open.** Untouched by this mission, as
  M-BOOT-02-01 also left it: a continuation state entry carries one what's-next line;
  which tasks are done and which is in progress still has no home. (This is the
  M-BOOT-02 plan's own T-04, distinct from this mission's own T-04 above; the two
  ledgers share a task number by coincidence, not by relation.)
- **Deferred points in the design doc stay deferred**, as the briefing instructed: the
  thread-to-Plan relationship, what two actors on one thread should actually do, and
  whether "Continuation" earns a place in the ubiquitous language on its own. None of
  this mission's findings touched them.
- **The double-fetch on `SessionStart`** noted above is a small, real inefficiency, not
  a defect. Worth folding into whatever mission next touches `claude-session-start.sh`
  or `thread-resolve-binding.sh`, not worth a mission of its own.
