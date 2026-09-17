# Agent context self-regulation and unattended handoff

Gathered by a Fable-model subagent's research, verified against primary Anthropic
documentation, 2026-09-16. Two of the load-bearing claims below — the `/goal` mechanics
and the exact wording of the context-awareness warning — were independently re-checked
against `code.claude.com` and `platform.claude.com` directly and match precisely.
`get_session`'s behaviour was independently confirmed by direct observation of a running
session, against actual output rather than documentation, since no public page describes
it. See
`docs/kb/session-creation-and-environments.md` for the session-orchestration mechanics
this document builds on.

## Operating contexts

Four contexts frame this whole document. Everything below — mechanisms, `/goal`, the
handoff pattern — applies differently depending on which one a session is in.

1. **Supervised interactive.** A human is present. `/clear` is available only here —
   there is no tool call for it, which is why it doesn't belong to context 2 — but what
   it does to session identity depends on the surface, and this splits the context in
   two:
   - **Cloud session** (Claude Code on the web, the apps, `claude --cloud`). `/clear`
     leaves the session's `id` field unchanged. What changes instead is
     `external_metadata.turn_handoff.worker_epoch`, an undocumented counter nested
     alongside `tools` and a version `v` inside a `turn_handoff` object. `worker_epoch`
     is scoped to the session, not the environment: two unrelated sessions in the same
     environment reported different values. It also increments on at least one other
     event besides `/clear` — a mid-session model switch was observed to move it, with
     the session ID unchanged. No restart mechanism has yet been found that changes a
     cloud session's own ID.

     It moves for more than those two events. One supervised session read it at 3, and
     at 16 later the same day, across four model switches and no `/clear`. Leaving a
     session and returning to it reloads the environment, which accounts for the rest,
     and makes the counter climb faster under supervision than under an unattended run
     that holds one turn until it reaches a limit.

     The consequential part is that an increment can fall inside a turn. The resumed
     turn then carries the conversation but not the tool calls made after the last
     persisted point, while those calls' side effects persist on disk and on any remote
     they reached. An agent in that position has acted, holds no record of acting, and
     can establish what happened only from external state: `git reflog`, the branch, the
     remote. Two consequences for an unattended design. A run record held in the
     transcript is not durable against this. And any script that writes shared state has
     to be safe to run twice, because a restart makes a second run ordinary rather than
     exceptional; `ops/local/push-bootstrap-ref.sh` was made idempotent for this reason.

     The cloud session ID is also readable directly, with no MCP tool call, as the
     environment variable `CLAUDE_CODE_REMOTE_SESSION_ID` (`cse_...`; swap the prefix
     for `session_` to match `get_session`'s `id` field exactly — confirmed
     byte-identical). This means a plain bash `SessionStart` hook can resolve a cloud
     session's identity itself, the same way it already can a CLI worktree's.

     There are two different session identifiers in a cloud session's environment, and
     they must not be conflated. `CLAUDE_CODE_REMOTE_SESSION_ID` is the CCR platform
     session ID — the one everything above refers to as "the cloud session ID," and the
     one to use for a thread binding. `CLAUDE_CODE_SESSION_ID` is a separate UUID
     (confirmed distinct from the above: it matches this session's own scratchpad
     directory path, so it looks like a lower-level container or instance identifier,
     not the CCR session). Any design that binds a thread to "the session ID" needs to
     say `CLAUDE_CODE_REMOTE_SESSION_ID` explicitly.
   - **CLI.** `/clear` produces a new session ID directly. There is no equivalent of
     `worker_epoch` to fall back on here; a worktree is the thing that persists across
     it instead.
   - Not yet explored: whether `external_metadata.permission_mode_seq` (also seen
     incrementing) tracks the same thing as `worker_epoch` or something independent,
     and whether anything reads `worker_epoch` back to resume state rather than only
     reporting it.

   Neither a session ID nor a worktree is thread identity; each is only what a given
   sub-context happens to keep durable. Resolution: tsk's own thread concept (Thread and
   Actor, `docs/domain/ubiquitous-language.md`) is the actual anchor — minted once when
   the thread starts, associated with an actor, and bound to whichever of these a
   sub-context offers, rather than being either of them. Getting this binding right is
   the one most exposed by ordinary use.
2. **Unsupervised autonomous.** No human present. The agent must both regulate its own
   context and decide when and how to continue, spawning its own successor until the
   mission is done. This is the context the `/goal` + `get_session` + `create_session`
   pattern below targets directly.
3. **Orchestrator spawning workers.** A level above (2): the orchestrator is itself a
   session, and it eventually hits the same context limit its workers do. Continuation
   has to apply recursively, not only to the leaves.
4. **Event-triggered.** A GitHub Action, a PR event, an issue to process. No session
   lineage going in at all; each firing starts genuinely fresh, by construction rather
   than by choice.

## Summary

An earlier hypothesis held that no mechanism lets an agent manage its own context, other
than platform-triggered auto-compaction, and that external work decomposition is the
only real strategy. The first half doesn't hold. Four mechanisms exist, in ascending
order of agent control:

1. **Automatic compaction.** Platform-triggered, no agent input.
2. **Context awareness.** The platform injects remaining capacity after every tool call;
   the agent paces against it.
3. **Agent-directed persistence.** The memory tool (API) and auto memory (Claude Code);
   the agent selects what to write.
4. **On-demand session inspection.** A cloud session reads its own token usage through
   an MCP tool. Unconfirmed in public documentation, observed directly.

Each level is a degree of agent control, not a single tool. More than one product can
sit at the same level — level 1 has two, level 3 has two — and a level's number says
how much say the agent has over what happens there, not how many implementations exist.
The sections below are ordered and numbered to match.

## 1. Automatic compaction (confirmed)

Platform-triggered, with no agent input — but not one tool. Two distinct products sit
at this level.

**Claude Code auto-compaction.** Triggers automatically as the context window fills,
cannot be disabled, and is not something an agent decides to do itself.
`CLAUDE_CODE_AUTO_COMPACT_WINDOW` overrides the trigger threshold; `/compact` remains
available for an on-demand pass with optional focus instructions. See
`docs/kb/session-creation-and-environments.md` for where this sits among a session's
other configuration.
Source: [Claude Code compaction docs](https://platform.claude.com/docs/en/build-with-claude/compaction), primary.

**API server-side compaction.** A separate surface from Claude Code's own behaviour
above: the Claude API automatically summarises earlier parts of a conversation on the
server once it approaches the context limit, so the conversation can continue past it.
In beta, for Claude 4.6 and later models.
Source: [Context windows](https://platform.claude.com/docs/en/build-with-claude/context-windows), primary, confirmed directly 2026-09-16.

Neither tool gives the agent a say in when compaction happens or what it preserves —
that is what distinguishes this level, the least agent control, from the three that
follow.

## 2. Context awareness (confirmed)

Claude Sonnet 5, Sonnet 4.6, Sonnet 4.5, and Haiku 4.5 track their remaining context
window automatically; there is nothing to enable. The system prompt of every request
carries the total budget, and after each tool call the API injects an update:

```
<system_warning>Token usage: 35000/200000; 165000 remaining</system_warning>
```

Source: [Context windows](https://platform.claude.com/docs/en/build-with-claude/context-windows), primary, confirmed verbatim 2026-09-16.

Documented purpose is pacing a task against remaining space, not checkpointing. Opus
models (4.7 and later), and the Fable and Mythos model families, do not receive these
injected tags; task budgets exist as an explicit alternative for those.

## 3. Agent-directed persistence (confirmed)

Again more than one tool at this level: two distinct stores, each with its own scope
and mechanics.

**API memory tool (`memory_20250818`).** A file-based store outside the context window,
with create, read, update and delete on a `/memories` directory. The backend is
client-side, implemented by whoever builds the agent. The model selects what and when to
save, as part of its own tool-use loop. Paired with context editing, the model receives
a warning as context approaches a configured clearing threshold, so it can write to
memory before clearing happens.
Sources: [memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool) and [context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing), primary.

**Claude Code auto memory.** Notes Claude writes for itself, at
`~/.claude/projects/<project>/memory/`, with a `MEMORY.md` index; the first 200 lines or
25KB load at session start. Selection is based on whether the information helps a future
conversation — user preferences, corrections, ongoing decisions, references — and skips
anything derivable from the codebase. It is a cross-session learnings store, not a
mid-task checkpoint. Critical for cloud work: auto memory is machine-local, not shared
across machines or cloud environments. A cloud session starts from a fresh clone and
does not read `~/.claude/` from anywhere; the only durable store for an unattended cloud
session is the repository itself.
Source: [Claude Code memory](https://code.claude.com/docs/en/memory), primary.

**Hooks are not this.** `PreCompact` and `PostCompact` fire at fixed lifecycle points,
not at the model's discretion, and are harness-side scripts rather than agent-initiated
persistence. Hooks do not receive context metrics.
Source: [Claude Code glossary](https://code.claude.com/docs/en/glossary), primary.

## 4. On-demand session inspection (unconfirmed in docs, observed live)

`get_session`, part of an MCP server named `Claude_Code_Remote`, alongside
`create_session`, `list_sessions` and `create_trigger`. Called with no arguments, it
describes the calling session. Relevant response path:

```
external_metadata.context_usage.max_tokens
external_metadata.context_usage.used_tokens
```

`external_metadata.usage` also carries input, output, cache read, cache write tokens and
`cost_usd`. No public page lists these tool names, and nothing indexes
`context_usage` — this is undocumented product surface with no compatibility promise.

Significance: combined with `create_session`, a single cloud session holds both halves
of self-directed handoff. It reads its own usage and starts a successor. No hook, no
injected warning, and no external decomposer is required for this specific capability
to exist — though see the open items below on how reliable a foundation it is.

## Verification loops and `/goal`

A verification loop is three things: a command returning pass or fail, an instruction to
run it and continue until it passes, and a decision point outside the model doing the
work.
Sources: [`/goal`](https://code.claude.com/docs/en/goal) and ["Give Claude a way to verify its work"](https://code.claude.com/docs/en/best-practices), primary, `/goal` confirmed directly 2026-09-16.

Mechanics, confirmed against the primary doc:

- One goal per session; setting it starts a turn immediately, with the condition as the
  directive.
- After each turn, the condition plus the conversation go to the configured small fast
  model (Haiku by default on the Claude API). It returns not-yet-met, met, or
  impossible, each with a reason.
- The evaluator runs no tools and reads no files — it assesses only what the transcript
  already contains — and holds no state between turns.
- Condition limit: 4,000 characters. No built-in token budget; a turn or time clause has
  to go in the condition itself.
- `/goal` doesn't change permission mode; pair it with auto mode for unattended turns.
- Requires workspace trust and hooks enabled, since the evaluator is part of the hooks
  system.
- Non-interactive form: `claude -p "/goal <condition>"`, runs the loop to completion in
  one invocation.
- On resume, the condition carries over; turn count, timer and token baseline reset.
- Documented surfaces: non-interactive mode, the desktop app, and Remote Control. Cloud
  sessions are not in that specific list, though the doc separately treats "a cloud
  session" as one of the hosts that manage their own credentials, alongside the desktop
  app — availability there is plausible but not confirmed as a first-class documented
  surface. Test directly before relying on it.

Related: `/loop` re-runs on a time interval rather than toward a condition; a Stop hook
lives in settings, applies to every session in scope, and can run a deterministic
script instead of a model judgement; auto mode approves tool calls within a turn but
starts no new turns on its own.

## The unattended handoff pattern

**Problem.** When a goal clears, control returns to the user. In a cloud session or a
routine, no user is present, so the handoff has to sit inside the loop, not after it.

**Construction.** Write the stop condition as a disjunction, with the context branch as
a conjunction of threshold and artefact:

```
/goal Either (a) every task in tasks.json has status "done",
or (b) reported context usage exceeds 800000 tokens.
In case (b) the condition is met only once HANDOFF.md
exists with the remaining tasks and the last commit, and
is committed and pushed.
Each turn, call get_session and print used_tokens and
max_tokens before doing any other work.
Stop after 40 turns.
```

**Sequence.** Each turn prints usage, then works a task: no, while tasks remain and
usage is under the ceiling. The turn where usage crosses the ceiling: branch (b)'s
threshold is met but not its artefact clause, so the verdict is no, with a reason
naming the absent `HANDOFF.md`. That reason feeds back as guidance for the next turn,
which writes `HANDOFF.md`, commits, pushes, prints the commit SHA, and calls
`create_session` pointing at the branch and the handoff file. Verdict: yes. The goal
clears with state on disk and a successor already running.

**Why the conjunction matters.** The two apparent phases are a property of the
condition, not of the evaluator: one test yields a different verdict on consecutive
turns purely because the transcript changed between them. The refusal itself produces
the handoff; nothing needs to be remembered and acted on later. Name the missing
artefact in the condition so the evaluator's reason names it too — a bare "condition
not met" leaves the next action underdetermined. `create_session` has to sit inside the
condition, so the spawn happens before the yes; placed after, it never runs, because
control has already returned.

**Requirement on the successor.** A cloud session starts from a fresh clone; the prompt
alone carries nothing forward. The handoff has to be in the repository.

## Cross-reference: the research literature

Anthropic's own published pattern for long-running agents is external decomposition: an
initializer sets up a `feature_list.json` with 200+ granular features before any coding
agent runs, and each session commits progress to git and a `claude-progress.txt` log.
Source: "Effective harnesses for long-running agents", Justin Young, Anthropic
Engineering, 26 November 2025 — read via search-aggregated secondary sources, since
`anthropic.com` is blocked by this session's egress proxy.

Earlier Anthropic guidance, "Effective context engineering for AI agents", lists three
long-horizon techniques: compaction, structured note-taking (writing to memory outside
the context window for later retrieval), and sub-agent architectures. Structured
note-taking is agent-directed persistence as stated guidance, not just an emergent
pattern.

Pan et al. formalise the file-backed state pattern with three properties: externalized,
path-addressable, compaction-stable. Anthropic's `claude-progress.txt`, `feature_list.json`
and git log satisfy all three; the Anthropic post predates the paper and is cited by it.
Pan's contribution is a middle position between "the agent selects" and "the harness
imposes": the harness specifies in natural language when compression is allowed, what
state must be externalised first, and what fields the compressed state must preserve,
and the agent performs the write. The handoff pattern above is an instance of that
position. Not independently verified here beyond the citation itself:
Linyue Pan, Lexiao Zou, Shuo Guo, Jingchen Ni, Hai-Tao Zheng, "Natural-Language Agent
Harnesses", 2026, [arXiv:2603.25723](https://arxiv.org/abs/2603.25723).

MemGPT's tiered-memory pattern, where the agent calls functions to move data between
tiers, remains a research pattern only — not confirmed inside Claude Code or the Claude
Agent SDK.

## Open items and weak joints

1. `/goal` availability in cloud sessions and in routine-started sessions is unverified.
2. `Claude_Code_Remote` availability in a routine-started session is unverified; one
   report states cloud sessions receive only the first-party GitHub MCP.
3. `used_tokens` comes from session metadata, not the model's own view. Confirm it
   agrees with the injected context-awareness line in the same turn before using it for
   thresholds.
4. Threshold assessment by a small model reading text is the weakest part of the design.
   The deterministic alternative is a Stop hook script that calls the session API
   itself and returns block or allow.
5. No documented route exists for an agent to trigger compaction on demand. A successor
   session is the only restart path found.
6. Compaction survival: project-root `CLAUDE.md` and auto memory reload from disk after
   compaction. Instructions given only in conversation can be lost. Anything that must
   survive belongs in a file.
