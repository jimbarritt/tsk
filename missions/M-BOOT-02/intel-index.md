# M-BOOT-02: intelligence index

All intelligence for M-BOOT-02 (harness), gathered in one file for now. Split into
topic files here once this grows unwieldy.

- `docs/domain/mission-briefing-template.md` (in the tsk repo): the briefing format, with tsk and harness fields
  marked
- `docs/` in the tsk repo, produced by M-BOOT-01
- Claude Code cloud environment documentation
- Cloud sessions clone the repo. Repo `.claude/` files transfer with it. User level
  `~/.claude` files do not, which is why T-01 exists.
- Transcripts are JSONL at `~/.claude/projects/<encoded-dir>/<sessionId>.jsonl`
- Threads hold their own context and can be paused, suspended and resumed. Thread state
  is tsk domain, because the next session may be a different actor. A run record is
  ksobr domain and retrospective. They are two artefacts, not one.
- The plan format's Checkpoint section and `What's Next` pointer are thread state in
  Jim's current practice
- Setup script filesystem output is cached per environment, not per session
- Plugins declared in `.claude/settings.json` are recognised, not installed. An
  unattended session has nobody to accept an install prompt, so the setup script must
  install them explicitly.
- `cargo` and `rustc` are pre-installed on Anthropic-hosted cloud sessions, part of the
  base VM image rather than anything an environment's setup script installs. Confirmed
  2026-09-15: `~/.rustup/settings.toml` carries a March 2026 timestamp, months older than
  the setup-script cache's roughly seven-day expiry, and no `dpkg` entry for `cargo` or
  `rustc` exists, ruling out both an environment setup script and an `apt` install.
- `tsk-bin` and `tsk-core` are already published on crates.io (`0.1.7` as of 2026-04-01).
  The general harness's session init can install tsk with `cargo install tsk-bin` rather
  than building from source or fetching a GitHub release binary. Cost is small (~30s from
  a clean build), so it fits a `SessionStart` hook rather than needing setup-script
  caching. Note the local checkout is ahead of the published crate, at workspace version
  `0.2.0`.
- Cloud environment setup scripts are configured from the web (claude.ai/code) or the
  Desktop app only. The iOS/mobile app selects an existing environment but has no UI to
  create or edit one.
- SessionStart hook confirmed working in a session with only `tsk` attached (2026-09-15):
  `$TSK_BOOTSTRAP_WT` set correctly, resolving the open question left by the prior
  session's cross-repo failure (cwd above both `tsk` and `tsk-nexus`, so
  `tsk/.claude/settings.json` never loaded).
- `tsk` has no prebuilt binary on `PATH` by default; building from source or
  `cargo install` is required.
- tsk is the only repo running this harness for now, so the harness can rely on paths in
  the tsk repo itself, `docs/domain/` included, rather than carrying its own copies.
  This assumption ends the first time another repo installs the harness.
- `jimbarritt/ksobr-transcripts` exists (private, target for T-06) but was empty until
  this session: no commits, no branches. Attaching a second repo mid-session with
  `add_repo` and pushing to it from a fresh clone works, confirmed 2026-09-15 by cloning
  it, committing a test file, and verifying the push landed via the GitHub API rather
  than trusting the git client's output (the push printed a "push negotiation failed"
  warning from the proxy but the push itself succeeded). The remaining question for
  T-06 is whether a repo can be attached automatically at session start rather than
  requiring an explicit `add_repo` call mid-session; not yet tested.

## Long-running agents managing their own context

Gathered by a subagent's web research, 2026-09-16, for the open question of whether and
how an agent can manage its own context across a mission of unknown length, rather than
a human intervening turn by turn. Not independently verified beyond the sources cited.
Treat specific numbers, dates and version details below as leads to check against a
primary source before relying on them, not as settled fact.

### What Claude Code already provides

- Auto-compaction triggers automatically as the context window fills, cannot be
  disabled, and is not something an agent decides to do itself.
  `CLAUDE_CODE_AUTO_COMPACT_WINDOW` overrides the trigger threshold; `/compact` remains
  available for an on-demand pass with optional focus instructions.
  Source: [Claude Code compaction docs](https://platform.claude.com/docs/en/build-with-claude/compaction), primary.
- `PreCompact` and `PostCompact` hooks fire immediately before and after compaction.
  `PreCompact` is reported as blocking and able to intercept the compaction. This is the
  closest thing found to a deterministic moment an agent, or the harness around it,
  could act on rather than relying on the agent remembering to checkpoint. Worth
  prototyping directly rather than trusting the secondary sources describing it.
  Source: [GitHub issue #91910 on hook behaviour](https://github.com/anthropics/claude-code/issues/91910), primary but a live issue thread, not settled documentation.
- A memory tool for managed agents is reported as workspace-scoped persistent storage,
  mounted into an agent's sandbox as a directory, read and written with ordinary file
  tools, versioned per write. Whether this applies to an ordinary Claude Code cloud
  session, as opposed to the separate Managed Agents product, is unconfirmed — check
  this directly before assuming it is available here.
  Source: [Using agent memory](https://platform.claude.com/docs/en/managed-agents/memory), primary.
- Anthropic's own harness pattern for long-running agents: an initializer/coder split,
  where the initializer sets up a structured environment and progress tracking once,
  then a coder makes incremental progress session by session, with state tracked
  externally — a progress file — rather than held only in context. This is Anthropic's
  direct answer to the question this mission is asking. Read it in full before designing
  further. Source: [Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents), primary.

### Patterns from the wider field

Not Anthropic-specific, and not verified beyond the source cited for each. Kept short,
for orientation rather than completeness.

- Memory hierarchies: fast, limited working memory in-context; an external store for
  anything that needs to outlive the context window.
- Tiered memory (MemGPT): an OS-inspired active/warm/cold hierarchy, where the agent
  moves data between tiers itself, via function calls, rather than the harness doing it
  for the agent.
- Reflection: periodic synthesis of raw observations into higher-order notes, each
  grounded by citing the specific observations behind it, to avoid ungrounded
  generalisation compounding over time.
- Progressive summarisation: keep the most recent turns verbatim, compress older ones,
  and always keep a fixed slice of recent context untouched regardless of how the rest
  is compressed.
- Checkpointing: state snapshots at defined decision points, enabling pause, resume and
  replay independent of the model's own context.

Two coding-agent harnesses (Cursor, OpenHands) came up with the same underlying idea,
both third-party rather than Anthropic's own: an external, durable artefact — a tasks
file, an event log — carries the ground truth, and the model's context is treated as
disposable working state layered on top of it. Reported durations and benchmark figures
for these tools are not verified here and are not load-bearing for the design question,
so they are omitted; the pattern is the useful part.

### Gaps

- No documentation found on what Claude Code's own compaction summary preserves versus
  discards, beyond the general description above.
- No documented heuristic, from Anthropic or elsewhere, for when an agent should compact
  its own context versus delegate a piece of work to a subagent instead.
- No comparative data found between compaction, external memory, and subagent
  delegation on the same task.

### Relevance to M-BOOT-02

- The initializer/coder pattern is close to what M-BOOT already does at the mission
  level: a mission briefing plays the initializer's role, setting up context once, and a
  session executes it incrementally. The open questions already recorded above — thread
  state under T-04, and handoff — are asking, in different words, what tsk's own
  progress file should look like. Read the harnesses article as direct input to both.
- `PreCompact` is the concrete mechanism to test for "the agent notices it's running low
  and checkpoints," since it fires deterministically rather than depending on a
  convention the agent might forget to follow.
- The memory tool, if it turns out to apply to a Claude Code cloud session, might be a
  ready-made place for handoff state to live, rather than tsk building its own artefact
  for it. This needs checking directly, not assuming from a summary.

### Read directly: Anthropic's harness article

`https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents`,
read 2026-09-16 via search-aggregated secondary sources, since `anthropic.com` is
blocked by this session's egress proxy and could not be fetched directly.

An initializer runs once: a `feature_list.json` enumerating 200+ granular features, all
marked failing at the start; a git repo with an initial commit; a `claude-progress.txt`
log; an `init.sh` script. A coder then runs incrementally, one feature at a time, each
session committing to git with a descriptive message and updating the progress log —
the artefacts a fresh context window needs to reconstruct state without replaying the
conversation. State is characterised as externalised, path-addressable and
compaction-stable, attributed by the secondary sources to "Pan et al.", suggesting this
formalises an existing framework rather than inventing one; the citation itself was not
chased down. JSON was chosen over Markdown for the feature list specifically because
models are less likely to inappropriately edit JSON. Nothing found, even indirectly,
describes the agent itself deciding when to checkpoint or interacting with compaction —
the pattern avoids that question by making state durable across compaction rather than
by influencing its timing.

### Working hypothesis: no self-regulation mechanism, only external decomposition

Jim's synthesis, 2026-09-16, checked against the findings above.

Hypothesis: no mechanism lets a session regulate its own context, other than
auto-compaction — and that is not really an exception, since the platform triggers it
regardless of what the agent wants, rather than the agent choosing to act. Most
strategies found instead rely on breaking the work into the smallest pieces feasible
before handing it to an agent, externally, rather than on the agent judging for itself
when to act.

Checked: holds for everything confirmed in Anthropic's own tooling. `PreCompact` and
`PostCompact` are hooks the harness can act on, not the agent choosing to. The
initializer/coder pattern above matches the hypothesis's second half directly: work is
decomposed into 200+ granular features before any coder agent runs. One qualification —
the wider research literature (MemGPT) does describe an agent-driven mechanism, the
agent itself calling functions to move data between memory tiers on its own judgement.
Whether anything resembling that exists inside Claude Code or the Claude Agent SDK
specifically, as opposed to only in third-party research, is the open question a
follow-up needs to settle.

### Confirmed directly: a session can inspect its own context usage on demand

Confirmed 2026-09-16, by calling `get_session` with no `session_id` on a running
session: the result includes `external_metadata.context_usage`, with `used_tokens` and
`max_tokens`. This is a genuine capability, distinct from everything else found above —
auto-compaction, `PreCompact`/`PostCompact`, external decomposition all act without the
agent needing to know its own usage; this lets an agent find out.

It does not, on its own, close the self-regulation question: nothing prompts an agent to
call it, and nothing here acts on the number once known. But it is a precondition any
self-directed mechanism would need, and it is confirmed to exist today, callable by a
session on itself, not merely reported in secondary sources. Worth checking whether the
fable follow-up finds anything that uses self-inspection like this as part of a larger
pattern, since this session could only confirm the primitive exists, not that anything
built on it does.

### Fable follow-up: the working hypothesis is refuted, not confirmed

Fable-model research, 2026-09-16, checked directly against primary Anthropic
documentation (two claims re-verified independently: the `/goal` mechanics against
`code.claude.com/docs/en/goal`, and the exact context-awareness warning wording against
`platform.claude.com/docs/en/build-with-claude/context-windows` — both matched
precisely). Full write-up, sources and open items:
`docs/kb/agent-context-self-regulation-and-unattended-handoff.md` in the tsk repo on
`main`.

The working hypothesis above does not hold as stated. Four mechanisms exist, in
ascending order of agent control: automatic compaction (no agent input — and more than
one product sits here, Claude Code's own auto-compaction and the API's separate
server-side compaction beta); context awareness (the API injects remaining budget after
every tool call, confirmed verbatim); agent-directed persistence (the API memory tool
and Claude Code's auto memory — the agent chooses what to write); and on-demand session
inspection (`get_session`, confirmed above, undocumented publicly). Combined with
`create_session`, the last two give a single cloud session everything it needs to read
its own usage and spawn its own successor — a concrete "unattended handoff" pattern
using `/goal` is written up in the doc, with its weak joints listed honestly (`/goal`'s
availability in a cloud or routine-started session is unverified; threshold judgement by
a small model reading text is the weakest link).

### Four operating contexts, the doc's actual framing

Named by Jim, 2026-09-16, once it became clear "the four mechanisms" and "the four
things Jim meant" were not the same four things — a real ambiguity, not a
misunderstanding, and the confusion is recorded in the doc itself as a live example
under context 1. These now open
`docs/kb/agent-context-self-regulation-and-unattended-handoff.md`, ahead of the
mechanisms above: every mechanism applies differently, or not at all, depending on
which of these a session is in.

1. **Supervised interactive.** A human present; this document was written inside one. A
   restart gives a new session ID, so continuity across that boundary needs an
   identifier that outlives the ID. Least developed context, and the one to work next.
2. **Unsupervised autonomous.** No human present; the agent regulates its own context
   and spawns its own continuation until the mission completes. What the `/goal` +
   `get_session` + `create_session` pattern above targets.
3. **Orchestrator spawning workers.** The orchestrator is itself a session and hits the
   same limit its workers do; continuation has to apply recursively.
4. **Event-triggered.** A GitHub Action, a PR event, an issue to process. No session
   lineage going in; each firing starts fresh by construction.

### Confirmed directly: `/clear` does not change session ID or worker epoch

Experiment run by Jim, 2026-09-16, bearing directly on context 1 (supervised
interactive): whether the session ID survives `/clear`.

Pre-clear, captured via `get_session`: `id` was `session_01WePrEonPkCV4kfJPK9D4Sy`,
`turn_handoff.worker_epoch` was `32`, `external_metadata.context_usage.used_tokens` was
622,545.

Post-clear, captured via `get_session` in the same session: `id` still read
`session_01WePrEonPkCV4kfJPK9D4Sy` and `turn_handoff.worker_epoch` still read `32`.
`external_metadata.context_usage.used_tokens` had reset to `0`.

Both the session ID and the worker epoch survived `/clear` in this test. `/clear` reset
the counted context window without starting a new session or a new worker epoch. This
narrows context 1's open question ("a restart gives a new session ID, so continuity
across that boundary needs an identifier that outlives the ID") — `/clear` is not the
kind of restart that changes the ID; some other boundary must be. Jim separately checked
a second, unrelated session to see whether `worker_epoch` is shared across sessions in
the same environment.

### Confirmed directly: `worker_epoch` is scoped to a session, not to an environment

Second data point, gathered by Jim, 2026-09-16, asking an unrelated session ("TSK -
Offline ideas capture interface", `session_015h8qmbPEuyKyaoY7xuur11`) to run
`get_session` on itself. Same environment as the session above,
`env_0173H2wsxugkZUm5Whrkmtv9`, but its own `worker_epoch` read `3`, not anywhere near
`32`.

If the epoch counted restarts of a container shared across all sessions in an
environment, two sessions in the same environment could not report such different
values. It must count something scoped to the individual session instead — most likely
restarts of that session's own worker.

That session's own gloss on the field — "tracks which generation of the worker
container this session is running on; increments each time the container restarts or
hands off work" — is the session's own explanation, not sourced from checked
documentation. Consistent with what has been confirmed here, but treat as an unverified
secondary claim, not settled fact, until checked against a primary source.

That session was also observed running on `claude-haiku-4-5-20251001` while configured
as `claude-sonnet-5`, switched via `/model` — a live example of `configured_model`
diverging from the model actually serving a turn, alongside `external_metadata.model`
and `last_served_model`. Not otherwise relevant to the epoch question; noted in passing.

### Web research: `worker_epoch` is undocumented by Anthropic; one reverse-engineered source corroborates it

Web research run 2026-09-16, checking the `worker_epoch` question against outside
sources while Jim ran a parallel check from the Claude Code command line.

No official Anthropic documentation defines `worker_epoch`, `turn_handoff`, or
`session_context`. Anthropic's public Compliance API does document a "Remote" sessions
endpoint (`platform.claude.com/docs/en/api/compliance/apps/sessions/remote`,
fetched directly), but it uses a different session-ID scheme (`cse_...`, not the
`session_...` IDs `get_session` returns here) and exposes only `id`, `status`
(`active`/`paused`/`archived`/`failed`/`pending`), `created_at`, `updated_at`,
`product_surface`, and user/agent ownership — no `worker_epoch` field, no
`turn_handoff`. So `worker_epoch` is not part of Anthropic's documented public API
surface; it belongs to the private CCR (Claude Code Remote) client protocol between a
session and its backend, not to anything published.

One unofficial source corroborates the field is real rather than a fabrication or an
artefact of the other session's own gloss on it. A community-maintained gist
(`gist.github.com/jedisct1/9627644cda1c3929affe9b1ce8eaf714`), reverse-engineered from
the Claude Code CLI's own code with specific file:line citations, lists
`CLAUDE_CODE_WORKER_EPOCH` as an "Internal/hidden" environment variable: "worker epoch
for CCR client state." It sits in the gist grouped with other CCR bridge/transport
internals — `CLAUDE_CODE_USE_CCR_V2`, `CLAUDE_CODE_POST_FOR_SESSION_INGRESS_V2`,
`CLAUDE_CODE_CCR_MIRROR`, `CLAUDE_CODE_ENVIRONMENT_KIND`,
`CLAUDE_CODE_ENVIRONMENT_RUNNER_VERSION`, `CLAUDE_CODE_SESSION_ACCESS_TOKEN` — which
places `worker_epoch` in the CCR client/transport layer, consistent with it being
scoped to a session's own connection state rather than a fact about the shared
environment. This is a reverse-engineered community source, not Anthropic
documentation, and does not settle what specifically increments the epoch (container
restart, reconnect, or hand-off between workers) — that remains open, resting only on
the two empirical data points already recorded above (survives `/clear`; differs
between two unrelated sessions in the same environment).

### Confirmed directly: `/clear` behaves differently on the CLI than in Claude Code on the web

Third data point, run by Jim, 2026-09-16. Running `/clear` in the Claude Code CLI
produces a new session ID. This is the opposite of the cloud-session result recorded
above, where `/clear` left the session ID (and the worker epoch) unchanged.

So the session ID is not a reliable thread anchor across `/clear` in general — it
depends on which surface the session is running on. It may still work as an anchor for
context 1 (supervised interactive) specifically inside Claude Code on the web / Cowork
cloud sessions, where this mission's own experiment showed it surviving. It does not
work on the CLI, where `/clear` is a session boundary that changes the ID. Any design
for a thread anchor needs to name which surface it targets rather than assuming session
ID behaves the same everywhere.
