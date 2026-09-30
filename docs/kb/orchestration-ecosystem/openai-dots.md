# OpenAI Dots

Status: announced 2026-09-29 at OpenAI DevDay. Rolling out in ChatGPT to Pro and Business
Premium plans. Enterprise workspaces can try the beta when an admin enables it.

Sources: OpenAI's [announcement](https://openai.com/index/introducing-dots/) and its
Help Center articles on dots were not read. `openai.com` and `help.openai.com` return a
Cloudflare challenge (`cf-mitigated: challenge`, HTTP 403) to automated fetches. Facts
come from press coverage read with `WebFetch`, listed under Sources. `WebFetch` passes
each page through a summarising model, so quoted phrases come from that output and are
not checked against the page text. A statement marked "Help Center" comes from a
search-result summary of an OpenAI Help Center page that was not opened. Figures and
claims are OpenAI's own, as reported, and were not run independently.

## What it is

A dot is an always-on agent in ChatGPT, powered by GPT-6 Astra. Each dot "works from a
cloud computer of its own that users can open at any time to see what it is doing". The
owner hands it a goal, and it continues the job "without direction at every step,
even while it has several other projects going". The owner can name it.

Examples reported: monitor customer feedback and implement bug fixes, rerun a scientific
analysis, investigate bugs from Slack mentions, generate invoices.

## Mechanics

| Aspect | Behaviour |
|---|---|
| Assignment | The owner gives a goal. The dot sends "progress updates and questions back to its owner". |
| Channels | ChatGPT on desktop, web and mobile, Slack, Teams, and voice calls. Text messages are announced as coming soon. "Context follows the agent between channels", so a project started in ChatGPT passes to a team in Slack "without a fresh briefing". |
| Memory | "Each agent learns its owner's preferences from feedback." How this is stored is not described. |
| Idle behaviour | With no task, a dot "goes looking for ways to help", called proactive research. Its connections to the owner's apps are read-only during this. |
| Integrations | More than 4,000 apps through OpenAI's plugins. With permission, a dot can work on the owner's laptop. |
| Review | "Actions that could touch a user's accounts or share information must first clear a check called auto-review." A monitoring system can pause or stop a dot over a safety concern. A dot is pretrained not to perform certain sensitive tasks, such as changing a password or transferring money. |
| Custom Rules | The owner sets which supported actions a dot takes on its own, which need approval, and which it must not take (Help Center). Rules cannot turn off core safety requirements, auto-review, or the restrictions on proactive research (Help Center). When auto-review blocks an action, the dot may ask for clarification or approval, try a permitted alternative, or stop (Help Center). |
| Specialist dots | Previewed. "Specialist agents that an employer provisions with their own identity and credentials for a single defined job". Each is set up with its own identity, credentials and tools through existing systems. |
| Teams of dots | Described as a direction: "teams of Dots working together". Not shipped. Integration with Microsoft Agent 365 security controls is reported as under way. |
| Availability | Pro and Business Premium in eligible markets. Pro plans in the European Economic Area, Switzerland and the UK are excluded from initial access. Business Premium is available in all supported regions. |
| Price | One dot is included in a Pro or Business Premium plan. Usage does not count against plan allowances for the first month. More dots, and more speed or capacity, are planned at prices OpenAI has not disclosed. |

## Not documented

The sources read do not state:

- how a dot's memory or working state is stored, exported or handed to another dot
- how work is structured: no mission, objective, plan or task record is described
- how a specialist dot is appointed, replaced or revoked
- how a team of dots coordinates
- whether a dot's actions are audited, or how permissions are revoked
- how a dot escalates when it is uncertain, beyond asking its owner
- usage limits and prices

## Comparison with tsk

| Aspect | OpenAI Dots | tsk |
|---|---|---|
| Unit | A dot: a persistent, named agent with its own cloud computer. | A Mission is the unit of delegated work. An Actor holds a Thread. |
| Authority | The owner sets Custom Rules per class of action: independent, approval, or never. | Post, a placeholder: a standing position that holds authority. Not designed. |
| Role-specific agent | Specialist dot: own identity, credentials and tools for one defined job. Authority is attached to the dot. No source describes a position another dot could fill. | Post separates the position from the actor appointed to it. |
| Escalation | Questions and progress updates go to the owner. A blocked action may ask for approval. | An escalation target is a post. Not designed. |
| Communication | ChatGPT, Slack, Teams, voice. Context follows the agent between channels. | A comms medium, such as Slack, is named as a condition for humans to hold posts. Not designed. |
| Memory and continuity | Preferences derived from feedback. Mechanism undocumented. | Thread continuation: an append-only entry at each pause. The ledger holds mission and task data. |
| Work structure | A goal from the owner. No mission, objective, briefing or report is documented. | Mission with an Objective and a briefing, and a mission report. |
| Multiple agents | Teams of dots described as a direction. | A coordination actor is anticipated by the Actor entry. Not built. |
| Human role | The owner assigns goals, answers questions and reviews consequential work. | Human and agent actors share one model. |
| Substrate | OpenAI's model and cloud. | The domain model names no substrate. Independence is untested. |
| Cost | One dot included. More dots at undisclosed prices. | Token spend as a budget decision for a post. Not designed. |
| Product, Delta, Scale | Not modelled. The product is not specific to software delivery. | Four dimensions. Not built. |

## Convergence and difference

Dots and tsk agree on five points:

- A human sets what an agent may do alone, what needs approval, and what it must not do.
- An agent for a defined role has its own identity and credentials.
- Questions go to a human, in the medium where that human already works.
- The agent's state outlives one conversation.
- More than one agent working together is the stated direction.

They differ on five:

- Dots attach authority to the agent. tsk's Post separates the position from the actor.
- Dots document no work model. tsk defines Mission, Objective, briefing and report.
- Dots document no continuation or handover record. tsk defines a thread continuation.
- Dots run on one vendor's model and cloud. tsk names no substrate.
- Dots are a general assistant, not a software-delivery product. tsk models Navigation
  and Delta for software delivery.

## Sources

- [Introducing dots, OpenAI](https://openai.com/index/introducing-dots/), not read
- [Dots privacy, security, and safety FAQs](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs),
  [Getting started with your dot](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
  and [Manage dots in ChatGPT workspaces](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces),
  OpenAI Help Center, not read
- [OpenAI launches Dots, always-on AI agents in ChatGPT with their own cloud computers, SiliconANGLE](https://siliconangle.com/2026/09/29/openai-launches-dots-always-on-ai-agents-in-chatgpt-with-their-own-cloud-computers/)
- [OpenAI launches Dots, its bubbly agentic avatar, TechCrunch](https://techcrunch.com/2026/09/29/openai-launches-dots-its-bubbly-agentic-avatar/)
- [OpenAI launches Dots, new always-on agents you can assign tasks to, 9to5Google](https://9to5google.com/2026/09/29/openai-dots-agent/)
- [With Dots, OpenAI Wants You to Stop Being Afraid of Its AI Agents, Gizmodo](https://gizmodo.com/with-dots-openai-wants-you-to-stop-being-afraid-of-its-ai-agents-2000819082)
- [OpenAI Launches Dots to Capture AI Agent Market, PYMNTS](https://www.pymnts.com/news/artificial-intelligence/2026/openai-launches-dots-to-capture-ai-agent-market/)

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position, and
  the Dots section that links here.
- [underlying-energy-constraints-of-running-a-factory.md](underlying-energy-constraints-of-running-a-factory.md):
  posts as an escalation target, and token spend as a budget decision.
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): Post, Actor,
  Mission and Thread continuation.
