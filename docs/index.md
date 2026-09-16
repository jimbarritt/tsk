# Documentation index

Start here to navigate the documentation. An agent with only the repo clone should use this index to find information by topic.

## Top level

- [vision.md](vision.md): thesis behind tsk and the four dimensions of work: Navigation, Delta, Product, Scale.
- [local-dev-setup.md](local-dev-setup.md): local development setup and the `.inbox/` directory for context files.
- [migration-to-global-daemon.md](migration-to-global-daemon.md): migration guide from per-project daemon to global daemon in tsk 1.6+.

## Domain

Core concepts and models that shape tsk's design. Start with ubiquitous language to understand the terminology.

- [ubiquitous-language.md](domain/ubiquitous-language.md): every term tsk uses, with definitions and rejected alternatives.
- [domain-model-overview.md](domain/domain-model-overview.md): the mission and task model's decided rules, the tsk/ksobr split table, and open questions.
- [mission-model.md](domain/mission-model.md): why the mission model is structured this way, grounded in military doctrine.
- [bootstrap-rationale.md](domain/bootstrap-rationale.md): the stage-zero self-hosting problem, and the plan format gaps behind tsk's task fields.
- [territory-and-nexus.md](domain/territory-and-nexus.md): why territory and nexus are separate concepts rather than fused.
- [persistence-and-sync.md](domain/persistence-and-sync.md): state persistence strategy and event log design using custom Rust sync.
- [mission-briefing-template.md](domain/mission-briefing-template.md): rendering format for missions as briefings for humans and agents.

## User guide

- [getting-started.md](user-guide/getting-started.md): running the daemon, threads, global storage, project binding, tests, building, publishing.
- [state-models.md](user-guide/state-models.md): task and thread state models, diversions, and how the daemon and client fit together.

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
- [beads-vs-tsk-viability.md](decisions/beads-vs-tsk-viability.md): strategic assessment of tsk against beads issue tracker.

## Knowledge base

Background theory and research grounding for key domain concepts.

- [background-theory.md](kb/background-theory.md): theoretical foundations for Navigation and Delta. Co-equality of event, delta, and path.
- [product-and-scale-theory.md](kb/product-and-scale-theory.md): separation of Story card, Product capability, and Delta. No-complecting applied to tracking.
- [session-creation-and-environments.md](kb/session-creation-and-environments.md): orchestrating agent sessions. Which mechanisms start one, and which levers set what it knows.
