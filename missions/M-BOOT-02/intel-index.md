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

### Fourth data point: `worker_epoch` moved from 32 to 33 across a model switch, session ID unchanged

Captured 2026-09-16 via `get_session` in the same session as the `/clear` experiment
above, some time after the post-clear reading. `id` still
`session_01WePrEonPkCV4kfJPK9D4Sy`. `turn_handoff.worker_epoch` now `33`. The one
recorded change to the session between the two readings is the serving model:
`user_switched_model` and `last_served_model` moved from `claude-sonnet-5` to
`claude-fable-5-1`, while `configured_model` stayed `claude-sonnet-5`.

So a model switch, or the worker hand-off it causes, is one thing that increments the
epoch, and it does so without touching the session ID. Consistent with the
reverse-engineered gloss above (the epoch counts hand-offs of the session's own worker),
but this is one observation with one intervening variable, not a controlled test:
nothing rules out an unrelated worker restart in the same window.

Discrepancy, recorded rather than smoothed over: at this reading
`external_metadata.context_usage.used_tokens` was 636,069, not 0, and the session's full
pre-clear conversation was still present in context. The post-clear reading above
reported `used_tokens` reset to `0`. Either the reset was momentary and the platform
rehydrated the conversation (documented behaviour when a reclaimed session is reopened
is "a fresh VM with your conversation history restored"), or the `0` was read at a
different moment or from a different vantage. Unresolved. The point for context 1
stands either way: `/clear` in a cloud session is not a boundary that changes the
session ID, and the CLI finding above shows the CLI is.

## Design: reverse-lookup from binding to thread

Jim's design, 2026-09-16, built on the confirmed findings above.

Decision: bind a thread to whatever each sub-context of context 1 offers as its
durable identifier. Cloud sessions bind thread to session ID, since that ID is now
confirmed stable across `/clear`. The CLI binds thread to worktree: to run several work
actors in parallel on the CLI, give each its own worktree. Neither the session ID nor
the worktree is the thread's identity — see Actor and Thread in
`docs/domain/ubiquitous-language.md` on `main` — they are just what each sub-context
keeps durable enough to bind to.

Given that, the harness needs one thing: a lookup that turns what it knows at
`SessionStart` (a session ID, or a worktree path) into a thread ID, or establishes that
none exists yet.

**Two tiers, not one.** A binding-table lookup alone only recovers a thread already
bound to the exact session or worktree in front of it — same cloud session across
`/clear`, same CLI worktree reused. It cannot recover a thread from a genuinely new
session or worktree, because nothing yet connects the new key to an old thread. That
case needs an explicit pointer — the initial prompt, or a Handover-style note — naming
which thread to resume. Tier 1 is the automatic case; tier 2 is the told case.

**Algorithm:**

1. Detect sub-context: `$CLAUDE_CODE_REMOTE` is `true` in a cloud session, unset
   locally. (Confirmed pattern, from the session-creation doc's own SessionStart hook
   example.)
2. Resolve the sub-context's key:
   - Cloud: the session ID, from `get_session` with no arguments. This is an MCP tool
     call, not something a bash `SessionStart` hook script can make on its own — open
     question below.
   - CLI: the worktree path, from `git rev-parse --show-toplevel`. A bash hook can do
     this itself, no agent turn required.
3. Look the key up in the binding table (see storage, below).
   - Found: this is that thread. Load its state and continue.
   - Not found: go to 4.
4. Check whether the initial prompt or a Handover pointer names a thread ID to resume.
   - Named: bind the new key to that thread ID (write the binding), resume that thread.
   - Not named: mint a new thread ID, write a new binding for the key, start fresh.

**Storage.** The binding table needs to outlive any single mission, because a thread
now can too (see the Thread/Mission relationship just added to the ubiquitous
language). It doesn't belong inside this mission's own intel or briefing. For now,
while `tsk/bootstrap` is still the scaffolding standing in for tsk's own data (see
M-BOOT's objective), a table at the root of `tsk/bootstrap` — sibling to `index.md` and
`future-missions-tbd.md` — is the pragmatic place; call it out explicitly as temporary,
since the actual home is `tsk/threads/` once tsk itself implements this. Not yet
created.

**Open questions, not yet resolved:**

- Can a `SessionStart` hook read the platform session ID itself, without the agent
  making an MCP tool call first? If not — and nothing found so far suggests it can —
  step 2's cloud branch has to happen as the agent's first behaviour, not inside the
  hook script, which means the hook can resolve a CLI thread before the first turn but
  a cloud session cannot resolve its own thread until the model is already running.
- The thread ID minting scheme itself: not decided.
- Whether "several work actors, one worktree each" needs anything beyond what
  `git worktree add` already gives, or whether tsk needs its own wrapper around it.

### Confirmed directly: a cloud session's ID is a plain environment variable, no MCP call needed

Confirmed 2026-09-16, by reading the environment directly in a running cloud session:
`CLAUDE_CODE_REMOTE_SESSION_ID` holds the session ID (as `cse_...`; swap the prefix to
`session_...` to match `get_session`'s `id` field exactly — confirmed byte-identical in
this session). This overturns the open question raised earlier: a bash `SessionStart`
hook can resolve a cloud session's identity itself, with no MCP tool call and no agent
turn required, exactly as it already can for a CLI worktree. `$CLAUDE_CODE_REMOTE=true`
distinguishes cloud from local, `$CLAUDE_CODE_WORKER_EPOCH` is also a plain env var
(read `36` here, against `get_session`'s `turn_handoff.worker_epoch` reading `34` at
the last check — a discrepancy between the two reporting paths, not yet explained).

Also present: `CLAUDE_CODE_SESSION_ID` (a UUID, e.g.
`67689032-cb06-598e-bf1d-8b0501a09628`), a distinct identifier from
`CLAUDE_CODE_REMOTE_SESSION_ID` — it matches the scratchpad directory path quoted in
this session's own system prompt, so it looks like a lower-level container or instance
ID, not the CCR platform session ID. Don't conflate the two; the thread binding should
use `CLAUDE_CODE_REMOTE_SESSION_ID`, not this one.

### Decided: thread ID scheme

Jim, 2026-09-16. Bare slug, no prefix — the `threads/` directory is the namespace, so
the ID doesn't need to carry one. 8 characters, lowercase alphanumeric (`0-9a-z`, base
36): space is 36^8 ≈ 2.82×10^12, giving a birthday-paradox collision probability of
roughly 0.18% across 100,000 threads (n²/2N). The creation script also checks the new
slug against the lookup file and regenerates on collision, which makes an actual
collision impossible regardless of that figure — the math only says how rarely the
retry would ever fire.

If a thread ID is ever passed outside its own namespace (cross-repo, external
reference), wrap it in a URN rather than baking a prefix into the ID itself. Not needed
yet; noted for when it is.

### Decided: two independent lookup maps, not one with a type field

Jim, 2026-09-16. `threads/lookup-by-cloud-session.json` and
`threads/lookup-by-worktree.json`, each a flat map from binding value to thread ID. The
lookup script branches structurally — check the worktree map first (its key,
`git rev-parse --show-toplevel`, is always computable, cloud or CLI), then the
cloud-session map (only queryable when `$CLAUDE_CODE_REMOTE_SESSION_ID` is set) — rather
than carrying a `binding_type` field to dispatch on. Worktree wins if both would somehow
match, since it's the more specific binding.

This is more general than "cloud defaults to session ID unless in a worktree": nothing
ties a cloud session to session-level binding specifically. A cloud session working from
more than one worktree in the same session gets a thread per worktree via the worktree
map; the cloud-session map only matters for the default case, one worktree per session,
where it doesn't come up. In ordinary use only one map will ever have an entry for a
given session, so the precedence rule is there for the deliberate case, not a decision
that bites in practice.

### Decided: threads/<slug>/ holds an index; full content deferred

Jim, 2026-09-16. Each thread directory has an `index.md` (same convention as missions
and as tsk's own existing thread directories, per the legacy backlog note above). At
minimum it links to the thread's mission — a link to the actual briefing document
(`missions/<id>/<id>-briefing.md`), not a bare mission ID string, so the reference is
directly followable rather than needing a second lookup to resolve. Everything else it
should hold — thread-local state, the handover note itself, its relationship to the
mission's Plan (the task table) — is deferred. Priority is getting `/start-thread` and
`/pause-thread` working first; the Plan relationship specifically is a separate question
to come back to, not decided against.

### Decided: /pause-thread, the handover counterpart to /start-thread

Jim, 2026-09-16. A second skill, doing the handover and tidy-up that ends a thread's
current session. Split deliberately: the agent writes a summary handover note (this is
inherently a judgement call, not scriptable — it requires understanding what happened),
then calls a script for everything downstream of having that note, especially the git
operations (commit, push), which should be as deterministic as `/start-thread`'s own
lookup-and-bind logic. The pattern this session has followed by hand all along — write
the note, then commit and push via `push-bootstrap-ref.sh` — is the working model for
what the script does; `/pause-thread` formalises it rather than inventing something new.

This is the concrete shape of T-13. Update the task to name both commands.

### Decided: what the SessionStart hook does on no binding found

Jim, 2026-09-16. The hook always runs the deterministic lookup (worktree map, then
cloud-session map). If neither has an entry, it doesn't create a thread silently — it
injects additionalContext telling the agent to ask the human directly: "I couldn't find
a thread, do you want to start one? If so tell me what mission we are picking up."
Consistent with staying in supervised mode: creation only happens on an explicit human
answer naming a mission, never automatically.

### Decided: /start-thread is entirely scriptable, no agent judgement

Jim, 2026-09-16. Unlike `/pause-thread`, which needs the agent to write the summary
note, everything `/start-thread` does once a mission is named is mechanical: mint the
slug, check it against the lookup file, write the binding, create the thread's
`index.md`. The skill is a thin wrapper over a deterministic script; the mission name
is the only input that isn't already fully determined by the environment.

### Decided: /start-thread's mission argument is resolved and validated before the script runs

Jim, 2026-09-16. The agent resolves whatever the human says in answer to the hook's
prompt into a concrete mission ID by reading the mission tree, before calling the
script. The script itself also validates that ID exists rather than trusting its
caller — two independent checks, not one relied on to catch everything.

### Decided: /resume-thread, symmetric to /pause-thread, and the hook's other branch

Jim, 2026-09-16. A third skill, `/resume-thread <thread-id>`, the load counterpart to
`/pause-thread`'s save. This resolves the hook's two branches cleanly: when the
deterministic lookup finds a binding, the hook doesn't load the thread itself — it
prompts the agent to run `/resume-thread` with that ID, the same way the not-found
branch prompts the agent to ask the human and then run `/start-thread`. Both branches
end the same way, by naming a command and letting the agent invoke it, rather than the
hook doing the loading or the asking itself.

This also settles the tier-2 case from the original reverse-lookup design (an explicit
pointer naming a thread to resume, rather than the automatic binding lookup finding
one): it's the same command, `/resume-thread <thread-id>`, just invoked directly with an
arbitrary ID instead of one the hook supplied. No separate mechanism needed. Jim's own
framing: this is what lets a different actor take over an existing thread — resuming
isn't restricted to the session or worktree the thread was originally bound to.

### Decided: /resume-thread on take-over is additive, with a printed warning

Jim, 2026-09-16. When `/resume-thread <thread-id>` runs for a session or worktree not
already bound to that thread, it appends the new binding rather than replacing the old
one. Nothing stops two actors running the same thread at once; the binding maps record
that as fact rather than hiding it. The script prints a warning when it finds the thread
already bound elsewhere, so the actor doing the take-over sees it, but does not block
the resume.

This means a lookup map's value for a thread ID is not necessarily unique in reverse
(one thread can appear against more than one session or worktree key). What running
the same thread from two actors at once actually does, in practice, is deferred: Jim's
words, "we can explore this behaviour more fully later."

### Decided: what /resume-thread loads, and how it presents it

Jim, 2026-09-16. No task-index lookup at this point: that's T-04's thread state format,
not yet designed, and stays out of `/resume-thread` for now. The script (the harness
side, deterministic) loads the thread's handover note and pushes it into the agent's
context as a prompt, the same shape as the SessionStart hook's additionalContext.

The agent's job on top of that is to summarise, concisely, then ask whether to carry on
with the thread or do something else. Fixed shape for the summary:

```
thread id: <id>
we are working on mission <mission title>
this is where we are at: <summary of the handover note>
```

### Confirmed by subagent: how to identify a git worktree, and its stable ID

Researched 2026-09-16, verified empirically in a scratch repo (main clone plus a linked
worktree, renamed mid-test to check stability).

- `git rev-parse --show-toplevel` returns the worktree's working directory. It tracks
  cwd, not a durable identity: it changes the moment the worktree directory is renamed
  or moved. Not suitable as a stable ID.
- `git rev-parse --git-dir` and `--git-common-dir` are equal in a normal clone
  (`.git`) and diverge in a linked worktree: `--git-dir` returns
  `<main-repo>/.git/worktrees/<name>`, `--git-common-dir` returns `<main-repo>/.git`.
  `git-dir != git-common-dir` is the test for "am I in a linked worktree at all."
- The stable ID is `<name>`, the basename of the path `--git-dir` returns. It's set once
  at `git worktree add` time (defaults to the target directory's basename, deduplicated
  on collision) and does not change when the worktree is later renamed or moved —
  confirmed: `--git-dir` kept reporting the same `<name>` after a rename, while
  `--show-toplevel` changed immediately.
- The link is bidirectional: the worktree's own `.git` is a file containing
  `gitdir: <main-repo>/.git/worktrees/<name>` (forward pointer); the admin directory
  holds a `gitdir` file with the reverse pointer and a `commondir` file (`../..`). Moving
  the worktree leaves the forward pointer working but the reverse pointer stale, which
  `git worktree list` flags as `prunable` until `git worktree repair <new-path>` fixes it.
- `git worktree list --porcelain` exposes path, full HEAD SHA and branch per worktree,
  but not `<name>` itself — that has to be derived from `--git-dir`, not read off `list`.
- Not a novel technique: the same pattern (derive worktree identity from the `.git` file's
  `gitdir:` target, basename after `/worktrees/`) already exists in other agent/worktree
  tooling doing the same session-to-worktree binding job.

Recommendation for the thread-binding scheme: key the worktree lookup map on `<name>`
(the basename `git rev-parse --git-dir` returns inside `/worktrees/`), not on the
absolute path. Detect "in a worktree" first via `git-dir != git-common-dir`.

### Confirmed by follow-up test: the worktree admin name is not permanently unique

2026-09-16, empirical, git 2.43.0, scratch repo. The `.git/worktrees/<name>` basename
from the earlier finding is stable across a rename or `git worktree move`, but not a
permanent identity:

- Basename collision at `git worktree add` time produces a suffix (`feature`,
  `feature1`, ...), decided by what else exists under `.git/worktrees/` at that moment,
  not by the path.
- Removing a worktree frees its name. A later, unrelated worktree created with the same
  directory basename, even at a different path, is assigned the same name. Confirmed
  directly: removed a worktree named `feature`, created an unconnected one elsewhere with
  the same basename, got `feature` again.
- The main worktree (the primary checkout) has no admin name at all: `--git-dir` and
  `--git-common-dir` are equal there, so this mechanism doesn't identify it.

### Decided: the worktree binding is tsk's own ID, stored as metadata inside .git

Jim, 2026-09-16. Because the git-assigned admin name can be recycled by an unrelated
worktree after removal, `/start-thread` writes its own thread ID as a file inside
`.git`, not just relying on git's `<name>`. Constraint: it must live inside `.git` as
metadata, untracked, so it never appears in the repo's tree or reaches the remote.
Natural location: `.git/worktrees/<name>/tsk-thread-id` for a linked worktree — that
directory is already private per-worktree metadata, and `git rev-parse --git-dir`
already resolves to it directly. Where the main worktree's own marker goes is still
open, since it has no `.git/worktrees/<name>` directory of its own.

### Decided: the worktree marker path is uniform, main worktree included

Jim, 2026-09-16, provisionally ("a little unneeded but let's do it and see"). The
thread-ID marker file lives at `$(git rev-parse --git-dir)/tsk-thread-id`, the same
rule for every worktree, main included. No special case: `--git-dir` already resolves
to a distinct path per worktree (`.git/worktrees/<name>` for a linked one, `.git`
itself for the main one), so the marker never collides even though the main worktree's
own `.git` is otherwise shared object/ref storage that every linked worktree also reads
via `--git-common-dir`. Revisit if it turns out not to earn its keep in practice.

### Decided: pausing is manual for now, scoped to the supervised context

Jim, 2026-09-16. For now, design covers the supervised operating context only. Jim runs
`/pause-thread` himself, manually, before `/clear`. The action set `/pause-thread`
performs is the same regardless of who or what triggers it; only the trigger is
expected to change later, likely to an automated one driven by the goal verifier
(`/goal`), once agent-initiated pausing is designed. Not designing that trigger now,
only noting the action set should not need to change when it arrives.

### Decided: the handover note schema

Jim, 2026-09-16. A fixed schema, not free text, with a template alongside the mission
briefing template's. Fields:

- Mission briefing: a link to the actual briefing document, not a bare mission ID
  (consistent with the earlier thread-index decision).
- Task ID: the task in flight when the thread paused.
- What's next: a brief descriptive summary, agent-written.
- Last pushed commit on `tsk/bootstrap`.
- Last pushed commit on `main`.

The two commit-hash fields are script-captured, not agent-written: `/pause-thread`'s
script already handles the git side, so it reads back the hash it just pushed on each
branch rather than the agent reporting it. Only "what's next" is the agent's own
judgement call; everything else in the schema is deterministic.

### Decided: the handover is an append-only event log, not a note that gets overwritten

Jim, 2026-09-16. The handover lives as JSONL under the thread's own directory, one event
per pause. `/pause-thread`'s job is to append a new event, not to overwrite a file. Each
event carries the schema already decided: mission briefing link, task ID, the
script-captured commit hashes on `tsk/bootstrap` and `main`, and the agent-written
what's-next summary.

Wrapped in a deterministic script, `append-handover.sh`, doing as much of the event as
possible without agent judgement: it captures both commit hashes and the timestamp
itself; the agent supplies only the mission link, task ID and what's-next text as
arguments (mission and task likely already known from the thread's own state, so the
agent may only need to supply what's-next in practice).

This also settles where the note lives: not `index.md` (that stays a stable pointer),
but its own append-only file inside `threads/<slug>/`. Exact filename not yet fixed.

### Decided: /resume-thread reads only the latest event by default

Jim, 2026-09-16. The automatic summary on resume is built from the latest event alone.
The full event log stays queryable by a human operator directly, for cases like a task
stalling across several pauses, but that querying is not part of the automatic resume
flow itself.

### Decided: the handover log is named continuation-state.jsonl

Jim, 2026-09-16. Not `handover.jsonl`. Jim's reasoning: it echoes a concept called
"continuation" from other work of his, adjacent to but not the same as thread. The file
lives at `threads/<slug>/continuation-state.jsonl`. Whether "continuation" itself
belongs in the ubiquitous language, and how it relates to Thread, is not decided yet.

### Decided: lookup map entries carry a registered-at timestamp

Jim, 2026-09-16. Each entry in `lookup-by-cloud-session.json` and
`lookup-by-worktree.json` is `{ "thread_id": "<slug>", "registered_at": "<timestamp>" }`,
not a bare string. Reasoning: cheap to add, useful for diagnosis later, particularly
once `/resume-thread`'s additive take-over means the same thread can have more than one
binding and their relative age matters. Event-sourcing the lookup maps themselves (an
append-only log of registrations rather than current-state JSON) was raised and set
aside for later, not needed yet.
