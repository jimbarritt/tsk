# Documentation index

Start here to navigate the documentation. An agent with only the repo clone should use this index to find information by topic.

## Top level

- [vision.md](vision.md): thesis behind tsk and the four dimensions of work: Navigation, Delta, Product, Scale.
- [local-dev-setup.md](local-dev-setup.md): local development setup and the `.inbox/` directory for context files.
- [migration-to-global-daemon.md](migration-to-global-daemon.md): historical. Migration guide from per-project daemon to global daemon in tsk 1.6+. ADR 0012 retired the daemon.

## Domain

Core concepts and models that shape tsk's design. Start with ubiquitous language to understand the terminology.

- [ubiquitous-language.md](domain/ubiquitous-language.md): every term tsk uses, with definitions and rejected alternatives. Includes Ledger (a repo's own mission and task data) and Artefact (what a mission builds), the two sides of the split.
- [domain-model-overview.md](domain/domain-model-overview.md): the mission and task model's decided rules, the tsk/ksobr split table, and open questions.
- [mission-model.md](domain/mission-model.md): why the mission model is structured this way, grounded in military doctrine.
- [bootstrap-rationale.md](domain/bootstrap-rationale.md): the stage-zero self-hosting problem, and the plan format gaps behind tsk's task fields.
- [territory-and-nexus.md](domain/territory-and-nexus.md): why territory and nexus are separate concepts rather than fused.
- [personal-and-team-territories.md](domain/personal-and-team-territories.md): analysis, 2026-10-16, of tsk without a nexus, two teams sharing one repo from separate nexuses, and where an actor's own cross-cutting work lives: a personal nexus beside team nexuses, with the open decisions.
- [persistence-and-sync.md](domain/persistence-and-sync.md): the superseded 2026-09-14 custom-ref sync design, kept as a record. ADR 0014 and the Dolt section of ledger-layout.md replace it.
- [mission-briefing-template.md](domain/mission-briefing-template.md): the shape a projection of a mission record will take, as a briefing for humans and agents. Projections are not built yet.
- [session-continuation-design.md](domain/session-continuation-design.md): thread binding, the continuation state store, and the `/start-thread`, `/pause-thread`, `/resume-thread` commands.
- [ledger-layout.md](domain/ledger-layout.md): the Dolt ledger (database location, tables, documents, the transport, push, status, manifest versions, migration of documents and of the transport), the removed files backend, the ledger refs and nexus URL normalisation, the continuation entry format, the external event queue, the local-only files, and what `tsk ledger fetch`, `tsk ledger path` and the session start command do.

## User guide

- [installation.md](user-guide/installation.md): prerequisites, installing and upgrading the `tsk` binary, installing the plugin with `tsk install-plugin claude-cli`, and CI.
- [new-nexus-on-a-clean-machine.md](user-guide/new-nexus-on-a-clean-machine.md): step by step, a new nexus, tsk and the plugin on a clean machine, `tsk nexus add`, `list` and `register-repo`, and a repo's ledger held in the nexus.
- [new-cloud-environment.md](user-guide/new-cloud-environment.md): step by step, a Claude cloud environment on claude.ai/code with the `tsk` binary, the plugin and the ledger ready in every session, and the setup script `ops/cloud/setup-environment.sh` that installs `tsk`, `dolt` and `just` with checksums.
- [checking-ledger-documents.md](user-guide/checking-ledger-documents.md): run a check, such as a prose lint, on ledger documents when they are written and pushed, with a script of your own in one of four places, including for a repo you do not control.
- [getting-started.md](user-guide/getting-started.md): the `tsk ledger`, `tsk thread`, `tsk events` and `tsk nexus` commands, tests, building, publishing.
- [state-models.md](user-guide/state-models.md): historical. Task and thread state models of the retired daemon design.
- [missions-threads-and-continuation.md](user-guide/missions-threads-and-continuation.md): how tsk's own missions and task data are stored and worked, the `/start-thread`, `/pause-thread` and `/resume-thread` commands, and what the `SessionStart` hook does.

## Architecture

Technical design documents describing how tsk's systems fit together.

- [2026-10-17-architecture-and-performance-review.md](arch/2026-10-17-architecture-and-performance-review.md): review at 0.15.1 of the storage seam, the record model, the sync design and the daemon-era remains, with measurements of every command path and a ranked list of proposals.
- [2026-10-18-remote-growth-design.md](arch/2026-10-18-remote-growth-design.md): why a fetch costs what it does, measured on the live origin, with the options (fewer round trips, Dolt 2.5.0, protocol version 0, compaction) and what beads does about the same problem.
- [2026-10-18-documents-in-dolt-comment.md](arch/2026-10-18-documents-in-dolt-comment.md): a comment on putting documents back into Dolt.
- [2026-10-19-m-perf-results.md](arch/2026-10-19-m-perf-results.md): the measured effect of the round-trip and process-launch work, command by command.
- [2026-10-19-is-dolt-the-right-store.md](arch/2026-10-19-is-dolt-the-right-store.md): a review of whether Dolt is still the right store, with beads under several actors, why table files accumulate, and the transport options.
- [2026-10-20-option-5-spikes.md](arch/2026-10-20-option-5-spikes.md): two spikes on the git-carried Dolt file remote: the lease push on the cloud proxy, and git rewriting the remote directory under Dolt.
- [2026-10-20-conformance-suite-speed.md](arch/2026-10-20-conformance-suite-speed.md): where the conformance suite spends its time, measured, with a ranked list of changes and the cause of the Text file busy flake.
- [2026-10-20-session-start-and-pause.md](arch/2026-10-20-session-start-and-pause.md): the profile of a thread pause and a session start through a 330 ms network shim, and the cuts: no question to origin before a pause or a push, fewer `dolt` processes, a refresh that reads while it asks, one fetch for a new clone, and the SessionStart hooks. Release [v0.22.0](releases/v0.22.0.md).
- [2026-10-21-local-dolt-server-spike.md](arch/2026-10-21-local-dolt-server-spike.md): measures a local Dolt server on a socket against one `dolt` launch per call.
- [2026-10-23-test-structure-review.md](arch/2026-10-23-test-structure-review.md): where the test suite spends its time with the server on, the setup each test needs, and a ranked list of changes with measured savings.
- [2026-10-23-markdown-versus-tsk-experiment.md](arch/2026-10-23-markdown-versus-tsk-experiment.md): M-PERF T-07. An agent doing seven ledger jobs by hand-editing Markdown against the `tsk` commands: tokens, tool calls, time and correctness over three runs each.
- [2026-10-21-local-dolt-server-results.md](arch/2026-10-21-local-dolt-server-results.md): the 0.23.0 server measured with the switch off and on: wall time and `dolt` launches for each command, the conformance and workspace suites, memory, and what the server did to older `tsk` binaries. Release [v0.23.0](releases/v0.23.0.md).
- [2026-10-21-publish-by-rule.md](arch/2026-10-21-publish-by-rule.md): which `tsk` commands push before and after 0.24.0, the rule for when records are published, what counts as a claim, the hooks, and `thread resume`, `detach` and `set mission` timed through a 330 ms network shim. Release [v0.24.0](releases/v0.24.0.md).
- [2026-10-22-background-publish.md](arch/2026-10-22-background-publish.md): the background publish of the `Stop` hook, its lock and result file, and the timings of the hook with nothing to push and with unpushed work, server on and off, and a background push through a 330 ms network shim. Release [v0.25.0](releases/v0.25.0.md).

The three documents below describe the client-daemon design, which ADR 0012 retired. They are kept as history.

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
- [0010-ledger-stays-a-branch-not-a-directory-on-main.md](adr/0010-ledger-stays-a-branch-not-a-directory-on-main.md): why the ledger stays a separate branch rather than a `.tsk/` directory committed on `main`, and why the fully-external option stays open rather than decided either way.
- [0011-logic-lives-in-the-binary-not-the-plugin.md](adr/0011-logic-lives-in-the-binary-not-the-plugin.md): all harness logic lives in the `tsk` binary. Plugin hooks, skills and scripts call `tsk` and hold only what belongs to the Claude Code harness.
- [0012-retire-tskd-ledgers-are-the-only-shared-state.md](adr/0012-retire-tskd-ledgers-are-the-only-shared-state.md): `tskd` is retired. Git ledgers, located through the nexus, are the only shared state, and the TUI is rebuilt to read them. A local cache index is a deferred extension.
- [0013-tsk-works-in-any-harness-mission-control-stays-separate.md](adr/0013-tsk-works-in-any-harness-mission-control-stays-separate.md): tsk depends on no harness and each harness reaches it through an adapter. Mission Control stays a separate product, and depends on tsk, not the reverse.
- [0014-dolt-is-the-ledger-store-behind-a-storage-seam.md](adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md): Dolt is the ledger store, run as an external binary and synced over git on a branch, behind a storage trait that kept the git-file ledger as a backend (decision 2, superseded by ADR 0015). Beads is not adopted. Six options compared.
- [0015-the-files-backend-is-removed.md](adr/0015-the-files-backend-is-removed.md): the files backend is removed in 0.16.0. Tsk is committed to Dolt, v0.15.1 is the last release with the backend, and the `LedgerStore` trait stays with one implementation.
- [0016-a-custom-ref-is-the-default-for-a-new-ledger.md](adr/0016-a-custom-ref-is-the-default-for-a-new-ledger.md): a new ledger uses a custom ref (`refs/tsk/*`), and branches under `refs/heads/*` are an override for a repo used from Claude Code cloud sessions. The setting is read from the nexus, then `.tsk/config.toml`, then discovery. Supersedes the "a branch, not a custom ref" part of ADR 0008 for a new ledger.
- [0017-the-ledger-is-a-dolt-file-remote-carried-by-plain-git.md](adr/0017-the-ledger-is-a-dolt-file-remote-carried-by-plain-git.md): the ledger ref holds a Dolt file remote directory that `tsk` moves with plain `git fetch` and `git push`, so Dolt never starts git. Plain pushes only, a retry loop of 10, growth of about 5 KB per publish, and `tsk ledger migrate-transport` for an existing ledger. Releases: [v0.21.0](releases/v0.21.0.md).
- [0018-a-local-dolt-server-per-ledger-store-behind-a-switch.md](adr/0018-a-local-dolt-server-per-ledger-store-behind-a-switch.md): the SQL of a ledger store goes to a `dolt sql-server` on a Unix socket, one for each store, behind `TSK_DOLT_SERVER` or `[ledger] server` (off by default in 0.23.0, on by default from 0.29.0, with a fallback to `dolt sql`). Own MySQL client with no new crate, replacement of a dead or hung server, idle stop, and why the `dolt` command line sees the store read only while a server runs. Releases: [v0.23.0](releases/v0.23.0.md), [v0.29.0](releases/v0.29.0.md).
- [0019-publish-less-often-by-rule.md](adr/0019-publish-less-often-by-rule.md): a record write never pushes. Records are published at thread start (the claim), thread pause and stop, the end of the session (a best-effort `SessionEnd` hook and the `Stop` hook) and `tsk ledger push`. What counts as a claim, which thread commands stop pushing, and the cost of the rule. Releases: [v0.24.0](releases/v0.24.0.md).
- [0020-publish-in-the-background-with-a-detached-one-shot.md](adr/0020-publish-in-the-background-with-a-detached-one-shot.md): the `Stop` hook publishes in the background and does not ask the agent to push. `tsk ledger push --background`, a detached one-shot child, the push lock, the result file, the warning for a failure, why not a daemon, and the limits. Also the 1800 s default idle stop of the Dolt server. Releases: [v0.25.0](releases/v0.25.0.md).
- [0021-state-is-typed-records-free-text-is-documents.md](adr/0021-state-is-typed-records-free-text-is-documents.md): missions, tasks, decisions, questions and thread state are typed rows written only through `tsk`; intelligence, research and reports are Markdown documents on a second branch, linked by path and role. The index is a query. Documents stay out of Dolt rows.
- [0022-numbers-are-per-mission-and-renumbered-at-push.md](adr/0022-numbers-are-per-mission-and-renumbered-at-push.md): short numbers per mission, minted locally; a publish that merges renumbers the publisher's own colliding rows and prints each change. Releases: [v0.19.0](releases/v0.19.0.md).
- [0023-a-tsk-older-than-the-ledger-updates-itself.md](adr/0023-a-tsk-older-than-the-ledger-updates-itself.md): a command stopped by a newer ledger layout updates tsk and runs again; never from the hook commands; a loop guard and an opt-out. Releases: [v0.18.0](releases/v0.18.0.md).
- [0024-a-thread-is-the-unit-of-continuation.md](adr/0024-a-thread-is-the-unit-of-continuation.md): a thread records where work stopped, names a mission or none, and there is no current mission in the ledger.
- [0025-a-documents-hook-point-and-no-git-hooks-in-the-ledger-worktrees.md](adr/0025-a-documents-hook-point-and-no-git-hooks-in-the-ledger-worktrees.md): `.tsk/hooks/pre-docs-commit` is a generic hook point owned by `tsk`, run on `doc put` and before a push commits, with no dependency on any lint tool; git hooks are off in every git process of the ledger. Releases: [v0.26.1](releases/v0.26.1.md).
- [0026-a-mission-has-an-internal-id-and-a-code.md](adr/0026-a-mission-has-an-internal-id-and-a-code.md): `missions.id` stays the key and becomes the internal ID, `missions.code` is the ID a person types, so a rename changes one cell; a changed primary key does not merge in Dolt, so the upgrade only adds a column. Releases: [v0.30.0](releases/v0.30.0.md).
- [0027-a-mission-prefix-belongs-to-the-ledger.md](adr/0027-a-mission-prefix-belongs-to-the-ledger.md): 3 to 4 capital letters that namespace every mission in a ledger, stored as the manifest key `mission_prefix`, chosen once by an agent, checked against `nexus.json`; a trailing number is only for a part of a parent; `create` and `mv` add the prefix when it is missing. Releases: [v0.31.0](releases/v0.31.0.md).

## Knowledge base

Background theory and research grounding for key domain concepts.

- [background-theory.md](kb/background-theory.md): theoretical foundations for Navigation and Delta. Co-equality of event, delta, and path.
- [product-and-scale-theory.md](kb/product-and-scale-theory.md): separation of Story card, Product capability, and Delta. No-complecting applied to tracking.
- [ai-and-the-loss-of-positive-friction.md](kb/ai-and-the-loss-of-positive-friction.md): John Cutler on AI removing the hand-offs where product judgement happened, mapped against tsk. tsk covers the delivery end of the loop; it has no object yet for signals, insights, or models.
- [session-creation-and-environments.md](kb/session-creation-and-environments.md): orchestrating agent sessions. Which mechanisms start one, and which levers set what it knows.
- [agent-context-self-regulation-and-unattended-handoff.md](kb/agent-context-self-regulation-and-unattended-handoff.md): context awareness, agent-directed memory, `/goal`, and a pattern for a cloud session to hand off to its own successor before running out of room.
- [military-doctrine-sources.md](kb/military-doctrine-sources.md): the primary doctrine sources the mission model draws on, with links, and which citations in the repo are unverified.
- [yegges-levels.md](kb/yegges-levels.md): Steve Yegge's maturity ladder from manual coding to running an orchestrator over many agents, as framing for tsk's own ambition rather than a feature description of his tools.
- [yegge-eight-levels.md](kb/yegge-eight-levels.md): the same ladder, The Pragmatic Engineer's telling, standalone with no tsk-specific framing, for sharing on its own.
- [claude-code-mods.md](kb/claude-code-mods.md): Claude Code mods, announced in 2.1.287: plugins of function hooks that rewrite prompts, tool calls and conversation rows and draw interface elements. Events, tiers, loading, built-in mods, and mechanisms that could apply to swe and tsk.
- [claude-code-plugin-packaging.md](kb/claude-code-plugin-packaging.md): how a Claude Code plugin is packaged and hosted: components, binaries, marketplace entries and source types, versions and updates, and how plugins load in cloud sessions.
- [claude-code-mastering-in-30-minutes.md](kb/claude-code-mastering-in-30-minutes.md): Anthropic talk by Boris Cherny, May 2025: the order he recommends for adopting Claude Code, the `CLAUDE.md` and configuration hierarchy, key bindings, `claude -p`, parallel sessions, and what the talk leaves out. Built from a machine transcript.
- [session-names-and-culture-ship-names.md](kb/session-names-and-culture-ship-names.md): how Culture ships are named, with a sample of ship names, and how Claude Code names, lists and addresses sessions: `--name`, `/rename`, the `sessionTitle` hook output, `SendMessage` and cloud sessions. Lists what is not established.
- [typesafe-jev-classifier.md](kb/typesafe-jev-classifier.md): research note on Jev, TypeSafe AI's non-generative decision model, its LangChain harness and evals integrations, and the classifier-in-the-loop pattern it is one data point for.
- [spicedb.md](kb/spicedb.md): SpiceDB, an open source authorization database modelled on Google's Zanzibar, set against the permissions a post would hold in tsk, with what the pages read leave out.
- [adversarial-and-sceptical-review-agents.md](kb/adversarial-and-sceptical-review-agents.md): adversarial and sceptical review agents for AI-generated code. Opens with definitions and provenance of the terms (adversarial, red team, devil's advocate, sceptic agent, critic, verifier, LLM-as-a-judge, debate, code inspection, adversarial collaboration, deterministic and inference checks), each with its canonical reference. Then the evidence: judge self-preference, self-critique, review products and their measured accuracy, sceptical roles in large multi-agent runs, and test gaming. Ends with recommendations for swe, a local reviewer, and a possible review stage in tsk.

### Orchestration ecosystem

Other systems building in tsk's space, and tsk's position against them.

- [tsk-market-position-analysis.md](kb/orchestration-ecosystem/tsk-market-position-analysis.md): tsk's market position against beads and Claude Code Projects, with one verdict over both, and the context-boundary difference that separates tsk from a coordinator model.
- [cursor-projects.md](kb/orchestration-ecosystem/cursor-projects.md): Cursor Projects, a coordinator that delegates to subagents and keeps synced project files, compared with tsk point by point, with what its documentation leaves out.
- [langsmith.md](kb/orchestration-ecosystem/langsmith.md): LangSmith Trajectories, Engine v2 and Fine-Tuning. A tracing and evaluation layer, not a coordinator. Compared with thread continuation, and with the signals and insights stages tsk does not model.
- [openai-dots.md](kb/orchestration-ecosystem/openai-dots.md): OpenAI Dots, always-on agents in ChatGPT with their own cloud computers, owner-set approval rules and role-specific agents, compared with tsk's Post and escalation, with what OpenAI's own pages could not be read for.
- [openai-software-factory.md](kb/orchestration-ecosystem/openai-software-factory.md): OpenAI's internal agentic software factory around Codex, as reported by The Pragmatic Engineer, compared with tsk's Delta Gate, Post and signals, with the paid sections that were not read.
- [unblocked.md](kb/orchestration-ecosystem/unblocked.md): Unblocked, a context layer and remote coding agent that reads code, chat, issues and production systems, compared with tsk's Product dimension, Post and permissions, with what its pages leave out.
- [openappa.md](kb/orchestration-ecosystem/openappa.md): OpenAPPA, an open-source information-flow policy engine that checks each agent tool call against declared contracts, compared with tsk's briefing constraints, Post and objectives, with what its documentation leaves out.
- [orca.md](kb/orchestration-ecosystem/orca.md): Orca, an open-source agent IDE that runs parallel agents in git worktrees with status indicators, orchestration and a mobile companion, compared with the Mission Control task and tsk, with what its documentation leaves out.
- [orca-integration.md](kb/orchestration-ecosystem/orca-integration.md): seams for plugging tsk into Orca as a second delivery surface, read from Orca's own TypeScript source: the worktree comment, `orca.yaml` hooks, a shareable `SKILL.md`, and the plugin system's current limits.
- [copilot-dynamic-workflows.md](kb/orchestration-ecosystem/copilot-dynamic-workflows.md): GitHub Copilot dynamic workflows, a program in a Copilot extension that defines a task's steps and when agents take part, with enforced limits and resumable runs. Compared with Claude Code's feature of the same name and with tsk.
- [claude-code-dynamic-workflows.md](kb/orchestration-ecosystem/claude-code-dynamic-workflows.md): Claude Code dynamic workflows and ultracode, a script a runtime executes with many subagents, resumable, with the `ultracode` setting that makes Claude write one for each substantive task. Compared with tsk, with four combinations (a task executor, an objective checker, delivery through the plugin, a continuation entry that names the run) and what the page leaves out.
- [antithesis.md](kb/orchestration-ecosystem/antithesis.md): Antithesis, a deterministic testing platform that injects faults and random inputs and checks stated properties across every timeline of a run, with the Formance ledger case. Compared with Jepsen and Jev, and with tsk's objectives and the ledger's concurrent writers, with what its pages leave out.
- [herdr.md](kb/orchestration-ecosystem/herdr.md): Herdr, an open-source terminal multiplexer for coding agents, with a background server, agent state per pane, an agent API and restore after a restart. Compared with tmux, the Mission Control task and tsk, with what its documentation leaves out.
- [rex.md](kb/orchestration-ecosystem/rex.md): Rex, Superlogical's terminal multiplexer that is also the terminal, with a self-documenting API, Lua automation and the Program Status Protocol (OSC 7501) by which a program reports idle, working, blocked or done. Compared with tmux, Herdr, the Mission Control task and tsk.
- [efrit.md](kb/orchestration-ecosystem/efrit.md): Efrit, Steve Yegge's open-source coding agent inside Emacs, with its Pure Executor principle, JSON session files, and a one-command channel by which other agents drive a live Emacs. Compared with tsk, with what the repository leaves out.
- [pi-durable.md](kb/orchestration-ecosystem/pi-durable.md): Pi and Pi Durable from Earendil, a durable TypeScript harness with immutable entries, atomic commits, task checkpoints, forks and a storage seam. Compared with tsk's thread continuation, ledger and storage seam, with what the sources leave out.
- [durable-and-proactive-agents-provenance.md](kb/orchestration-ecosystem/durable-and-proactive-agents-provenance.md): where the terms "durable agents" and "proactive agents" come from: durable execution in workflow engines (Temporal, Microsoft, Inngest), the 1995 agent-theory property, and the 2024 to 2025 LLM usage. Includes the sense IndyDevDan's `durable-pi` uses.
- [openspec.md](kb/orchestration-ecosystem/openspec.md): OpenSpec, an open-source spec layer: specs as the record of what the system does, changes as delta specs merged on archive, and stores for planning in a separate repo. Compared with tsk's Product capability, Delta, `Delta Gate` and nexus, with what its documentation leaves out.
- [hosted-agent-platforms-as-tsk-substrates.md](kb/orchestration-ecosystem/hosted-agent-platforms-as-tsk-substrates.md): Dify, n8n, Dust and StackAI as hosted agent platforms, tested as deployment substrates for tsk against Claude Code cloud sessions and routines, with the wider field of coding-agent clouds and sandboxes, the 2026 market, and the changes tsk needs to run on more of them.
- [entire.md](kb/orchestration-ecosystem/entire.md): Entire, Thomas Dohmke's checkpoint CLI and hosted git forge, which records each agent session and links it to the commit. Not a substrate for tsk; overlaps on trace-back from commit to session; an integration layer beside the ledger, with the push-rule, privacy and hook risks.
- [nemoclaw.md](kb/orchestration-ecosystem/nemoclaw.md): NVIDIA NemoClaw, its OpenShell sandbox runtime and the OpenClaw agent. OpenShell is a substrate a tsk robot could run in, with a deny-by-default egress policy and no scheduler; NemoClaw is an alpha single-host distribution; OpenClaw is a harness tsk could integrate with. None competes with tsk.
