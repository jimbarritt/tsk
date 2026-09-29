# LangSmith: Trajectories, Engine and Fine-Tuning

Status: announced 2026-09-24, in a roundup post dated 2026-09-25. Trajectories is
available on all plans in the US. Engine v2 is available on Plus and Enterprise SaaS, with
red teaming in private beta. Fine-Tuning is in public beta.

Sources: LangChain's [roundup post](https://www.langchain.com/blog/langsmith-engine-agents-fine-tuning-trajectories)
and the three posts it links, listed under Sources. All four were read directly with
`WebFetch`, which passes each page through a summarising model. Quoted phrases come from
that output and are not checked against the page text. Figures are LangChain's own and
were not run independently.

## What it is

LangSmith is LangChain's platform for tracing, evaluating and improving agents. The
roundup post, "New in LangSmith: Engine v2, Managed Deep Agents, Fine-Tuning, and More"
(Jacob Talbot), announces Engine v2, Managed Deep Agents v0.8, Trajectories,
Fine-Tuning and Custom Apps. This note covers Trajectories, Engine v2 and Fine-Tuning.
Managed Deep Agents v0.8 and Custom Apps are not assessed.

LangSmith is not a coordinator. It observes agents that run elsewhere and feeds what it
observes back into their evaluation, repair and training. The other documents in this
directory assess coordinators.

## Trajectories

A trajectory is "a chronological, conversational view of an agent session". It collects
messages from humans, AI and tools across the main agent and any subagents, and shows
them "in the order they first appeared". Each message appears once.

| Term | Meaning in LangSmith |
|---|---|
| Trace | The full execution tree: nested runs, timing, retries, inputs, outputs and metadata. |
| Thread | Linked traces from a multi-turn session. |
| Trajectory | "A projection over the traces in a thread". It drops the nested structure and keeps the path of behaviour. |

What a team can do with one:

- View a thread as a trajectory.
- Score it with an online evaluator. The evaluator reads each message once, in order,
  instead of context repeated turn by turn.
- Route it to an annotation queue, where subject-matter experts score behaviour, flag
  issues and give feedback.
- Save a high-quality trajectory to a dataset and export it for supervised fine-tuning.

The roundup gives the reason: a long-running session "can include many turns, tool calls,
retries, and subagent handoffs", and a trajectory gives a reader "a readable session view
that they can score, flag, and annotate without having to parse execution metadata".

Announced 2026-09-24 by Winston Huynh and Bonnie Pecevich.

## Engine v2

Engine launched in May 2026. It reads production traces from connected tracing projects
on a dynamic schedule, not in real time, and is charged in LangChain Compute Units.

The loop:

1. Detect a recurring issue. Engine classifies related traces and groups them into one
   issue. The issue carries a root cause, proposed solutions and monitoring data.
2. Propose a fix, as a prompt change or a code change.
3. Track new traces that match the issue.
4. Reopen the issue if it recurs.

Detection covers errors and unmet user requests, inefficient trajectories such as
incorrect tool calls, and trends in error rate, latency and cost.

Engine v2 adds two capabilities:

- **Red teaming.** Engine reads an agent's production traces and repositories to infer its
  purpose, then tests for weaknesses such as hallucinations and prompt violations before
  they reach production. Private beta, for LangSmith Deployment users.
- **Fix validation.** Engine reproduces the failure with the offending inputs, proposes a
  change, tests it against the same inputs, iterates, and confirms the fix before a
  person sees it. The roundup adds that Engine "tests candidate fixes against a broader
  eval set". A person opens a pull request from the result with one click.

Other outputs and integrations: ground-truth dataset examples taken from production
traces, pull requests through GitHub, and notifications through Slack and webhooks to
incident, paging and chat tools. Supported agents are built with Deep Agents, LangChain
or LangGraph.

Reported results:

- Since the May launch, Engine analysed more than 70M traces and diagnosed tens of
  thousands of issues. The roundup gives 60M for the same period.
- Issue detection improved by more than 2x on IssueBench.
- Generated fixes are 25% more effective on a Terminal-Bench-style measure.

Announced 2026-09-24 by Ben Tannyhill and Trammell Saltzgaber. Self-hosted support and
bring-your-own-key are announced as coming shortly.

## Fine-Tuning

Supervised fine-tuning (SFT) of an open model on "high quality examples of a task". A
training example is a trajectory: an ordered sequence of messages, tool calls and tool
results.

Data selection has three stages:

1. Pull trajectories from LangSmith projects, with optional filters.
2. Label the good ones against rubrics. Humans and agents both label, and a council of
   agents reviews and filters the candidates.
3. Store the approved trajectories as a LangSmith dataset. The workflow filters for
   sequence length and splits the data into train, validation and test sets.

The post states that data selection is one of the biggest drivers of fine-tuning gains.

The `smithtune` CLI runs four steps: plan (model, examples, hyperparameters), train
(LoRA jobs on Fireworks or Baseten), evaluate (replay evaluation of the base and tuned
models against golden trajectories, with results in LangSmith) and deploy
(`smithtune deploy`).

Two internal results are reported:

| Case | Setup | Result |
|---|---|---|
| Engine | Kimi K3 with SFT | Task score 96.0, against 90.0 for the base model and 87.0 for GPT-5.6 Sol. |
| OpenSWE Review | Qwen-3.8-27B with SFT | F1 from 48.9% to 53.7%, with 29.8% fewer model calls. |

Announced 2026-09-24 by Ankush Gola, Jake Broekhuizen and Vivek Trivedy. The roundup
links this post at a staging domain, `langchain-tonik.webflow.io`. The post is also at
`langchain.com/blog/langsmith-fine-tuning`, which is the URL read here.

## Not documented

The sources read do not state:

- how a trajectory marks a subagent handoff or a context reset
- limits on trajectory size, or pricing for Trajectories
- whether Engine supports agents built on other frameworks
- Engine's false-positive rate or precision
- the council-of-agents mechanism used in Fine-Tuning
- pricing, training time or compute cost for Fine-Tuning, or support for reinforcement
  learning
- a date for self-hosted Engine v2

## Comparison with tsk

| Aspect | LangSmith | tsk |
|---|---|---|
| Record of a session | Trajectory. Derived by the platform from traces, after the fact. Each message once, in order. | [Thread continuation](../../domain/ubiquitous-language.md#thread-continuation). Written at each pause, append only. The raw session record is a transcript pushed to `ksobr-transcripts`. |
| Who writes it | The tracing layer. | An actor writes the account of what is next. A script writes the commit fields. |
| What it is for | Scoring, annotation and training data. | Resuming the thread. |
| "Thread" | Linked traces from a multi-turn session. | The [execution sequence](../../domain/ubiquitous-language.md#thread) with its own identity, pausable and resumable. The same word with a different meaning. |
| Signals | Production traces from agents. | Named under Product, not modelled. |
| Insights | An Engine issue: grouped traces, root cause, proposed fix, monitoring data, reopened on recurrence. | No object. |
| Proof of a fix | Reproduce the failure, test the change on the same inputs, iterate, confirm before showing a person. | A behavioural delta is proven by a test going from failing to passing. Under the Delta Gate, a capability is delivered when its delta is deployed and the system is healthy. |
| Human role | Experts annotate in queues. A person opens the pull request. | Human and agent actors share one model. Post, a placeholder, would hold authority and escalation. |
| Coordination | None. | Missions, threads and actors. |
| Substrate | Engine covers Deep Agents, LangChain and LangGraph. Other frameworks undocumented. | The domain model names none. Independence untested. |
| Product, Delta, Scale | Not modelled. | Four dimensions. Not built. |

## Convergence and difference

LangSmith and tsk agree on four points:

- A session leaves written state that outlives it.
- Human judgement stays at review: an annotation queue, a pull request.
- A fix is proven against the failing case before a person is asked to accept it.
- Work is traceable to its source: an Engine issue links to the traces behind it.
  [Remedy 1](../ai-and-the-loss-of-positive-friction.md#remedies-mapped-to-tsk) in the
  positive-friction note asks for this, and tsk has no counterpart for it.

They differ on four:

- A trajectory is a read model derived after the fact. A continuation is written for the
  next actor. One does not replace the other.
- LangSmith models signals and insights, but only for an agent's runtime behaviour.
  Customer signals stay a gap in tsk, and LangSmith does not fill it. Engine flags unmet
  user requests found in traces, which is the nearest it comes.
- LangSmith has no mission, no coordination and no Product, Delta or Scale.
- The word "thread" names two different things. Use "LangSmith thread" when both meanings
  appear in one document.

## Not decided here

The ledger holds no record of what a session did beyond continuation entries and mission
reports. The transcript push in `CLAUDE.md` is a raw copy in another repository. A
trajectory is one candidate shape for that record. Whether tsk needs the record is not
decided here. This is a research note, not a proposal.

## Sources

- [New in LangSmith: Engine v2, Managed Deep Agents, Fine-Tuning, and More](https://www.langchain.com/blog/langsmith-engine-agents-fine-tuning-trajectories),
  LangChain, 2026-09-25
- [Trajectories now in LangSmith: A readable view of every agent session](https://www.langchain.com/blog/langsmith-trajectories-tracing),
  LangChain, 2026-09-24
- [LangSmith Engine v2: Red teaming and automated testing](https://www.langchain.com/blog/langsmith-engine-v2-redteam),
  LangChain, 2026-09-24
- [LangSmith Fine-Tuning](https://www.langchain.com/blog/langsmith-fine-tuning),
  LangChain, 2026-09-24
- [LangSmith Engine, documentation](https://docs.langchain.com/langsmith/engine-overview)
- [Trajectories, documentation](https://docs.langchain.com/langsmith/observability-concepts#trajectories),
  linked from the Trajectories post, not read

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position, with
  a LangSmith section beside the Jev section.
- [ai-and-the-loss-of-positive-friction.md](../ai-and-the-loss-of-positive-friction.md):
  the signals and insights stages that Engine covers for agent behaviour and tsk does
  not model.
- [typesafe-jev-classifier.md](../typesafe-jev-classifier.md): Jev-as-a-Judge, run through
  LangSmith evals.
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): the tsk
  terms used above.
