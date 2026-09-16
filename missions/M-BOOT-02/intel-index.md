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
