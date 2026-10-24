# Intel: compaction and long-running sessions, October 2026

Date: 2026-10-24. Raised by Jim, who sees compaction work better than before and asked for
the latest. Linked to M-BOOT-02-01 (Continuation harness). It extends the section "Long-running
agents managing their own context" in `missions/operational/M-BOOT-02/intel-index.md`
(2026-09-16) and `docs/kb/agent-context-self-regulation-and-unattended-handoff.md` in the
tsk repo.

Evidence classes: **official** (Anthropic documentation, fetched and read on 2026-10-24),
**changelog** (the Claude Code changelog, newest entries to 2.1.310, 2026-10-23), **paper**
(an arXiv abstract), **search** (a search result summary, not the page) and **general**
(not read for this note). Nothing here was run or measured.

## Summary

- Compaction is now a first-party feature on three levels: Claude Code compacts for the
  user, the API compacts on request or at a token threshold, and the API can clear old tool
  results and thinking by rule. None needs code in tsk.
- Three changes make long sessions cheaper and rarer to compact: 1M-token windows are the
  default on current models, compaction can run in the background, and Claude Code compacts
  an idle conversation before its prompt cache expires.
- Anthropic's own harness article reports that newer models need less than older ones did.
  It dropped context resets when Opus 4.5 largely removed context anxiety.
- The pattern for work across sessions is unchanged and now documented in two places: a
  memory store or progress file outside the window, read first in each new session. That is
  the role of tsk's ledger and continuation entry.
- tsk already re-primes after a compaction. It has no hook for the moment before one.

## What changed since 2026-09-16

| Date | Item | Detail | Class |
|---|---|---|---|
| 2026-03-24 | Anthropic harness article | "Harness design for long-running application development", Prithvi Rajasekaran. Compaction "summarizes earlier conversation in place", so "the agent keeps continuity but not a clean slate, so context anxiety can persist". Sonnet 4.5 showed it strongly enough that resets were needed. With Opus 4.6 the author dropped resets "because Opus 4.5 had largely removed context anxiety on its own" | official |
| 2026-05-09 | Slipstream (arXiv 2605.08580) | Runs compaction asynchronously, beside the agent's continued work on the original context, and checks the candidate summary against what the agent does next. Up to 8.8 points better task accuracy and 39.7% lower end-to-end latency, on SWE-bench Verified and BrowseComp | paper |
| 2026-09-04 | API: on-demand compaction | Beta header `compact-2026-09-04` and a `compaction` parameter. "Your application determines when": one request returns a signed `compaction` block in place of a reply. Options: keep recent turns word for word, run in the background while the conversation continues, replace the summary prompt (up to 16,384 characters), compact again | official |
| 2026-09-22 | CliffCompaction (arXiv 2609.26779) | Drops or truncates content and never rewrites it. Never compacts a compaction: each pass works on original content. Up to 50% lower cost under a bounded context. Open-source API proxy for Claude Code, Codex and others | paper |
| 2026-10-14 | Claude Code 2.1.299 | Idle compaction: a conversation of 200K or more tokens, idle about 54 minutes, is compacted before its one-hour prompt cache expires. `CLAUDE_CODE_IDLE_COMPACT_MIN_TOKENS` sets the minimum. 2.1.302 added a `/config` row. The caching page says the `idleCompaction` setting needs 2.1.290 or later, so the version differs between two official pages | changelog, official |
| 2026-10-15 | Claude Code 2.1.301 | Switching to a model with a smaller window: a conversation too long for the new model is summarized by the previous model first | changelog |
| 2026-10-19 | Claude Code 2.1.304 | Compaction sends the skill list again afterwards. Resume of large compacted sessions in SDK and headless mode sends the first request sooner | changelog |
| 2026-10-20 | Claude Code 2.1.305 | Fix: cloud sessions and teammates now tell the model when a compaction had to leave the oldest messages out of its summary | changelog |
| 2026-10-14 to 2026-10-23 | Claude Code 2.1.300 to 2.1.310 | Fixes to resume after compaction in headless and SDK sessions (summary and kept messages lost), repeated compaction after reading a large nested CLAUDE.md, `/autocompact` reverting after a cloud session restart, and compaction after a "Prompt is too long" error keeping too much | changelog |

## Mechanics in the documentation

**Windows.** Fable 5.1, Mythos 5.1, Opus 4.6 and later, Sonnet 5.5, Haiku 5.5 and others have
a 1M-token window, the default, with no beta header (official, context windows page). In
Claude Code, models with a native 1M window compact "at about 967K tokens by default". Opus 4.6
and Sonnet 4.6 without extended context compact at 200K. `/autocompact 500k` or
`CLAUDE_CODE_AUTO_COMPACT_WINDOW` moves the point, between 100K and 1M (official, model
configuration). The API's page says "response quality degrades as a conversation grows", and
names the effect context rot.

**What survives a compaction in Claude Code** (official, context window page):

| Kept | Reloaded or re-run | Lost to the summary |
|---|---|---|
| System prompt and output style | Project-root CLAUDE.md, unscoped rules and auto memory, re-read from disk | Path-scoped rules and nested CLAUDE.md that loaded into the history |
| Background commands and subagents (still running, with a reminder to the model) | Skill list (2.1.304), up to five recently read or edited files, invoked skill bodies (5,000 tokens each, 25,000 in total) | Context that hooks added earlier |
| | **`SessionStart` hooks that match the `compact` source: Claude Code runs them and adds their output** | Images and fetched pages in the summarized range |

As of v2.1.198 the summarization request inherits the session's thinking configuration.

**Cost.** Compaction invalidates the conversation layer of the prompt cache. A mid-session
`/compact` reads its prefix from a warm cache. After a gap longer than the cache lifetime it
"reprocesses that history as uncached input", so `/compact` costs the most when an old session is
resumed (official, prompt caching page). The cache lasts one hour for the main conversation on a
subscription within plan usage and five minutes otherwise. Idle compaction is the response.

**API options.** Server-side compaction is "the primary strategy" for long conversations
(official). On-demand compaction leaves the timing to the application and returns a signed
block that must be sent back unchanged, first in `messages`, exactly one per request. Threshold
compaction (`compact_20260112`) runs inside the request that reaches a trigger; the default of
150K and minimum of 50K come from a search result and were not read on the page. Context
editing clears old tool results (`clear_tool_uses_20250919`, default trigger 100,000 input
tokens, keep the last 3) or thinking blocks (`clear_thinking_20251015`) instead of summarizing.
The two kinds of compaction cannot be combined with `context_management` on one request.

**Telling the model how much is left.** Sonnet 5 and older Sonnet and Haiku 4.x models get
injected `token_budget` and `system_warning` tags. Opus 4.7 and later, Sonnet 5.5, Fable 5.1,
Mythos and Haiku 5.5 do not; they take a task budget, a beta advisory countdown for a whole
agentic loop (minimum 20,000 tokens), "visible only to the model". Task budgets are "not
supported on Claude Code or Cowork surfaces" (official). `/goal` in Claude Code sets a
completion condition that a small model checks after each turn, and a goal is cleared by "a
context overflow that auto-compaction couldn't clear".

**Memory.** The memory tool (`memory_20250818`) is client-side: the model asks, the
application stores files under `/memories`. Anthropic's advice for long-running agents is to use
both, because "compaction keeps the active context small... and memory preserves the
information that must survive summarization". Its multisession pattern: an initializer session
writes a progress log, a feature checklist and a startup script. Each later session reads them
first and updates the log before it ends. Mark a feature complete only after end-to-end
verification (official).

## Other implementations

- **OpenAI Codex.** For OpenAI models the client calls a server-side compaction endpoint that
  returns an encrypted blob the client passes back unchanged. For other providers it asks the
  model for a plain-text handoff summary. Reported reason for the encryption: no client edits to
  the summary (search, third-party write-ups, unverified).
- **Pi Durable.** Compaction is a background task. It starts a set number of tokens before the
  limit and places the summary at the next turn boundary. "The older messages always stay in
  storage" (`docs/kb/orchestration-ecosystem/pi-durable.md`).

## Why it seems more efficient

Jim's observation fits several documented changes. None isolates a cause, and no measurement
was made for this note.

1. The window is five times larger by default, so compaction is rarer. Claude Code compacts at
   about 967K on current models, not 200K.
2. The model is told what to do with less. The article on harness design says resets and
   sprint structure became unnecessary as models improved, and that each harness component
   "reflects an assumption about model limits".
3. The runtime handles more. Compaction can run in the background, an idle conversation is
   compacted while the cache is warm, and a failed or partial compaction is now reported to the
   model (2.1.305).
4. Cost is down. CliffCompaction reports up to 50% lower cost, and Slipstream up to 39.7%
   lower latency. These are the papers' own figures on their benchmarks, not Claude Code's.

## What this means for tsk's continuation

1. **A continuation entry sits outside every compaction mechanism above.** Compaction summarizes
   a session. A thread's continuation state is a record in the ledger that any later actor reads.
   The Anthropic memory pattern (progress file, feature list, read first, update last) is the same
   idea held in a store tied to one application. tsk's version is tied to no harness.
2. **tsk already re-primes after a compaction.** `SessionStart` hooks that match the `compact`
   source are run after a compaction, and their output is added to the compacted context. The
   binary has a claim for the source `compact` (`cli/src/thread/session_start.rs`, tests with
   `Some("compact")`). So the thread binding prompt returns after a compaction.
3. **The moment before a compaction has no hook in the tsk plugin.** The plugin registers
   `SessionStart`, `Stop` and `SessionEnd` only. A `PreCompact` hook is the deterministic point at
   which an actor could write a continuation entry, the open item of the 2026-09-16 notes. This
   note did not re-read the hook's behaviour in the current hooks reference.
4. **Compaction can now leave the oldest messages out and tell the model so.** A continuation
   entry written before a long run is therefore more reliable than the start of the conversation.
5. **Resume cost differs by path.** `claude --resume` of an old session reprocesses the history
   uncached and, if it is compacted, costs the most. `tsk thread resume` reads a short entry. This
   is an argument for a continuation entry over a resumed transcript for long gaps.
6. **Consistent with the 2026-10-16 position.** The mechanics (summaries, background compaction,
   idle compaction, memory stores) are moving into the harness and the API. What the harness does
   not hold is the model of the work: mission, objective, who holds the thread, and what a later
   actor on another machine reads first.

## Changes to the 2026-09-16 notes

- "Auto-compaction... is not something an agent decides": still true in Claude Code. The API
  now lets an application decide when (on-demand compaction), which was not in those notes.
- The context awareness wording in the earlier note applies to some models only. Current
  models take a task budget instead, and task budgets are not supported in Claude Code.
- The note called the compaction beta one thing. There are now two kinds, with different
  headers, and an SDK-side `compaction_control` that is deprecated.

## Not established

- Whether any measurement shows Claude Code compacting better than before. The sources show
  changes and fixes, not a result.
- What Claude Code's compaction summary keeps and drops, beyond the survival table. The
  summary text is not documented.
- The threshold compaction page, `compact_20260112`, was not read. Its defaults come from a
  search result.
- Whether the tsk plugin's `SessionStart` hook for the `compact` source works as designed in a
  cloud session. The code has a test. No cloud run was made.
- Whether the Codex details (encrypted blob, memory reuse) hold. They come from third-party
  write-ups.
- The Claude Code changelog was read to 2.1.299 and the newest entries. Older entries were not.

## Sources

- Anthropic API docs: [Compaction overview](https://platform.claude.com/docs/en/build-with-claude/compaction),
  [Compaction on demand](https://platform.claude.com/docs/en/build-with-claude/compaction-on-demand),
  [Compaction in the background](https://platform.claude.com/docs/en/build-with-claude/compaction-background),
  [Compaction that keeps recent turns](https://platform.claude.com/docs/en/build-with-claude/compaction-keep-recent-turns),
  [Context windows](https://platform.claude.com/docs/en/build-with-claude/context-windows),
  [Context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing),
  [Task budgets](https://platform.claude.com/docs/en/build-with-claude/task-budgets),
  [Memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool).
- Claude Code docs: [Explore the context window](https://code.claude.com/docs/en/context-window.md),
  [Model configuration](https://code.claude.com/docs/en/model-config.md),
  [How Claude Code uses prompt caching](https://code.claude.com/docs/en/prompt-caching.md),
  [Keep Claude working toward a goal](https://code.claude.com/docs/en/goal.md),
  [Changelog](https://code.claude.com/docs/en/changelog).
- Anthropic engineering: [Harness design for long-running application development](https://www.anthropic.com/engineering/harness-design-long-running-apps), 2026-03-24.
- Papers: [Slipstream, arXiv 2605.08580](https://arxiv.org/abs/2605.08580),
  [CliffCompaction, arXiv 2609.26779](https://arxiv.org/abs/2609.26779).
- tsk repo: `cli/src/thread/session_start.rs`, `plugin/hooks/hooks.json`,
  `docs/kb/orchestration-ecosystem/pi-durable.md`.
