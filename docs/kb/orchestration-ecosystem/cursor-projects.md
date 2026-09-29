# Cursor Projects

Status: beta, announced 2026-09-10. Rolling out to all users, except Enterprise plans and
Privacy Mode (Legacy).

Sources: Cursor's [changelog entry](https://cursor.com/changelog/projects),
[documentation](https://cursor.com/docs/agent/projects) and
[launch post](https://cursor.com/blog/projects), with independent coverage listed under
Sources. A claim from independent coverage is marked as such.

## What it is

A project is a container for "a feature, a migration, or a full app". A coordinator agent
plans the work and delegates it to subagents. The coordinator "doesn't write code
itself". It "creates and manage[s] agents on your behalf, running as many in parallel as
the work needs".

Cursor states that a project "maintains context over months of work, delegates tasks to
thousands of subagents, and performs recurring work without being prompted".

## Mechanics

| Aspect | Behaviour |
|---|---|
| Creation | In the Agents window: New Project, then a name, a workspace, and a model for the coordinator. |
| Scope | One repository per project, chosen from connected GitHub repositories. |
| Coordinator | Plans, delegates, and manages parallel agents. It stays responsive to direction because it does not edit code. Where local testing is needed, it starts a local agent. |
| Subagents | Cloud or local agents that work inside the project. |
| Project files | "A set of files that sync across every cloud and local machine its agents use". Agents add "research and artifacts, along with what they learn about the codebase and how you prefer work to be done". A first context file can be the product spec, saved as `docs/project-context.md` (independent coverage). |
| Execution | Each project runs on its own cloud machine. Work continues when the laptop is closed. |
| Subscriptions | A project can watch pull requests (open, merge, CI failure, review comments), a Slack channel or thread, or a schedule. An active subscription shows a "Listening" pill. |
| Oversight | The coordinator returns finished work for review. The launch post describes review easing over a migration: "Early on, you review each PR closely. As the fixes hold up, you review less." |

## Reported results and costs

Cursor reports, in the launch post:

- New users "merge 30% more PRs".
- Primary Projects users "merge six times as many".
- A design-system project is "on track to touch 20 to 100 PRs a day".

Independent coverage reports:

- Five parallel subagents use about five times the tokens of one agent
  ([eesel](https://www.eesel.ai/blog/cursor-projects-review)).
- There is no standalone price. Use is billed by usage
  ([eesel](https://www.eesel.ai/blog/cursor-projects-review)). Cursor has not published
  a price or compute model for project machines
  ([Pondero](https://pondero.ai/news/2026-09-11-cursor-projects/)).
- Slack triggers work for public channels only, and pull request triggers do not run on
  fork-based pull requests ([eesel](https://www.eesel.ai/blog/cursor-projects-review)).
- Subagents do not see prior conversation history. The parent decides what to put in
  the prompt, and reasoning left out of a summary is lost
  ([MemoryLake](https://www.memorylake.ai/en/blogs/cursor-projects-shared-context-files)).
- Project files are scoped to the project. A decision that spans projects is lost unless
  it is kept elsewhere
  ([MemoryLake](https://www.memorylake.ai/en/blogs/cursor-projects-shared-context-files)).

## Not documented

Cursor's documentation and launch post do not state:

- how project files sync, or whether they are stored in git
- which models subagents use, or whether non-Cursor models can run in a project
- a permission or approval model, or a procedure for escalation
- whether a project can be paused and resumed
- what happens when one agent's context fills mid-task
- whether a project can span more than one repository
- usage limits, or spend controls

## Comparison with tsk

| Aspect | Cursor Projects | tsk |
|---|---|---|
| Unit | A project: an outcome, one repository, a coordinator. | Mission: an objective and a briefing. Territory bounds isolation. No container above mission. |
| Coordinator | An agent that produces a plan and delegates the work. It does not write code. | An actor. The Actor entry anticipates a coordination actor. Not built. |
| Delegated worker | Subagent, cloud or local. | Actor holding a thread. A different session picking up a thread is a different actor. |
| Handover to a worker | The parent writes the prompt, and the worker reads shared files. | A mission briefing, written when a task passes to another actor. |
| Persistent state | Project files: research, artifacts, learned preferences and outputs in one set. Storage mechanism undocumented. | Ledger for mission and task data, and Artefact for what a mission builds, split by definition. The ledger is a git branch. |
| Continuity | Context kept in files. Pause, resume, and behaviour at a context limit undocumented. | Thread continuation: an append-only entry at each pause. A successor resumes from the latest entry. |
| Triggers | Subscriptions: pull requests, Slack, schedule. | Event-triggered operating context is named. Not built. |
| Human role | Reviews pull requests. No approval or escalation model documented. | Human and agent actors share one model. Post, a placeholder, would hold authority and escalation. |
| Substrate | Cursor's coordinator and cloud machines. Non-Cursor models undocumented. | The domain model names no substrate. Independence untested. |
| Cost | Usage-based, unpublished. | Token spend is a budget decision for a post. Not designed. |
| Product, Delta, Scale | Not modelled. | Four dimensions. Not built. |

## Convergence and difference

Cursor Projects and tsk agree on four points:

- A coordinator delegates and never edits code.
- A worker does not share the coordinator's context. It is handed only what it needs.
- State that outlives a session is written down.
- Work can start from an event, not only from a prompt.

They differ on five:

- Cursor keeps inputs, outputs and preferences in one set of files. tsk separates the
  ledger from the artefacts.
- Cursor documents no pause, resume or continuation. tsk defines a continuation entry.
- Cursor documents no approval or escalation model. tsk's Post is a placeholder for one.
- Cursor's project is a fixed container of one repository. tsk defines no container
  above mission.
- Cursor models neither Product, Delta nor Scale. Its subscriptions dispatch agents
  directly from an event, with no record of the signal that started the work. See
  [ai-and-the-loss-of-positive-friction.md](../ai-and-the-loss-of-positive-friction.md).

## Sources

- [Cursor Projects, changelog](https://cursor.com/changelog/projects), 2026-09-10
- [Projects, Cursor documentation](https://cursor.com/docs/agent/projects)
- [Introducing Projects, Cursor blog](https://cursor.com/blog/projects)
- [Cursor Projects review, eesel AI](https://www.eesel.ai/blog/cursor-projects-review)
- [Nothing Reaches the Next Agent Unless a File Carries It, MemoryLake](https://www.memorylake.ai/en/blogs/cursor-projects-shared-context-files)
- [Cursor launches Projects in beta, Pondero](https://pondero.ai/news/2026-09-11-cursor-projects/)
- [How to use Cursor Projects, Flavio Copes](https://flaviocopes.com/cursor-projects/)

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position
  against this and the other systems assessed.
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): the tsk
  terms used above.
