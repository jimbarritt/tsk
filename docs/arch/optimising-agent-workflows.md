# Optimising Agent Workflows: Lessons from the Global Daemon Refactor

## Context
Refactored `tsk` from a per-project daemon model to a single global daemon across 5 sequential phases, with phases 4-5 running in parallel as background agents.

## Model Selection Matters

**Planning and architecture** — the "what and why" — requires stronger reasoning (Sonnet/Opus). Initial design, trade-offs, and breaking down work need deep context and iterative refinement.

**Execution phases** — "read file X, make these specific edits, run `cargo build`" — are mechanical enough for smaller models (Haiku). Once the task is precisely scoped, execution can be delegated.

This isn't about capability; it's about efficiency. Don't waste expensive tokens on deterministic, well-defined work.

## Parallelism Only Where Genuinely Independent

Phases 1-3 ran **sequentially** as foreground agents:
- Phase 1 (core library) → Phase 2 (daemon) needed the types
- Phase 2 (daemon) → Phase 3 (CLI) needed the RPC changes

Trying to parallelize would have caused merge conflicts on shared files (`core/src/lib.rs`, etc).

Phases 4-5 ran **in parallel** as background agents:
- Phase 4 (zoom feature) touched `cli/src/main.rs` for context detection
- Phase 5 (tests/docs) touched `cli/tests/e2e.rs` and migration guide

Different file scopes = safe parallelism.

**Rule**: Parallelize only when you can commit to genuinely independent outputs. Otherwise, sequential with foreground agents avoids rework.

## Subagent Prompts Must Be Complete Briefings

Each subagent starts **cold** with no prior context. Briefs must include:
- What's already done (which phases completed, what state was changed)
- Exact file paths involved
- Specific edits needed
- Verification step (build/test command that proves success)

Skipping detail here causes agents to re-explore, backtrack, or miss dependencies.

## Verify at Every Phase Boundary

End every phase with `cargo build` (or `cargo test`). This:
- Catches errors before they propagate downstream
- Gives the agent a clear, objective success criterion
- Prevents error compounding across phases

Defer verification to the end, and phase N errors blow up phase N+1.

## Test Isolation Pattern: TSK_HOME

The old approach (`TSK_PROJECT_ROOT`) tied tests to a specific project structure. The new pattern:
- Test sets `TSK_HOME` to a temp directory
- Daemon reads `TSK_HOME` at startup
- Each test gets full isolation

This decouples test setup from project structure and makes tests composable.

## Summary

Large refactors with subagents succeed by: **stronger models for design**, **mechanical execution for smaller models**, **parallelism only where independent**, **complete context in briefs**, **verification at boundaries**, and **isolation patterns** in tests.
