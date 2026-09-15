# tsk bootstrap ref index

## Plan

- [plan.md](plan.md): current state and what's next, in mission format.

## Missions

- [missions/M-BOOT.md](missions/M-BOOT.md): top mission, bootstrap tsk self hosting.
- [missions/M-BOOT-01-substrate.md](missions/M-BOOT-01-substrate.md): every place the bootstrap needs exists and holds its first content.
- [missions/M-BOOT-02/M-BOOT-02-briefing.md](missions/M-BOOT-02/M-BOOT-02-briefing.md): a local and a cloud session both load the harness and read a briefing.
- [missions/M-BOOT-03-operation.md](missions/M-BOOT-03-operation.md): one unattended run produces a pull request and a run record.
- [missions/M-LAB-ai-lab-notes.md](missions/M-LAB-ai-lab-notes.md): skeleton mission for a journalling plugin, most fields TBD.

M-BOOT-04 (the official data ref) and M-BOOT-05 (migration off the bootstrap ref) are
listed on M-BOOT.md but have no breakout briefing yet.

## Domain design reference

Distilled design decisions for tsk's own domain model live in the tsk repo's
`docs/domain/` directory: `ubiquitous-language.md`, `domain-model-overview.md`,
`mission-model.md`, `bootstrap-rationale.md`, `territory-and-nexus.md`,
`persistence-and-sync.md`, and `mission-briefing-template.md`.
