# Provenance of "durable agents" and "proactive agents"

Status: research note, read 2026-10-16. Neither term has one originator that the sources
show. Each is a recent label on an older idea.

Raised by Jim, 2026-10-16, who heard both terms from IndyDevDan (the community repository
`disler/durable-pi`, see `pi-durable.md`) and asked whether they come from elsewhere.

Sources: the `disler/durable-pi` README at `37165c6`, Earendil's posts on Pi Durable and
Pi 1.0 (saved copies), and the pages linked below. Search results gave the dates for the
Temporal and OpenAI press release, the Microsoft announcement and the arXiv paper. Pages
that a fetch could not read are named under "Not established".

## Summary

| Term | Where it comes from | Earliest use found |
|---|---|---|
| Durable agents | Durable execution, from workflow engines, applied to LLM agents by vendors in 2025 | Temporal and OpenAI integration, 2025-07-30 (press release URL date) |
| Proactive agents | Two lines. The 1995 agent-theory property "pro-activeness". The 2024 to 2025 LLM usage, which contrasts it with reactive agents | Wooldridge and Jennings, 1995. LLM usage: arXiv 2410.12361, 2024-10-16 |

IndyDevDan did not coin either term. His README uses both in senses the vendors and
researchers already use. Earendil's two Pi posts use neither term.

## Durable agents

**Parent term: durable execution.** Maxim Fateev, Temporal's co-founder, says durable
execution is "a new abstraction, which we kind of introduced". That is his own claim, not
an independent attribution. The lineage he gives: Amazon Simple Workflow Service, the
Durable Task Framework at Microsoft, Cadence at Uber (about 2015), then Temporal (2019,
a fork of Cadence). A program is a workflow whose steps are recorded, so a crash resumes
from the last recorded step. Sources: the
[Software Engineering Radio interview](https://se-radio.net/?p=8712) and the
[Temporal podcast page](https://temporal.io/blog/oss-startups-podcast).

**The label on agents.** No page read states who first wrote "durable agents". Uses
found, in date order:

| Date | Who | Use |
|---|---|---|
| 2025-07-30 | Temporal and OpenAI | [Public-preview integration](https://www.businesswire.com/news/home/20250730783559/en/Temporal-and-OpenAI-Launch-Integration-for-Enterprises-Developing-Production-Agents) of Temporal with the OpenAI Agents SDK. Agent loops run in a workflow. Model and tool calls run as activities. The press release text was not read, so the exact phrase there is not confirmed |
| 2025-11-13 | Microsoft | [Durable task extension](https://techcommunity.microsoft.com/blog/appsonazureblog/-/4467122) for Microsoft Agent Framework, public preview. It brings Azure Durable Functions' durable execution to agents |
| 2026-01-20 | Temporal | [Building durable agents with Temporal and AI SDK by Vercel](https://temporal.io/blog/building-durable-agents-with-temporal-and-ai-sdk-by-vercel). Durable agents "run for hours, survive infrastructure hiccups, and pick up exactly where they left off" |
| 2026-05-04 | Microsoft | [Learn page](https://github.com/MicrosoftDocs/azure-docs/blob/main/articles/durable-task/sdks/durable-agents-microsoft-agent-framework.md): agents are "automatically durable with persistent sessions". Durable agents are built on durable entities |
| undated | Inngest | [Docs](https://www.inngest.com/docs/learn/durable-agents): "A durable agent keeps its progress when anything goes wrong." "A durable agent is a durable workflow whose next step is chosen by a model." |

OpenAI's Agents SDK documentation lists Temporal, Dapr, Restate and DBOS as durable
execution integrations. Orkes, Mastra and a Devoxx talk also use the label. So
the term spread across workflow vendors within about a year, and each defines it as a
checkpointed agent loop that resumes after a failure.

**The sense in Pi Durable.** The `durable-pi` README defines it the same way, with one
addition: "A durable agent commits all of it (history, queued input, task checkpoints and
application state) to storage **before** anything is shown." It contrasts this with
`/resume`, which "restores a chat transcript". The README adds that the device owns the
run and clients only watch and steer. The vendors above use the mechanism, and the
device-owns-the-run framing is the README's own.

## Proactive agents

**Line 1: agent theory, 1995.** Wooldridge and Jennings, "Intelligent agents: theory and
practice", Knowledge Engineering Review 10(2). Their weak notion of an agent has four
properties: autonomy, social ability, reactivity and pro-activeness. Pro-activeness is
goal-directed behaviour by "taking the initiative". The paper is at
[Oxford](https://www.cs.ox.ac.uk/people/michael.wooldridge/pubs/ker95/subsubsectionstar3_1_1_1.html).
Here every agent is proactive by definition. The term is a property, not a product class.

**Line 2: LLM agents, 2024 to 2025.** The LLM sense contrasts with a chatbot that waits
for a prompt.

- [Proactive Agent: Shifting LLM Agents from Reactive Responses to Active Assistance](https://arxiv.org/abs/2410.12361),
  Lu and 14 co-authors, submitted 2024-10-16, accepted at ICLR 2025. It builds
  ProactiveBench, 6,790 events, to score an agent that offers help without being asked.
- LangChain's [Introducing ambient agents](https://www.langchain.com/blog/introducing-ambient-agents),
  Harrison Chase, 2025-01-14. Ambient agents "listen to an event stream and act on it
  accordingly". The post uses neither "proactive" nor "durable". A post by swyx on
  2025-01-14 says Chase had earlier called them proactive agents. The post could not be
  fetched, so this rests on a search snippet.
- The Stack Overflow podcast of 2025-10-14, "AI agents for your digital chores". Dhruv
  Batra of Yutori: "we're building proactive agents", and "the proactive nature comes in
  in the monitoring aspect of it". The host frames it as "proactive agents instead of
  reactive agents".

**The sense in `durable-pi`.** The README: "Proactive agents start working before you
sit down. They don't wait for you to work. They work for you, then wait." Its table
defines them as "Agents that start from an event (a log row, a webhook, a schedule) and
have the work ready when you arrive". That is the ambient agent pattern from LangChain
and the monitoring sense Yutori describes. The README adds a stop at an approval: the
agent works, then waits for a person to approve a deploy.

## How the two terms relate

The README makes them one design: a cheap classifier in front of an event stream starts a
durable agent, which does the first hour of work and waits for approval. The terms name
different properties.

| Term | Property | What it needs |
|---|---|---|
| Proactive | What starts the run: an event, not a prompt | An event source and a trigger |
| Durable | How the run survives: checkpoints, not a transcript | A store that commits each step |

A proactive agent that is not durable loses its run when the process dies. A durable
agent that is not proactive waits for a prompt. IndyDevDan's contribution, as far as the
sources show, is the pairing and its name, not either term.

## Relevance to tsk

- tsk's thread continuation addresses the durable property across actors and machines.
  The vendors address it inside one workflow engine.
- tsk has an external event queue in the ledger (`efrit.md`). An event that starts work
  from that queue is the proactive pattern. No tsk document uses the label "proactive".
- The comparison with Pi Durable is in `tsk-market-position-analysis.md`.

## Not established

- Who first wrote "durable agents". The earliest dated use found is the Temporal and
  OpenAI release of 2025-07-30, and the press release text was not read.
- Whether "durable agents" appears before July 2025. No search targeted 2024 pages.
- IndyDevDan's video, [Pi Durable PROACTIVE Agents](https://youtu.be/DNDYYHsypZg). It
  was not watched. The README is the source for his usage, and the video may attribute
  the terms.
- The swyx post on Chase's earlier term. X returned HTTP 402.
- Whether Yutori coined its sense of "proactive agents". Its launch material was not
  found.
