# Unblocked

Status: commercial product, read 2026-10-02. Plans from $29 per user per month. A 21-day
trial of the Platform plan needs no credit card.

Sources: Unblocked's [website](https://getunblocked.com/) and one page of its
[documentation](https://docs.getunblocked.com/what-is-unblocked), listed under Sources.
All pages were fetched as raw HTML and read as text. Quoted phrases were matched against
that text. Figures and claims are Unblocked's own and were not run independently. Two
pages were opened only in part: the blog post on the Context Engine and the Context
Maturity Guide. The trust centre and the rest of the documentation were not read.

## What it is

Unblocked calls itself "the context layer for agentic software development". It
"reasons across your code, conversations, issues, docs, product, and production systems so
agents can complete work correctly with less human intervention". The founder and CEO is
Dennis Pilarinos.

The company argues that connecting an agent to many tools gives it access and not
understanding: "Markdown goes stale. MCP exposes individual systems. Neither resolves
conflicts, weighs recency, or determines what matters for the task at hand."

## Products

| Product | What it does |
|---|---|
| Context Engine | The reasoning system inside the context layer. Unblocked says it selects which sources to trust, resolves conflicts between them and assembles what an agent receives. |
| Unblocked Code | A remote coding agent. A user hands it a feature, bug fix, refactor or test task and gets back a draft pull request. |
| AI Code Review | Pull request review grounded in team decisions and conventions. It also analyses CI failures and posts fixes, and summarises pull requests. |
| Coding Agents and MCP | One MCP server that gives Claude Code, Cursor, Copilot, Windsurf, Codex or any MCP client the same organisational context. |
| Developer Q&A | Questions answered on the web, on a Mac, in Slack or Teams. Every answer links its sources. |

Channels: MCP, a CLI, a REST API, Slack, Microsoft Teams, a web app and a desktop app.

## Context Engine

| Capability | Described behaviour |
|---|---|
| Cross-source reasoning | It "combines semantic retrieval, structured data, and relationships in its knowledge graph" to follow a path such as a ticket to a pull request to a Slack conversation to code. |
| Conflict resolution | Sources are weighed by "current code behavior, recency, domain expertise, and relevance to the task". When one source has more authority, the engine explains why. When a conflict cannot be resolved, it "surfaces the disagreement instead of silently choosing a winner". The pricing page says the main branch is the source of truth and answers are grounded in production code first. |
| Task-specific context | Results are ranked and compressed server-side before an agent receives them. Citations stay attached. |
| Organisational memory | It identifies recurring feedback in pull requests and learns strongly supported conventions from them, scoped to the teams and repositories where they apply. It also identifies who holds expertise in each area. |
| Permission-aware delivery | It "inherits access controls from the systems your organization already uses and enforces them when context is retrieved". It reconciles identities across GitHub, Slack, Jira and SSO. Changing someone's access in the source system changes what the engine returns. The feature is named Data Shield. |
| Freshness | The index syncs continuously. The site contrasts this with rules files, which are "only as good as the last time a human touched it". |

Unblocked states that the engine does not replace rules files such as `CLAUDE.md` or
Cursor rules. It also turns recurring pull request feedback into per-repository rules
delivered with the code.

## Unblocked Code

| Aspect | Behaviour |
|---|---|
| Hand-off | Tag `@unblocked` in a Slack thread, a GitHub or Bitbucket pull request, or a GitLab merge request, or start a task in the Unblocked app. |
| Scope | It "confirms the task scope with you before starting". |
| Execution | Its stated steps are plan, write code, run the build and tests, and verify the result. Each task runs on an isolated, ephemeral machine that is destroyed afterwards, on its own branch. Egress uses static IP addresses a customer can allowlist. |
| Verification | Tasks iterate "until the build passes, the tests pass, and an independent verifier agent confirms the work". |
| Output | A draft pull request with the implementation and verification results attached. |
| Review | "Unblocked Code Review scores the risk, so routine changes do not wait behind high-risk work." Reviewers, CI and branch protection rules are unchanged. |
| Control | Tasks run in the background. A user can ask for status, list tasks or cancel one from any channel. |
| Build set-up | It works out how to build and test from setup instructions, build files and CI configuration. Admins can configure secrets. A repository can include a cloud-init script. |
| Suitable work | "Unattended tasks that frontier coding models can typically complete within a few hours." |
| Repositories | Organisation repositories on GitHub.com, GitHub Enterprise Server, GitLab.com, GitLab Self-Managed, Bitbucket Cloud and Bitbucket Data Center. Personal repositories are not supported. |
| Price | Included with all current plans and the free trial "for a limited time". |

Unblocked states that its products complement Claude Code, Cursor and Copilot. Interactive
agents connect through the MCP server. Unblocked Code takes the tasks handed off entirely.

## Data sources

| Kind | Sources named |
|---|---|
| Source code | GitHub, GitLab, Bitbucket, Azure DevOps, and their self-managed and data-centre editions |
| Continuous integration | Buildkite, CircleCI, GitHub Actions, Jenkins |
| Knowledge | Confluence, Notion, Google Drive, Stack Overflow for Teams, SharePoint, Coda Enterprise, websites |
| Issues | Jira, Asana, GitHub Issues, Linear |
| Messaging | Slack, Microsoft Teams |
| Incidents and product data | Datadog, Sentry, PostHog, Snowflake, Zendesk |
| Other | Custom MCP servers, Google Cloud MCP |

## Reported results

Unblocked reports:

- A single task run twice with the same coding agent, codebase and prompt, "the only
  difference was access to Unblocked". The homepage shows 48% beside fewer tokens and 83%
  beside faster, and a second figure beside each (14% and 25%) that the page does not
  explain. It says the engineer got back "2 hours and 5 minutes".
- The MCP page: "9/10" mergeable code on the first pass, "60%" lower token cost and
  "83%" faster to done, "measured on real tasks, real codebases". The method is not
  described on the page.
- An ROI calculator for 200 employees at a 160K average salary shows $5M annual savings,
  from time saved searching, onboarding and reduced support.
- Customers named: Cloudbeds, Rally, RB Global and Webflow.

Unblocked publishes open-source tools for measuring the effect:

| Tool | What it does |
|---|---|
| Context Engine Simulator | Runs the same coding task twice against a repository, once from the task description alone and once after gathering context. An independent evaluator agent scores both from 0 to 100 against acceptance criteria. It reports quality, speed and cost, and works with Claude Code, Codex, Cursor and Grok. |
| Claude Harness | Runs Claude Code in two git worktrees from one branch, one with Unblocked tools blocked and one with them. It records diffs, tokens, cost, tool calls and timing. |
| Repo Rules Agent | Reads about 40 rules-file conventions (`CLAUDE.md`, `AGENTS.md`, `.cursorrules` and others), extracts structured rules, deduplicates them with embeddings, and flags contradictions for a human. |
| Engineering Social Graph Builder | Builds a weighted graph of who reviews whose pull requests, clusters teams, finds domain experts and flags single-owner areas. |
| Document Query Engine | A workshop project that turns natural language into MongoDB queries over pull request and issue data. |

## Pricing

| Plan | Price | Includes |
|---|---|---|
| Platform | $29 per user per month, billed annually | MCP, chat, Slack and Teams, AI Code Review, CI failure agent, API access, incognito mode. 300 credits per licence. |
| Enterprise | Custom | SSO and advanced security, Data Shield permission enforcement, on-premises deployment options, audit logs, dedicated support, support for GitHub Enterprise, Jira Data Center and Bitbucket Data Center |

The pages do not define a credit.

## Security

SOC 2 Type II, CASA Tier II and GDPR. Authentication is by SAML SSO or OAuth 2.0 to the
customer's identity provider, with no stored passwords. Data is encrypted in transit and at
rest, with customer-specific keys for team data access. Unblocked states that customer
data is not used to train shared models. Unblocked Code runs on Unblocked-managed cloud
infrastructure. Enterprise offers on-premises deployment options.

## Not documented

- How the Context Engine ranks sources, beyond the factors named.
- The model behind Unblocked Code and the Context Engine.
- How an Unblocked Code task is recorded: no mission, objective, plan or task record is
  described beyond free text from a user.
- How a task resumes after a pause, beyond status and cancel.
- How risk is scored, and what a score changes beyond review order.
- What a credit is, and what Unblocked Code costs after the limited-time offer.
- The method behind the 9/10, 60% and 83% figures.
- Whether the Context Engine's index of Slack and tickets is under the user's control
  beyond the permissions of the source systems.

## Comparison with tsk

The table compares what each product does. The next table maps tsk's own terms.

| Aspect | Unblocked | tsk |
|---|---|---|
| Layer | A context layer, a remote coding agent and a reviewer. | Mission, Thread and ledger. Navigation, Delta, Product and Scale. |
| Signals | Reads Slack, Jira, Datadog, Sentry, PostHog and Snowflake into one graph. | The Product dimension names customer signals and observability. No domain object represents a signal. |
| Context for an agent | Retrieved, reconciled, ranked and cited server-side for each request. | A mission briefing, written when a task passes to another actor. |
| Source conflicts | Weighed by recency, authority, expertise and proximity. The main branch is the source of truth. | No mechanism. Decisions are recorded in ADRs and the ledger. |
| Conventions | Mined from pull request review feedback, scoped to teams and repositories. | Rules are written by hand in `CLAUDE.md`, ADRs and, in the future, a post's standing instructions. Not designed. |
| Permissions | Enforced at query time as the requesting user. Identities reconciled across systems. | Post holds authority. A permission store such as SpiceDB is under research. Not designed. |
| Hand-off | `@unblocked` in Slack or a pull request. The scope is confirmed first. A draft pull request comes back. | A Mission delegated to an actor, with a briefing and a report. M-BOOT-03 aims for one unattended run that produces a pull request and a run record. Not built. |
| Risk | Code Review scores risk. Routine changes skip the queue behind high-risk work. | No risk class. Authority per post. Not designed. |
| Work record | A task is free text from a user. | A Mission has an Objective, a checkable end state. |
| Continuity | Organisational memory across tasks. Status and cancel on a running task. | Thread continuation: an append-only entry at each pause. |
| Substrate | Unblocked-managed cloud. The model is not stated. | The domain model names no substrate. Independence is untested. |
| Measuring the effect | An open-source simulator and a harness run each task with and without context. | The token-saving experiment is scoped and not run. |
| Product, Delta, Scale | Not modelled. | Four dimensions. Not built. |

## Comparison by tsk dimension and term

Each row sets what the Unblocked pages describe against the tsk term. A blank mechanism
means the pages describe none.

| tsk term | tsk definition | Unblocked |
|---|---|---|
| Navigation | A route walked. It can split into parallel routes and be abandoned. An abandoned route produces navigational knowledge. | No route is recorded as work happens. The Q&A product answers "Has anyone tried this migration before?" from past pull requests and chat, so abandoned routes are recovered afterwards by retrieval. |
| Path | The story of how a delta came to exist: commits, pull requests, decisions, abandoned routes. | The Context Engine indexes the same material: pull requests, review discussion and decisions in chat. It reads what people wrote. tsk defines what is recorded. |
| Delta | A change to the state of the system. A delta is valid only if the system stays functional before and after. | The output is a draft pull request, with build and test results and a verifier agent's confirmation. Deployment is not part of the described task. |
| Delta Gate | A capability is delivered when its delta deploys and the system is healthy against acceptance criteria. | Not described. Production systems (Datadog, Sentry) are sources of context. No step checks a deployed change against them. |
| Product | What the product does for users, and the state it is in. | PostHog, Snowflake and Zendesk are data sources. Product capabilities with acceptance criteria are not described. |
| System health | Healthy or unhealthy, queryable at any zoom level. | Not described. |
| Scale | One entity viewed at different zoom levels. | A task is sized to "a few hours" of model work. Memory is scoped to teams and repositories. Nesting of tasks is not described. |
| Intelligence | Input context for a mission. | This is Unblocked's product. It assembles, reconciles and ranks input context for an agent. |
| Mission and Objective | A mission is delegated with a checkable end state. | A task is free text. The scope is confirmed before the task starts. The end state is a passing build, passing tests and a verifier's confirmation. |
| Mission briefing | The document handed to an actor, rendering a mission for that actor. | The task and the context the engine assembles for it. The pages do not describe a briefing format. |
| Mission report | Feedback on what the briefing failed to give. | Verification results attached to the pull request. Feedback on missing context is not described. |
| Actor | A human holds many threads. An agent session is bound to one. | The requester's identity is reconciled across GitHub, Slack, Jira and SSO. An Unblocked Code task is a hosted agent. Its identity model is not described. |
| Thread continuation | An append-only entry at each pause. | Not described. A task has status and cancel. |
| Territory | A bounding area where isolation is defined. | The boundary is the source systems' access controls, enforced per requester. The pricing page lists Role-Based Access Control. |
| Nexus | An index of the repos in an area and links to other nexuses. | The knowledge graph indexes repositories and the sources around them, and links tickets, pull requests, chat and code. It is a hosted service, not a routing index kept in the user's own repositories. |
| Ledger | A repo's own mission and task data, held in git. | Not described. Task history sits in the Unblocked service. |
| Post | A standing position that holds authority. | Not described. Authority follows the requesting user. |

## The sharpest difference

Unblocked reads what people already wrote. tsk defines what gets written.

Unblocked's input is the trace that work leaves in other tools: pull request comments,
chat threads, tickets, incident records. It reconciles them after the fact. It states
that "some knowledge, such as unwritten conventions and who actually knows a service,
exists only as patterns across systems over time", and it mines pull request feedback for
conventions. tsk's records are produced as the work runs: a briefing at delegation, a
report at the end, a continuation entry at each pause, and a Path of abandoned routes.

The two do not use the same material. tsk's records are structured and small. Unblocked's
sources are unstructured and large, and the engine does the structuring. A tsk mission
that needs organisational history has an Intelligence section. Unblocked is a source for
one, and its MCP server connects to Claude Code, which tsk runs on. Nothing in the pages
describes a link between the two.

## Convergence and difference

Unblocked and tsk agree on three points:

- Agents fail for lack of organisational context, not for lack of code access.
- A written briefing or ranked context goes to an agent before it starts.
- An unattended run ends in a pull request for human review. For tsk this is the
  objective of M-BOOT-03, not built.

Two more directions are shared, and tsk's design has neither: permissions that follow the
requester, and a risk class that selects the review path. Both depend on Post.

They differ on five:

- Unblocked retrieves context from live sources. tsk's briefing is written once, when a
  task is delegated.
- Unblocked reads production and product systems. tsk has no object for a signal.
- Unblocked records no work structure. tsk defines Mission, Objective, briefing and report.
- Unblocked is a hosted product on its own cloud. tsk is built to run standalone, with a
  ledger in the user's own repositories or a nexus repo.
- Unblocked's output is a pull request. tsk's Delta Gate treats delivery as a deployed
  delta in a healthy system.

Unblocked's open-source harnesses run the same task with and without context and score
both. This is the method of the token-saving experiment that tsk scoped and did not run.

## Sources

- [Unblocked homepage](https://getunblocked.com/)
- [Context Engine](https://getunblocked.com/context-engine/)
- [Unblocked Code](https://getunblocked.com/unblocked-code/)
- [AI Code Review](https://getunblocked.com/code-review/)
- [MCP and coding agents](https://getunblocked.com/unblocked-mcp/)
- [Pricing](https://getunblocked.com/pricing/)
- [Security](https://getunblocked.com/security/)
- [Open source tools](https://getunblocked.com/open-source/)
- [About](https://getunblocked.com/about/)
- [What is Unblocked?, documentation](https://docs.getunblocked.com/what-is-unblocked)
- [The 8 Levels of Context Maturity](https://getunblocked.com/context-maturity/), opened
  in part

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position, and
  the Unblocked section that links here.
- [openai-software-factory.md](openai-software-factory.md): risk classes and agentic
  review inside OpenAI.
- [../spicedb.md](../spicedb.md): a permission store, set against the permissions a post
  would hold.
- [../ai-and-the-loss-of-positive-friction.md](../ai-and-the-loss-of-positive-friction.md):
  the signals and insights tsk does not model.
- [../yegge-eight-levels.md](../yegge-eight-levels.md): the eight-level ladder that the
  Context Maturity Guide builds on.
