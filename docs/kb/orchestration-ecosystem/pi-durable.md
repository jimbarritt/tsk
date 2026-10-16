# Pi and Pi Durable

Status: open source, MIT licence, from Earendil. Pi 1.0 and Pi Durable were released on
2026-10-01. Pi Durable is experimental: "The API changes without notice between
releases." Every package in the repository was at version 1.3.0 on 2026-10-15. Read
2026-10-16.

Raised from the name "durable pi", which overlaps tsk's thread continuation.

Sources: Earendil's posts
[Pi Durable](https://earendil.com/posts/pi-durable/) and
[Pi 1.0](https://earendil.com/posts/pi-1-0/), both dated 2026-10-01, and a clone of
[earendil-works/pi](https://github.com/earendil-works/pi) at `4954d76` (2026-10-15).
Files read in the clone: `packages/durable/README.md`, `packages/durable/docs/spec.md`
(5,352 lines: the terms and invariants, the effect sandwich and the non-goals),
`packages/coding-agent/docs/session-format.md` and `packages/chord/README.md`. The
community repository [disler/durable-pi](https://github.com/disler/durable-pi) at
`37165c6` (2026-10-11) was read for its README. Nothing was installed or run. Claims are
Earendil's own.

## What they are

- **Pi** is a terminal coding agent. Earendil: "Pi the coding agent is built to run on
  your (remote) machine, inside a terminal, driven by one person. If the process dies, you
  look at what happened and tell it to continue." Earendil says "Hundreds of thousands of
  people around the world use Pi every week."
- **Pi Durable** is a separate package, "a framework for building any agentic
  application, coding agents included". It shares code with Pi, such as `pi-ai`, and the
  principles "minimalism and malleability". Earendil's stated aims: a harness that "runs
  anywhere, can be reached from different surfaces, supports infinitely long
  conversations, survives catastrophic internal and external failures, and lets multiple
  humans steer the same agents".
- **Earendil** says it "was formed to craft software and open protocols that strengthen
  human agency". The Pi 1.0 post says Mario Zechner, Pi's author, writes about the
  principles in more detail in a separate post. That post was not read.

Packages in the repository, in lines of TypeScript source: `coding-agent` 76,002, `ai`
27,070, `durable` 24,989, `tui` 19,540, `chord` 13,553, `mcp` 3,277, `codemode` 3,037,
`agent` 2,519 and `env` 2,346. The Pi Durable post puts its source at "about 15,000
lines" without tests. The clone has 24,989 across 82 files at version 1.3.0.

## Model

| Concept | Meaning |
|---|---|
| Harness | "storage plus the machinery needed to run one or more conversations with large language models in parallel". It provides the tools the models call and the execution environments the tools run in |
| Conversation | A transcript of immutable entries. It starts fresh or forks another at any entry, and sees the parent's history up to that point without copying it |
| Entry | One immutable transcript record: user message, model response, tool result, system prompt change, reset, or an application's own kind |
| Commit | An atomic write: entries, documents and tasks together, or none |
| Document | Typed JSON state stored beside the transcript and changed in the same commits, so "the state never disagrees with the transcript that produced it" |
| Task | "a durable state machine that saves a checkpoint at every step". Each model request, tool call and compaction is one |
| Submission | Input handed to a conversation. A `requestId` makes it exactly-once |
| Extension | A named bundle of system prompt sections, tools, hooks and tasks, installed in a registry |
| Agent | What a conversation runs with: model, thinking level, selected extensions and tools, instructions and working directory. Stored per conversation, by name |

The spec's core rule: "A Session atomically commits immutable entries, full task records,
and Chord-tracked documents. Only committed state is observable." Chord is a companion
package for observable, hot-reloadable object state across processes.

## Crash recovery

- Every step stores a checkpoint before it moves on. A new process opens the same storage,
  finds unfinished tasks and continues each from its last checkpoint: `harness.resume()`.
- The spec calls the pattern an effect sandwich: commit the intent, perform the external
  effect, commit the outcome. "Reopening in an intent phase means the effect may have
  happened."
- A model request cut off by a crash is sent again, and the partial answer stays in the
  transcript, marked aborted.
- A tool call cut off by a crash reruns only if the tool declares `replay: "safe"`.
  Otherwise "the model is told it was interrupted, with the output stored so far".
- An approval hook stores its decision in a memo, a small first-writer-wins value stored
  with the task, so a restart finds the answer instead of asking again.

## Handoff, compaction and history

- `reset(note)` "starts a new context". The model no longer sees older entries, "but they
  stay in storage". A tool can request the same with `control: { handoff }`. A second
  tool can search everything before the handoff.
- Compaction is a background task. A summary starts a set number of tokens before the
  context limit and is placed at the next turn boundary. "The older messages always stay
  in storage."
- A document declares its history as `latest` or `rewindable`, and what a fork starts
  with: `initial`, `current` or `asOf` the fork point.

## Conversations and ownership

- One harness runs many conversations at once. Earendil's example: a Slack channel is one
  conversation, and a thread is a fork of it at the message it replies to.
- Tasks and conversations form one ownership tree. "Aborting a task aborts what it owns,
  bottom-up." A subagent is a conversation owned by the tool call that started it, so it
  also resumes after a crash. Pi Durable has no built-in subagents.
- A background task belongs to a conversation but not to its current work, so an ordinary
  abort leaves it alone.

## Storage

| Backend | Notes |
|---|---|
| Memory | Not persisted |
| SQLite | One file. WAL mode with `synchronous = NORMAL`: commits survive a process crash, and the newest may be lost on power or host failure |
| JSONL | Append-only files in one directory, with an optional `fsync`. Keeps the whole history in memory too |
| Cloudflare Durable Object | The Durable Object's own SQLite storage |

The interface is "small and easy to implement on top of whatever you have, like a
key-value store or Postgres", with "a conformance suite and benchmarks for your own
backend". "One process owns a storage at a time; there is no cross-process locking."
Other clients attach to that process. The spec lists these as non-goals: "CRDT/offline
multi-writer merge", "JSONL global compaction or automatic corruption repair".

## Surfaces and steering

- Everything a user interface needs is committed state. Any number of clients attach to a
  conversation, get "the current view first" and then only changes, and join late or
  reconnect from the current view.
- Any client steers a running conversation or queues a follow-up: `whenBusy: "steer"`.
- Extensions are replaceable while conversations run. "Conversations store extension and
  tool names, never code", so a restart picks up the new code.

## Pi 1.0

Added to Pi on 2026-10-01: Codemode (native MCP, and non-LLM models "like Jev and image
models"), extension support for virtual models, deferred tool loading, cache warming for
Anthropic models, and mid-conversation system messages. The demo builds a virtual model
that "plans with Claude Opus and implements with GPT, with Jev deciding when to switch".
That is a use of the classifier described in `typesafe-jev-classifier.md`.

## Community use

[disler/durable-pi](https://github.com/disler/durable-pi), by IndyDevDan, is "a durable
agent network": durable Pi agents on several machines, each checkpointing to SQLite, and
reached from a web UI, a terminal client or `curl`. Its demo is a proactive agent that
watches a production log stream and has a fix ready for review before the person arrives.
It uses OpenRouter for models, `just` for tasks, and `ssh` to reach remote devices.

## Terms that differ from tsk's

| Term | In Pi Durable | In tsk |
|---|---|---|
| Harness | The runtime: storage plus the machinery that runs conversations | The tooling a person runs agent sessions in, under ADR 0013 |
| Task | A durable state machine that runs one step of work, such as a tool call | A planned unit of work with an objective, in a mission's plan |
| Session | One open storage with one mutation line | A platform-level execution instance, ephemeral |
| Conversation | A transcript, with forks | A thread holds work. The transcript belongs to the harness |

## Comparison with tsk

| tsk term | Pi Durable |
|---|---|
| Thread continuation | A transcript, task checkpoints and `reset(note)`. They resume one run after a process restart, in one process that owns the storage. A tsk entry is a handover that another actor, on another machine, reads |
| Ledger | A Session's storage: SQLite, JSONL or a Durable Object, owned by one process. tsk's ledger is a Dolt database synced over git, with a compare and swap across clones |
| Storage seam | A small storage interface with memory, SQLite, JSONL and Cloudflare backends, a conformance suite and benchmarks. ADR 0014 names the same shape: one trait, two backends, one conformance suite |
| Append-only record | Immutable entries, and documents with `rewindable` history |
| Idempotency | `requestId` makes a submission exactly-once. `tsk thread session-start` claims an event by session ID and source for the same reason |
| Mission, objective | None. A submission ends when the model answers |
| Actor | None. A conversation names an owner for abort purposes. Any client may steer it |
| Delegation | A subagent is a conversation owned by a tool call. No briefing and no report |
| Approval | A `beforeTool` hook blocks a call and stores the decision in a memo. tsk's Post and escalation are not built |
| Execution constraints | Usage and cost are counted per conversation. No limit enforcement appears in the pages read |
| Territory, nexus | None |
| Product, Delta, Scale | None |

## Convergence and difference

Both reached the same mechanisms without reference to each other, as far as the sources
show:

- An immutable, append-only record, with state that is versioned beside it.
- A handoff note that starts a new context while the old one stays searchable.
- Forks of a record at a chosen point.
- A storage interface with several backends and a conformance suite.
- An idempotency key so a retry returns the original result.
- More than one client steering the same work.

They differ on what they hold continuous and where. Pi Durable holds one run steady
across process restarts, and one process owns the storage. tsk holds work steady across
actors, sessions and machines, through a git-synced ledger. Pi Durable models no mission,
objective, report, territory or nexus. It is a runtime for building agent applications,
and tsk is a model of work.

Two items intersect tsk:

- **A runtime beneath an actor.** A tsk actor could run on Pi Durable. A Pi extension,
  written in TypeScript and run in the same process, is the adapter under ADR 0013: it
  calls the `tsk` binary.
- **Declared replay safety.** `replay: "safe"` is a per-tool statement of whether a call
  may run twice. A `tsk` command could carry the same statement.

## Not established

- Whether Pi Durable runs outside TypeScript. Earendil says "We're not ruling this
  out in the future" about a port to Rust, and that it is focusing on TypeScript.
- How a conversation is addressed from another machine. The spec describes clients that
  attach to one process. A network layer was not read, apart from the community repository.
- Whether Pi extensions can call `tsk` commands today. No adapter was written or tried.
- Earendil's funding and the ownership of Pi. The posts read name Earendil and "the Pi
  community" and say nothing more.
