# Documentation index

Start here to navigate the documentation. An agent with only the repo clone should use this index to find information by topic.

## Top level

- [vision.md](vision.md): thesis behind tsk and the four dimensions of work: Navigation, Delta, Product, Scale.
- [local-dev-setup.md](local-dev-setup.md): local development setup and the `.inbox/` directory for context files.
- [migration-to-global-daemon.md](migration-to-global-daemon.md): migration guide from per-project daemon to global daemon in tsk 1.6+.

## Domain

Core concepts and models that shape tsk's design. Start with ubiquitous language to understand the terminology.

- [ubiquitous-language.md](domain/ubiquitous-language.md): every term tsk uses, with definitions and rejected alternatives. Includes Ledger (a repo's own mission and task data) and Artefact (what a mission builds), the two sides of the split.
- [domain-model-overview.md](domain/domain-model-overview.md): the mission and task model's decided rules, the tsk/ksobr split table, and open questions.
- [mission-model.md](domain/mission-model.md): why the mission model is structured this way, grounded in military doctrine.
- [bootstrap-rationale.md](domain/bootstrap-rationale.md): the stage-zero self-hosting problem, and the plan format gaps behind tsk's task fields.
- [territory-and-nexus.md](domain/territory-and-nexus.md): why territory and nexus are separate concepts rather than fused.
- [persistence-and-sync.md](domain/persistence-and-sync.md): state persistence strategy and event log design using custom Rust sync.
- [mission-briefing-template.md](domain/mission-briefing-template.md): rendering format for missions as briefings for humans and agents.
- [session-continuation-design.md](domain/session-continuation-design.md): thread binding, the continuation state store, and the `/start-thread`, `/pause-thread`, `/resume-thread` commands.

## User guide

- [installation.md](user-guide/installation.md): prerequisites, installing and upgrading the `tsk` and `tskd` binaries, and CI.
- [getting-started.md](user-guide/getting-started.md): running the daemon, threads, global storage, project binding, tests, building, publishing.
- [state-models.md](user-guide/state-models.md): task and thread state models, diversions, and how the daemon and client fit together.
- [missions-threads-and-continuation.md](user-guide/missions-threads-and-continuation.md): how tsk's own missions and task data are stored and worked, the `/start-thread`, `/pause-thread` and `/resume-thread` commands, and what the `SessionStart` hook does.

## Architecture

Technical design documents describing how tsk's systems fit together.

- [2026-03-08T07-49-54Z-ipc-architecture.v3.md](arch/2026-03-08T07-49-54Z-ipc-architecture.v3.md): client-daemon architecture over Unix socket using JSON-RPC 2.0.
- [2026-03-10-domain-model.md](arch/2026-03-10-domain-model.md): core concepts (threads, tasks, priorities), state machines, data structures.
- [optimising-agent-workflows.md](arch/optimising-agent-workflows.md): lessons from daemon refactor on model selection and delegation.

## Decisions and ADRs

Architecture Decision Records capture why significant technical choices were made.

- [decisions.md](adr/decisions.md): index of all Architecture Decision Records.
- [0001-record-architecture-decisions.md](adr/0001-record-architecture-decisions.md): decision to use Architecture Decision Records.
- [0002-client-daemon-cqrs-architecture.md](adr/0002-client-daemon-cqrs-architecture.md): split into client-daemon with CQRS. Partially superseded by 0004.
- [0003-json-rpc-ipc-protocol.md](adr/0003-json-rpc-ipc-protocol.md): JSON-RPC 2.0 chosen for IPC message protocol.
- [0004-unified-tsk-binary.md](adr/0004-unified-tsk-binary.md): merge CLI and TUI into single binary, superseding ADR 0002 structure.
- [0005-tui-state-refresh-via-file-watch.md](adr/0005-tui-state-refresh-via-file-watch.md): initial approach using OS file system watchers. Superseded by 0006.
- [0006-tui-state-refresh-via-daemon-polling.md](adr/0006-tui-state-refresh-via-daemon-polling.md): current approach via daemon polling. File watch unreliable on macOS.
- [0007-event-log-as-source-of-truth.md](adr/0007-event-log-as-source-of-truth.md): NDJSON log holds state. SQLite cache and markdown are projections.
- [0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md](adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md): why the bootstrap data store moved from a custom git ref to a branch — the Claude Code cloud sandbox proxy blocks writes outside `refs/heads/*`.
- [0009-bootstrap-worktree-outside-the-git-directory.md](adr/0009-bootstrap-worktree-outside-the-git-directory.md): why the bootstrap worktree moved out of `.git/` to an XDG state path, keyed per clone.

## Knowledge base

Background theory and research grounding for key domain concepts.

- [background-theory.md](kb/background-theory.md): theoretical foundations for Navigation and Delta. Co-equality of event, delta, and path.
- [product-and-scale-theory.md](kb/product-and-scale-theory.md): separation of Story card, Product capability, and Delta. No-complecting applied to tracking.
- [session-creation-and-environments.md](kb/session-creation-and-environments.md): orchestrating agent sessions. Which mechanisms start one, and which levers set what it knows.
- [agent-context-self-regulation-and-unattended-handoff.md](kb/agent-context-self-regulation-and-unattended-handoff.md): context awareness, agent-directed memory, `/goal`, and a pattern for a cloud session to hand off to its own successor before running out of room.
- [military-doctrine-sources.md](kb/military-doctrine-sources.md): the primary doctrine sources the mission model draws on, with links, and which citations in the repo are unverified.
- [yegges-levels.md](kb/yegges-levels.md): Steve Yegge's maturity ladder from manual coding to running an orchestrator over many agents, as framing for tsk's own ambition rather than a feature description of his tools.
- [yegge-eight-levels.md](kb/yegge-eight-levels.md): the same ladder, The Pragmatic Engineer's telling, standalone with no tsk-specific framing, for sharing on its own.
- [claude-code-mods.md](kb/claude-code-mods.md): the mods hooks surface in Claude Code, confirmed live in this container via the `agents-md` mod, and why a harness-level hook attaching to engine events matters more to tsk than the `AGENTS.md` question that raised it.
- [typesafe-jev-classifier.md](kb/typesafe-jev-classifier.md): research note on Jev, TypeSafe AI's non-generative decision model, its LangChain harness and evals integrations, and the classifier-in-the-loop pattern it is one data point for.

### Orchestration ecosystem

Other systems building in tsk's space, and tsk's position against them.

- [tsk-market-position-analysis.md](kb/orchestration-ecosystem/tsk-market-position-analysis.md): tsk's market position against beads and Claude Code Projects, with one verdict over both, and the context-boundary difference that separates tsk from a coordinator model.
