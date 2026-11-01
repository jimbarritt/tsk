# Supervisory engineering work, and what tsk records of supervision

Status: research note, read 2026-11-01. Annie Vella applies the human factors research on supervisory control to coding agents, and her model fits tsk's objective. tsk records the work a supervisor reads, and records nothing of the supervision itself. The note also records the provenance of the term "dreaming".

Raised by Jim, 2026-11-01, who asked where the article fits. Jim then named two ideas that this note keeps as future ideas: a robot can be a supervisor, and tsk needs an audit history of the feedback that supervisor agents gave to other robots.

Sources: Annie Vella, "Supervisory Engineering Work", 23 October 2026 ([article](https://annievella.com/posts/supervisory-engineering-work/)), and the arXiv paper it reports ([2605.23135](https://arxiv.org/abs/2605.23135), abstract read). The article was read through a summarising model, so the paraphrases are reliable and the quotations are short. The older papers it cites were not read. They are listed under References as the article cites them.

## What the article argues

Vella argues that AI changes the job of a software engineer, because the engineer moves from creating code to supervising the AI that creates it. Her Masters research found a "creation-to-verification shift": engineers spend less time writing code and more time testing and reviewing ([arXiv:2605.23135](https://arxiv.org/abs/2605.23135)). The paper names the new category "supervisory engineering work": directing AI, evaluating its output, and correcting it. In the survey, 82% of respondents reported less time writing code.

She argues that the claim "AI creates demand for engineers" misses the point. The job is a different one, so it needs different skills, training and hiring.

## The model the article borrows

**Supervisory control.** Thomas Sheridan and colleagues at MIT developed the field from the late 1960s. In direct control the human closes the control loop. In supervisory control the computer closes it, and the human sets its goals from time to time. Vella writes that "supervisory engineering work is supervisory control applied to software development". Before coding agents, programming was direct control. A coding agent closes the loop itself.

**Five supervisor roles.** The article attributes to Sheridan and Verplank (1978) five roles: "plan, command, monitor, intervene and trust". It maps them to directing the AI (plan and command), evaluating its output (monitor), correcting it when it is wrong (intervene), and calibrating trust. The list of roles in Sheridan's own work is commonly given with "teach" and "learn" in place of "command" and "trust". This note did not read the 1978 report, so the wording of the roles is the article's.

**Five levels of automation.** Endsley and Kiris (1995) narrowed Sheridan and Verplank's ten levels to five. The article maps them to AI-assisted development:

| Level | In AI-assisted development |
|---|---|
| None | No AI assistance |
| Decision support | The early autocomplete of coding assistants |
| Consensual AI | Coding agents where you must approve each action |
| Monitored AI | Auto mode, with the ability to veto the agent's actions |
| Full automation | Full auto mode, such as in a dark software factory |

Vella adds one difference from the older research. The earlier studies assumed that the computer's account of its own actions was accurate. A coding agent may report something other than what it did, so the supervisor needs a source other than the agent's own report.

**Sharing and trading.** Sharing is the AI as a copilot that works beside the human. Trading is the human handing over a whole task, sometimes for good.

## What fails at higher levels

Endsley and Kiris found that situation awareness fell as automation rose. Operators noticed changes and understood them less. They were slower to take over by hand after a failure, although they still decided well. Their workload did not fall, probably because monitoring added load.

Vella links this to code. Writing code builds a mental model, and reviewing a diff builds less understanding. She cites Margaret-Anne Storey on cognitive debt and Addy Osmani on comprehension debt.

Lisanne Bainbridge's "Ironies of Automation" (1983) describes monitoring as "very boring but very responsible". The most capable automation needs the best-trained operator, because the operator rarely practises the skill that a failure requires.

Parasuraman and Riley (1997) warn that the operator's role can become a by-product of the automation. They argue that the role should follow the operator's responsibilities and capabilities.

## What the article proposes

Vella proposes that the industry redesign the job and the system around it together:

- Choose the level of automation for each type of work.
- Design system signals that support situation awareness, so that "review harder" is not the plan.
- Describe when and how a human intervenes, and what the human is accountable for.
- Train people for supervision, especially for the rare intervention.

She announces a follow-up post on treating the software development life cycle as a control system that people build and supervise.

## How the article relates to tsk

**The objective.** The README states the objective of tsk as scaling humans: the less supervision the work needs, the more work one human delegates. In Vella's terms, a rise in the level of automation. Her evidence adds a constraint. A higher level needs better signals for the supervisor and trained interventions, not less of both.

**The ledger as a system signal.** The signal she asks for is what the supervisor reads to regain situation awareness without reading every diff. In tsk that is the pause entry of a thread, the task list, the decisions and the open questions of a mission. A supervisor reads these to learn what a robot did, why, and what is undecided.

**Trust has no record.** Calibrating trust is one of the five roles, and the ledger has no record of it. The review stage that [adversarial-and-sceptical-review-agents.md](adversarial-and-sceptical-review-agents.md) describes is a mechanism that calibrates it, with deterministic checks first and a model reviewer second.

**The experiment at the top level.** Mission M-TSK-SCALE sets out to build or rewrite a system autonomously. That is her level of full automation. The article predicts the failure points: the supervisor loses situation awareness and takes over slowly. The experiments can measure both.

## A robot as supervisor (Jim, 2026-11-01)

Jim's point: a sceptical agent can run a feedback loop inside the factory and escalate to a human supervisor only when needed. A robot then has a supervisor role.

This changes the shape of the five roles. Monitor and intervene can belong to a robot, and the human takes the role of the supervisor of supervisors. Escalation is the human's intervene role, called by a robot.

tsk's model already allows it. An actor is a human or a robot, and a task or thread can name any actor. The role is not explicit in the ledger.

## What tsk records of supervision

Today the ledger records the work. Decisions record the choices a person made, and a question records an open point and its answer. A pause entry records the state of a thread. The ksobr domain has run records, mission reports and session logs.

No record exists of the supervision process: what feedback a supervisor gave to a robot, what the robot did with it, and when a human was called in. A decision may record part of it, but a decision is a choice about the work and does not record review feedback.

**A future idea, not decided (Jim, 2026-11-01):** an audit history of the feedback that supervisor agents gave to other robots. It would be the evidence trail of supervision. It needs the communication layer of M-TSK-COMMS, so that several agents can interact without a human supervisor in each exchange. Until that layer exists, a robot cannot give feedback to another robot except through a human or through the ledger records.

Questions this idea would have to settle, none answered here:

- Which record stores a piece of feedback, and which robot, session and task it concerns.
- Who may supervise whom, and how a robot's feedback is attributed in the same way as a robot's commit.
- What triggers escalation to a human, and what the human answers with.

## Dreaming

Jim, 2026-11-01, hears people say that agents "dream" and asked for the origin of the term. In 2026 the agent community uses it for one thing: an offline job that reads past session transcripts and a memory store, then merges, prunes and rewrites the memory between sessions. This note found no source for who first used the word for agent memory. The marks below show which sources were read and which are search summaries.

### Where the word appears in 2026

- **Anthropic Managed Agents "Dreams".** The official documentation describes a dream that "reads an existing memory store alongside past session transcripts, then produces a new, reorganized memory store". The inputs are one memory store and 1 to 100 past sessions. The output is a separate store, and "the input store is never modified". It is an asynchronous job that takes minutes to a few hours, in research preview, with access by request ([Dreams docs](https://platform.claude.com/docs/en/managed-agents/dreams), read). The announcement date conflicts: the Anthropic blog post is dated 2026-05-19 ([blog](https://claude.com/blog/new-in-claude-managed-agents), read) and SiliconANGLE reports it on 2026-05-06 ([article](https://siliconangle.com/2026/05/06/anthropic-letting-claude-agents-dream-dont-sleep-job/), read). This note does not resolve the conflict.
- **OpenClaw "Dreaming".** The documentation describes a "background memory consolidation system" in `memory-core`, with three phases named light, REM and deep. Only the deep phase writes `MEMORY.md`. It runs on a cron schedule, by default at 03:00, and it writes a diary file, `DREAMS.md` ([OpenClaw docs](https://docs.openclaw.ai/concepts/dreaming), read). A search summary dates its general availability to release v2026.4.5. The docs show `enabled: true` as the default, and a search summary describes it as opt-in. The conflict is unresolved.
- **Letta Code "Dreaming".** The documentation states that "Dreaming uses background subagents to review recent conversations". It is set with `/sleeptime`, and the memory is stored in git ([Letta Code docs](https://docs.letta.com/letta-code/memory), read).
- **Claude Code "auto dream".** A third-party article dated 2026-03-25 describes a background sub-agent that consolidates memory files in four phases, runs only after 24 hours and more than five sessions, and can be started with `/dream` ([article](https://www.implicator.ai/anthropic-adds-auto-dream-to-claude-code-fixing-memory-decay-between-sessions), read). The article states that Anthropic had announced nothing and that no documentation existed. The Claude Code memory page that this note read does not mention it. The name belongs to the community in the sources found.

### Older sources behind the word

The 2026 uses borrow the word. No source found states that a vendor chose it from these papers. The link is the commentators' analogy.

- **Sleep and memory in neuroscience.** Wilson and McNaughton (1994) found that place cells that fired together during a task fired together again in later sleep ([PubMed 8036517](https://pubmed.ncbi.nlm.nih.gov/8036517/), search summary). Diekelmann and Born (2010) review the memory function of sleep ([Nature Reviews Neuroscience](https://www.nature.com/articles/nrn2762), bibliographic record only). This is the closest source to the agent-memory sense.
- **Replay.** Lin (1992) introduced experience replay ([DOI 10.1007/BF00992699](https://doi.org/10.1007/BF00992699), record only). Shin and colleagues (2017) use a generator to replay data for earlier tasks, inspired by the hippocampus ([arXiv:1705.08690](https://arxiv.org/abs/1705.08690), read).
- **Wake-sleep.** Hinton, Dayan, Frey and Neal (1995) train a generative model with alternating wake and sleep phases ([Hinton's page](https://www.cs.toronto.edu/~hinton/absps/ws.htm), search summary). DreamCoder (Ellis and others, 2020) applies wake-sleep learning to a library of abstractions and trains on "imagined and replayed problems" ([arXiv:2006.08381](https://arxiv.org/abs/2006.08381), read).
- **Imagined rollouts.** In World Models (Ha and Schmidhuber, 2018) an agent is trained "entirely within a hallucinated dream" and the policy is transferred to the actual environment ([arXiv:1803.10122](https://arxiv.org/abs/1803.10122), read). Dreamer (Hafner and others, 2019) learns behaviours from trajectories imagined in a learned world model ([arXiv:1912.01603](https://arxiv.org/abs/1912.01603), read). This sense is about planning and does not rewrite a memory.
- **Image generation.** Google's DeepDream (2015) is unrelated to memory. The dates came from a secondary source and were not verified.
- **Sleep-time compute.** Letta's April 2025 term is "sleep-time", not "dreaming" ([Letta blog](https://www.letta.com/blog/sleep-time-compute), read; [arXiv:2504.13171](https://arxiv.org/abs/2504.13171), read). The paper precomputes inferences about a context to cut the cost per query. It does not rewrite a memory store. The Letta blog describes a sleep-time agent that edits memory blocks in the background, so its function overlaps with the 2026 uses.

### What the process changes, and what it records

No weights change in any agent-memory use found. The process rewrites text. Anthropic describes it as "a synthesis pass over the inputs, not an editor applied to the text of the store".

Records of the change differ:

- **Anthropic.** The input store stays unchanged and the output is a separate store, so a person reviews it and uses or discards it. Every change to a memory creates an immutable version, kept for 30 days, with an endpoint that redacts content ([Memory docs](https://platform.claude.com/docs/en/managed-agents/memory), read).
- **OpenClaw.** The previous `MEMORY.md` is saved before an accepted rewrite. `DREAMS.md` records the counts of entries added, merged and superseded, with short diff highlights.
- **Letta Code.** Memory updates are committed to git. An optional setting makes a second agent review the update before it applies.
- **Claude Code auto dream.** No record is documented. The third-party article reports that contradicted facts are deleted with "no flag, no comment".

None of these records a typed reason for each edit. They record the result: a version, diff counts or a commit. This is relevant to the audit history of supervision above, because dreaming is a process that changes a robot's memory without a human reading each change.

### Risks the sources name

- **Silent deletion.** A GitHub issue, seen only through a search summary, reports that auto dream deleted 23 memory files in about a day with no record of what or why. It is a single unverified report.
- **Transcripts not masked.** The OpenClaw documentation states that secret values in transcripts are not masked before consolidation.
- **Injected content that persists.** Among the sources read, only OpenClaw removes candidates with untrusted provenance before consolidation.
- **Cost.** Anthropic states that cost grows with the number and length of the input sessions, and that a failed dream produces a partial output store.
- **Weak instructions.** Anthropic states that instructions aimed at specific lines "generally produce no change".

### How dreaming relates to tsk

This part is the note's own inference. tsk's memory is the ledger: typed records written by commands, with each change recorded in the ledger history. A dreaming process would be a robot that rewrites records or documents without a person reading the change. Anthropic's split of an input store from a reviewable output store is the nearest model to tsk's own habit of writing locally and publishing at set points.

## Not established

- The wording of the five roles in Sheridan and Verplank's 1978 report. The note quotes the article.
- The content of the arXiv paper beyond its abstract.
- Whether the follow-up post on the control system has appeared.
- Who first used "dreaming" for agent memory, and when. Reddit, Hacker News and Mem0, Zep, Cursor and OpenAI memory features were not searched.
- Whether Claude Code records anything about an auto dream run, and whether Anthropic documents the feature.

## References

- Annie Vella, "Supervisory Engineering Work", 23 October 2026. https://annievella.com/posts/supervisory-engineering-work/ (read through a summariser).
- Annie Vella and Kelly Blincoe, "The Impact of AI Coding Assistants on Software Engineering: A Longitudinal Study", arXiv:2605.23135, 22 May 2026. https://arxiv.org/abs/2605.23135 (abstract read).
- Cited by the article, not read for this note: Sheridan and Verplank (1978), undersea teleoperators, https://apps.dtic.mil/sti/citations/tr/ADA057655. Sheridan, Telerobotics, Automation, and Human Supervisory Control (MIT Press, 1992), https://mitpress.mit.edu/9780262515474/telerobotics-automation-and-human-supervisory-control/. Endsley and Kiris (1995), https://journals.sagepub.com/doi/10.1518/001872095779064555. Parasuraman and Riley (1997), https://journals.sagepub.com/doi/10.1518/001872097778543886. Bainbridge, "Ironies of Automation" (1983), https://www.complexcognition.co.uk/2021/06/ironies-of-automation.html. Storey, "Cognitive debt", https://margaretstorey.com/blog/2026/02/09/cognitive-debt/. Osmani, "Comprehension debt", https://addyosmani.com/blog/comprehension-debt/.
- Vella, "The middle loop", https://annievella.com/posts/the-middle-loop/. Thoughtworks, "Supervisory engineering", https://www.thoughtworks.com/insights/blog/agile-engineering-practices/supervisory-engineering-orchestrating-software-middle-loop.
