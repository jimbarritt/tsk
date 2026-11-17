# LangSmith: rebuilt around the agent

LangSmith now groups every trace, environment, deployment and evaluation under one agent
record, with an owner, a globally unique and immutable ID, and a type. tsk's domain model
has the same structure for a robot. An actor has a uid that never changes, a robot works
in many harness instances at once, and the sessions and their commits name the actor. The
agent record has no unit of work between the agent and its traces. tsk has two such units:
the thread and the mission.

Status: a blog post, "We rebuilt LangSmith around the agent", by Sam Crowder of LangChain,
dated 2026-11-16, read 2026-11-17. It describes the LangSmith Agent Platform, announced the
week before with LangChain's series C. Raised by Jim on 2026-11-17: the post "seems to have
a lot of parallels with our domain convos about threads, harness instances and robots."

Source: [We rebuilt LangSmith around the agent](https://www.langchain.com/blog/we-rebuilt-langsmith-around-the-agent).
The page was fetched as raw HTML and converted to text with a Python HTML parser, and the
whole article was read as text. Quoted phrases were matched against that text. The claims
are LangChain's own and were not run independently. No LangSmith documentation was read.

## What it is

The post states the reasons for the rebuild. Engineers use separate places for tracing,
monitoring and deployment. Platform teams cannot answer "Which agents exist, and who owns
each of them? What do they cost, and how are they performing?"

LangSmith covers the stages build, run, improve and govern, connected "through a unique
agent record":

| Stage | What LangSmith provides |
|---|---|
| Build | A natural language builder in LangSmith Studio, LangChain's open source harnesses, or any other framework |
| Run | LangSmith Agent Server, or the runtime the agent already uses |
| Improve | Tracing, evals, datasets, annotation and Engine, for any agent that sends traces to LangSmith |
| Govern | An agent registry and an LLM Gateway, with org-level visibility and controls |

### The agent record

"Before, traces sent to LangSmith used a flat project hierarchy not associated with any
particular agent." The traces of one agent in different environments had no link to each
other. Now each agent has one record, with its environments and traces grouped under it.
"Each agent's local, dev, staging, and production environments sit under the same agent
ID."

Every agent record has these properties:

- **Owner**: "a named person responsible for the agent".
- **ID**: "a globally unique, immutable identifier".
- **Type**: the kind of agent. The type selects the dashboard, the data connections and the
  evaluation criteria.

Two types exist: general purpose and coding. Voice is planned. A coding agent's dashboard
shows merged pull requests, token cost and the type of work, and a GitHub connection tracks
"downstream outcomes of agent usage". The post gives the reason for types: "a coding agent
is judged by whether the code it writes gets merged, while a customer support agent
requires low latency." Every agent of one type is measured the same way, so the measures of
three people's coding agents are comparable.

### Governance across runtimes

LangSmith registers an agent that runs on any platform that supports the OpenTelemetry
Protocol (OTLP). Such an agent is measured the same way as one built on LangSmith. "Every
registered agent has an owner, ID, and type." Cost and quality metrics roll up to each
agent across every environment. The post's example: a cost spike from a Glean agent has "a
named owner to contact, along with trace details".

### Roadmap

- More agent types, with voice next.
- Dashboards at the level of the organisation, beside the dashboards per workspace.
- Identity: "you'll be able to register agents in your identity provider so that security
  teams can manage and audit them like any other identity in your organization."
- Engine red teams an agent, a team annotates production traces, the reviewed examples go
  to eval datasets, and trajectories from the best runs become fine-tuning data, all on the
  same agent record. [langsmith.md](langsmith.md) describes Engine, Trajectories and
  Fine-Tuning.

## Mapping to tsk's domain model

The tsk terms are defined in
[ubiquitous-language.md](../../domain/ubiquitous-language.md). The decisions are ADR 0024 (a
thread is the unit of continuation), ADR 0029 (session and harness instance URNs), ADR 0030
(an actor belongs to a session, and a harness instance is the continuity key), ADR 0031 (a
thread's mission is its current focus) and ADR 0032 (a thread has one open session, and a
robot works in many), all under [docs/adr](../../adr/decisions.md).

### The agent record and the actor record

**Parallel.** An agent record is the identity that every other record names. In tsk, an
[Actor](../../domain/ubiquitous-language.md#actor) is a record of its own, so a robot's
identity persists outside any session or harness instance. ADR 0030 decision 3: "An actor is
identified by a uid, and its name can change. The uid is minted at registration and is the
key every link names." LangSmith's ID is "globally unique, immutable". tsk's actor URN,
`urn:tsk:actor:<uid>`, is the same kind of identifier. tsk adds a name history: a robot's
name can change, and `actor_names` records each earlier name with the time it was in use.
The post states no name for an agent record beside its ID.

**Difference: owner.** LangSmith gives every agent "a named person responsible for the
agent". tsk has no owner on an actor. The nearest tsk fact is per session: a human who
supervises a robot's session is a supporting actor of that session (ADR 0030 decision 2),
and a robot commit in that session has a `Co-authored-by` trailer that names the person.
So tsk records who supervised each session, and not who answers for the robot. Jim,
2026-11-17: a robot can be owned by a team, not one person. A model of owners needs the
people and their relationships, an organisation map, which is in tsk's remit to model work
as a whole and is not built (M-TSK-AUTO T-06).

**Difference: type.** A LangSmith type selects the dashboard and the evaluation criteria. A
tsk actor's kind is `robot` or `human`, and nothing reads it to select a measure. Robot
roles and archetypes are planned and not built (M-TSK-AUTO T-07). Evaluation of a robot by
sceptical robots and supervisor robots is proposed and not built (M-TSK-IDEA T-07,
M-TSK-AUTO T-03 and T-09).

### One identity, many places where it runs

**Parallel.** An agent's local, dev, staging and production environments are under one
agent ID, and an agent on another runtime joins through OTLP. In tsk, a
[Harness instance](../../domain/ubiquitous-language.md#harness-instance) is "a place where a
harness runs: its harness, its environment and its capabilities", and it "has no direct
connection to an actor" (ADR 0030 decision 1). A robot "works in any number of sessions at
the same time, in any number of harness instances" (ADR 0032 decision 1). In both models one
identity runs in many places, and no place defines the identity.

**Difference.** LangSmith's environments are stages in the deployment of one agent
definition, and a person compares performance between them after a change. tsk's harness
instances have no order: a cloud session and a code worktree on a laptop are equal places.
tsk names each harness with an open set of names (`claude-cloud`, `claude-cli`,
`copilot-cli`, `pi`, ADR 0029), where LangSmith accepts any runtime that sends OTLP traces.

### Traces grouped by agent, and sessions grouped by actor

**Parallel.** LangSmith moved from traces in a flat project, with no link to an agent, to
traces grouped under one record. tsk made the same change in layout 12. Before it, an actor
was registered on a harness instance, and a pause named its writer by the instance key.
ADR 0030 records that thread uu44oy4p had 41 bound instances and no actor. Its decision 2
states that "An actor is associated with a session, never with an instance". A session has one primary actor and any
number of supporting actors, as rows of `session_actors`. A pause names the session that
wrote it. "The actors of a thread are the actors of its sessions: a query over the
timeline, not a field on the thread."

**Parallel.** A LangSmith trace names the agent across runtimes. A tsk
[Session](../../domain/ubiquitous-language.md#session) is named by
`urn:tsk:session:<harness>:<id>` (ADR 0029), so a session ID minted by one harness is never
confused with the same string from another. A commit made in a session has the trailer
`Agent-Session` with that URN. LangSmith's coding agent type connects GitHub to read merged
pull requests. tsk links each commit to the session that made it, and through the session to
its actors.

**Difference.** A trace records an agent's execution in detail. A tsk session row records a
start, an instance and its actors, and the work of the session is in its commits and its
pause entry. [langsmith.md](langsmith.md) compares a LangSmith trajectory with a thread
continuation.

### No thread in the agent record

**Difference.** The agent record has the agent, its environments and its traces. It has no
unit of work between them. tsk has two:

- A [Thread](../../domain/ubiquitous-language.md#thread) is "the unit of continuation" (ADR
  0024): a line of work whose timeline is its sessions in order, with a continuation entry at
  each pause. A thread has at most one open session at a time, its holder (ADR 0032
  decision 2), and a different actor can resume a paused thread.
- A [Mission](../../domain/ubiquitous-language.md#mission) is the definition of the work: its
  objective, tasks, decisions and questions. A thread's mission is its current focus, and a
  change of mission keeps the thread (ADR 0031).

In LangSmith the agent is the unit that every record names. In tsk the robot is an
identity, the thread ties sessions together, and "the mission and the task bind the work"
(ADR 0032 decision 3).

The word "thread" does not appear in the post. In other LangSmith documents a thread is
"linked traces from a multi-turn session" ([langsmith.md](langsmith.md)), which is closer to a
tsk session than to a tsk thread.

### Governance: owner, cost and identity

**Difference: cost.** LangSmith rolls cost up to each agent, and a cost spike has a named
owner to contact. tsk records no token cost per actor, session or mission. A budget is named
in a briefing's Execution constraints as text, and no component enforces it. Cost per actor,
and cost in general, is a later addition, possibly with ksobr (M-TSK-AUTO T-08).

**Difference: authority.** LangSmith's registry and LLM Gateway give "strict controls over
every registered agent". tsk's authority model is proposed and not built: the
[Post](../../domain/ubiquitous-language.md#post), a standing position with a scope of
authority, and supervisor robots. The permissions that control what a robot can do, such as
its tools, repositories and credentials, are a separate question (M-TSK-HARDEN T-19).

**Parallel: identity.** LangSmith's roadmap registers agents in an identity provider, so
security teams manage them "like any other identity". tsk gives a robot a git identity
now ([commit-attribution.md](../../domain/commit-attribution.md)). A robot commit has the
robot's actor name as author and committer, the email of a shared GitHub robot account,
and a signature with that account's key, so GitHub shows it as Verified. The GitHub account
identifies robots as a group, and the actor name identifies the robot.

## Comparison with tsk

| LangSmith | tsk | Relation |
|---|---|---|
| Agent record | Actor record | Parallel |
| ID, "globally unique, immutable" | Actor uid, `urn:tsk:actor:<uid>` (ADR 0030) | Parallel. tsk also keeps a name history |
| Owner, a named person | A supporting human actor, per session (ADR 0030) | Different. tsk has no owner per robot. Team ownership and an organisation map are to explore |
| Type, selects dashboard and evaluation | Kind, `robot` or `human` | Different. Roles and archetypes are planned, evaluation by supervisor robots is proposed |
| Environments under one agent ID | Harness instances, any number per robot (ADR 0030, 0032) | Parallel. LangSmith orders them as stages, and tsk does not |
| Traces grouped under the agent | Sessions with their actors in `session_actors` (ADR 0030) | Parallel. Both replaced a link from the place to the identity |
| Runtimes that send OTLP | Harness names in the session URN (ADR 0029) | Parallel. Both name the runtime and accept new ones |
| GitHub connection for a coding agent | `Agent-Session` trailer on each commit (ADR 0029) | Parallel. Opposite direction: LangSmith reads GitHub, tsk writes the link into git |
| None | Thread, the unit of continuation (ADR 0024, 0032) | tsk only |
| None | Mission, with the thread's current focus (ADR 0031) | tsk only |
| Cost per agent | None | LangSmith only. Cost is a later addition for tsk |
| Registry and LLM Gateway controls | Post and supervisor robots, proposed | Built in LangSmith, proposed in tsk |
| Identity provider, on the roadmap | Robot name on signed commits, with a shared GitHub robot account | Parallel. tsk has it now, for git |

## Convergence and difference

LangSmith and tsk agree:

- An agent or a robot is an identity with an ID that never changes.
- One identity runs in many places at once, and no place defines the identity.
- Execution records name the identity, not the place: traces in LangSmith, sessions and
  commits in tsk.
- A runtime is named, and the set of runtimes is open.

They differ:

- LangSmith's unit is the agent. tsk separates the robot (who), the thread (the line of
  work) and the mission (the work), so a different robot continues the same work.
- LangSmith records an owner, a type, cost and quality for each agent. tsk records none of
  these for an actor yet. Roles and archetypes are planned, and cost is a later addition.
- LangSmith observes agents and evaluates them. tsk coordinates work. Evaluation by
  sceptical robots and supervisor robots is proposed, and tsk checks no objective yet.

The post models no [Product](../../domain/ubiquitous-language.md#product),
[Delta](../../domain/ubiquitous-language.md#delta) or
[Scale](../../domain/ubiquitous-language.md#scale), and no mission or thread. LangSmith stays
a related layer, beside the systems assessed in the market position analysis.

## Not documented

The post does not state:

- whether an agent record has a name, and whether it can change
- whether an agent can have more than one owner, or none
- how a coding agent's work is split into units smaller than the agent, such as a task or a
  session
- how a run that pauses and resumes, on the same runtime or another, is recorded
- how the LLM Gateway enforces a control, and which controls exist

## Related

- [langsmith.md](langsmith.md): LangSmith Trajectories, Engine v2 and Fine-Tuning, with the
  LangSmith meaning of "thread".
- [langchain-from-tasks-to-goals.md](langchain-from-tasks-to-goals.md): LangChain's post on
  goals, published the same day, mapped to tsk's mission model.
- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position, with a
  LangSmith section.
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): the tsk terms
  used above.
