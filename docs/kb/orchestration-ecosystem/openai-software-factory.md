# OpenAI's agentic software factory

Status: reported 2026-09-15. An internal OpenAI system, described by a third party. Not a
product.

Source: Gergely Orosz,
["Inside OpenAI's agentic software factory"](https://newsletter.pragmaticengineer.com/p/openai-software-factory),
The Pragmatic Engineer, 2026-09-15. Orosz visited OpenAI's headquarters and interviewed
seven engineering leaders and engineers there. The article is paid. The free preview
holds sections 1 to 3 in full and the start of section 4. Sections 4 to 7 were not
read. Quoted phrases were matched against the page's raw text. Claims and figures are
OpenAI staff's own, as reported by Orosz, and were not run independently.

## What it is

OpenAI built a "software factory" around Codex, its coding agent. Orosz defines the term:
"a physical factory where robots and humans produce autos together. In the software
context, it is AI agents and humans producing software." He notes that some factories
are "dark factories" with no humans, and asks whether software engineering could follow.

The article describes an agentic pipeline, from a human stating an outcome to production
monitoring that starts new work. Venkat Venkataramani, VP of Engineering, Applied Infra,
described the pipeline to Orosz.

## Pipeline

| Step | What the article reports |
|---|---|
| 1. Define the outcome | A human builder, an engineer or product manager, specifies the problem and the desired outcome. Judgment, prioritisation and taste grow in importance in this step. Venkataramani says engineers at OpenAI are "more like product managers than traditional systems engineers". |
| 2. Gather context | Codex reads Git repositories and GitHub, Slack and Notion, internal data sources (Databricks, Datadog, logs) and internal skills. OpenAI moved all its documentation into the source code. New engineers are told to ask Codex questions during onboarding. |
| 3. Implement | Codex makes code changes until its goal is met, then verifies the software works. |
| 4. Build, test, CI | The agent builds, runs tests, fixes failures and opens a pull request. The pull request starts CI. The agent "babysits the PR until it's 'green'". A perf harness sends problematic pull requests to a Synthetics A/B framework. |
| 5. Agentic code review | Several agents, each configured as a domain specialist, review a change. Changes are classified by risk. High-risk changes get stricter review, possibly a human reviewer after the agents. Areas of the codebase can opt in to an agent that auto-approves low-risk pull requests. Risk assessment also triggers compliance input, from an agent or a human. |
| 6. Agentic deploy | After a human approves a change, an agent is assigned to it with an instruction summarised as "Handhold this change until it is safely and fully rolled out into production." For a change behind a feature flag, the agent finds the flag, reads what the change does, decides which signals mean success or failure, builds its own monitoring dashboard and watches production. |
| 7. Observe production | Dashboards the agents built, plus OpenAI's internal observability stack. Agents now build dashboards per change. Engineers built them per service before. |
| 8. Feed back | "Perf Factory" sifts alerts and dashboards, de-duplicates signals, identifies latency regressions, finds root causes and proposes fixes. |
| 9. Respond to outages | Sevbot, an incident agent built on Codex, collects context, determines possible mitigations "but never executes any", answers questions in Slack, and applies a mitigation when an engineer tells it to. |

OpenAI's stated long-term goals: a "per-change autonomous SRE" in the deploy agent, and
a Sevbot that mitigates routine outages without waking a human. On-call duty remains.

## Reported figures and practices

- Every OpenAI engineer, researcher, finance and marketing colleague works with an
  "unlimited token budget".
- Non-engineering teams (finance, recruitment, legal) went from about 0% to 90% Codex
  usage in four months. Nearly all employees use Codex or ChatGPT Work weekly.
- Pull requests per engineer grow "like a hockey stick". Some systems see roughly a 10x
  load increase, which Venkataramani says would take two or three years at most
  companies and took about six months at OpenAI.
- OpenAI's internal Codex is "a lot more advanced" than the external product because it
  is connected to most OpenAI systems.
- A `/goal` setting keeps an agent working until a stated goal is complete. Usage rose
  from 60% to 90% between April and May. People keep threads open for days. A
  long-running agent spins off other agents, which reduces what a human manages.
- Role-specific plugins and skills adapt the agent to a team.
- Domain experts are embedded in the ChatGPT Work engineering teams, because the models
  are better than developers in some domains.
- Venkataramani says "the way we do code review today makes less and less sense, and the
  same is true for pull requests".
- Deploying native mobile apps is a bottleneck. Apple and Google review each update by
  hand, which takes hours or days.

## Not read or not documented

- Sections 4 to 7 of the article: tooling and practice changes, infrastructure scaling,
  API reliability, and how the engineering job changes. They are behind the paywall.
- How the pipeline records work: no mission, objective, plan or task record is described.
- How an agent's state passes to another agent or session.
- How permissions are assigned or revoked, beyond risk classification.
- What the token budget costs OpenAI.

## Comparison with tsk

| Aspect | OpenAI software factory | tsk |
|---|---|---|
| Entry | A human defines an outcome. | A Mission with an Objective, a checkable end state, and a briefing. |
| Context | Codex reads repositories, Slack, Notion and data sources. Documentation is in the source code. | A briefing is written when a task passes to another actor. Documentation and artefacts are in the repo. |
| Specialist agents | Domain-specialist review agents, each with its own configuration and context. | Post, a placeholder, holds standing instructions and a scope of authority. Not designed. |
| Authority by risk | A risk class selects the review path. A human approves before deploy. An opted-in area auto-approves low-risk changes. | Authority attaches to a post. Escalation goes to a post. Not designed. |
| Human approval point | Human review above a risk class, and a human approval before deploy. | A post's permissions, with a human or an agent appointed. Not designed. |
| Delivery | The deploy agent watches production signals it selected until the change is rolled out. | Delta Gate: a capability is delivered only when its delta deploys and the system is healthy against acceptance criteria. |
| Signals | The agents build dashboards. Perf Factory feeds regressions back as new work. | Product names signals and observability. No object represents a signal. |
| Incident action | Sevbot proposes mitigations and an engineer applies them. | Escalation to a post. Not designed. |
| Long-running work | `/goal`, threads open for days. Handover is not described. | Thread and thread continuation: an append-only entry at each pause. |
| Cost | An unlimited token budget for all staff. | Token spend is a budget decision for a post. Not designed. |
| Substrate | One vendor's harness, models and internal systems. | The domain model names no substrate. Independence is untested. |
| Scope | Software delivery inside one company. | Software delivery. A repo's own mission and task data in a ledger. |

## Convergence and difference

The factory and tsk agree on three points:

- A human states the outcome. Agents do the work.
- Delivery includes production signals, not only a merged change.
- Documentation sits next to the code, where an agent can read it.

Two more directions are shared, and tsk has not built either: specialist roles with their
own context, and a human approval that depends on a risk threshold. Both depend on Post.

They differ on five:

- OpenAI has no stated budget limit. tsk treats token spend as a decision a post makes.
- The article describes no work record, handover record or continuation entry. tsk
  defines Mission, Objective, briefing, report and thread continuation.
- The pipeline is one company's internal system on one harness. tsk names no substrate.
- Production signals start new work through Perf Factory. tsk has no object for a signal,
  so no work traces back to one. See
  [ai-and-the-loss-of-positive-friction.md](../ai-and-the-loss-of-positive-friction.md).
- OpenAI states a goal of autonomous deploy and incident action. tsk's bootstrap runs
  supervised, with one human appointed to every post.

## Sources

- [Inside OpenAI's agentic software factory, The Pragmatic Engineer](https://newsletter.pragmaticengineer.com/p/openai-software-factory),
  paid article, sections 1 to 3 read

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position
  against the systems assessed.
- [openai-dots.md](openai-dots.md): OpenAI's product for always-on agents, with owner-set
  approval rules.
- [underlying-energy-constraints-of-running-a-factory.md](underlying-energy-constraints-of-running-a-factory.md):
  posts as an escalation target, and token spend as a budget decision.
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): Post, Actor,
  Mission, Thread continuation and Delta Gate.
