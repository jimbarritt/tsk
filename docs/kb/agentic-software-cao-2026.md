# Agentic Software (Cao, 2026): agents as software, and the retreat from "The End of Software Engineering"

Status: research note, read 2026-11-02. A single-author preprint argues that an AI agent is itself software, with its decision logic generated at runtime, and that the human role moves to intent, coordination and audit. Its first version claimed the end of software engineering. Its second version, six days later, retracted that claim and changed the title.

Raised by Jim, 2026-11-02, who found it through a LinkedIn post by Rubén Domínguez Ibar and asked for the paper to be read.

Sources: the paper, arXiv [2606.05608](https://arxiv.org/abs/2606.05608), version 2 read in full from its PDF text, and version 1 compared with it paragraph by paragraph. The LinkedIn post was read through a summariser. The works the paper cites were not read, so every figure from them is the paper's report of it.

## The paper

Zhenfeng Cao, `Agentic Software: How AI Agents Are Restructuring the Software Paradigm`, 15 pages, subjects cs.SE and cs.AI. The author's affiliation is a company, Lingxi Intelligent Investment (Shenzhen) Development Co., Ltd. The paper is a preprint with no peer review.

Version 1, dated 4 June 2026, was titled `The End of Software Engineering: How AI Agents Are Fundamentally Restructuring the Software Paradigm`. Version 2 is dated 10 June 2026.

## What the paper argues

**Two kinds of software.** In a traditional system, the paper defines a tuple of computational resources, decision rules written in advance, and an execution environment. All decision logic is written by humans before any input arrives. In an agentic system, a language model is the reasoning engine, with tools, memory and a planner. The model "generates the decision logic at runtime". The code it writes is "a transient artifact", produced and discarded as needed.

**A complexity argument.** The number of possible interaction topologies among n components grows as 2 to the power n(n-1)/2, while human cognitive capacity is a constant. The paper concludes that an agent whose capacity grows with training compute "decouples solution capacity from human cognitive limits". The paper states the premise and does not measure it.

**Three generations of delivery.** Licensed software, SaaS and Agent-as-a-Service. Each step transfers complexity from the user to the vendor. The last step transfers decision-making as well. The paper argues that "AI to software to result" keeps the human on the critical path, and that "agent to result" removes the separate artifact.

**Agentic Engineering.** The paper credits LangChain with introducing the term in April 2026, as "a multi-agent coordination model where AI agents function as digital team members, each with defined roles, shared memory, and a unified observability layer". The LinkedIn post credits Cao with coining it. The paper does not claim that. Its contribution is the framing as an expansion of software engineering.

**The human role.** The paper names four differentiators: intent articulation, architectural oversight, quality calibration and ethical governance. Practitioners become "intent architects, agent coordinators, and outcome auditors". The paper recommends a posture of "human-in-the-loop, agent-in-the-driver's-seat": agents own execution, and humans own intent, critical judgment and ethical oversight.

**Four stages.** Each stage has a human role:

| Stage | Agent capability | Human role |
|---|---|---|
| I. Tool-augmented (2023 to 2025) | Completion, single-issue fixes | Author and reviewer |
| II. Single-task autonomous (2025 to 2027) | End-to-end features, debugging | Intent architect and auditor |
| III. Multi-agent teams (2026 to 2029) | Coordinated swarms, full lifecycle | Product manager, architect and auditor |
| IV. Self-evolving ecosystems (2028 onward) | Autonomous discovery and self-modification | Goal setter and ethics governor |

## The evidence the paper uses

- **SWE-bench Verified.** A 72B open model resolves 30.20% of issues, against 31.80% for GPT-4o. The source is a 2024 paper, so the figure is not from a 2026 model.
- **A LangChain pilot.** Coordinated agent swarms on more than 20 enterprise debugging workflows cut the time to root cause by 93%, saving over 200 engineering hours in a month. The paper states that the gain came from orchestration: shared context, parallel investigation and cross-validation. The source is a vendor blog post.
- **Hermes Agent.** An open-source framework with over 179,000 GitHub stars, cited for skills that the agent creates and patches itself. The paper gives stars as evidence of use.
- **EvoClaw.** Performance falls from "above 80% on isolated tasks to at most 38% in continuous settings", over 12 frontier models and 4 agent frameworks. The paper names four causes: context drift, error propagation, no awareness of technical debt, and incomplete verification. The paper concludes that autonomous development needs "several more years" of research.

## What changed between version 1 and version 2

| | Version 1 | Version 2 |
|---|---|---|
| Title | The End of Software Engineering | Agentic Software |
| Second claim | "Paradigm Shift, Not Optimization": agents eliminate "the software artifact as a necessary intermediary" | "Software Redefined, Not Replaced": the agent `is software, albeit of a fundamentally different kind` |
| Section 3.3 | "Eliminating the Intermediary" | "The Agent as Software" |
| Agentic Engineering | "A New Discipline", distinct from software engineering | "Expanding the Discipline", an expansion of it |
| Section 2.4 | "Why Agents Inevitably Scale Better" | "Why the Agentic Paradigm Scales Differently" |
| Complexity bound | grows "exponentially", with a formula that does not match its own derivation | grows "super-exponentially", with the formula corrected |
| Conclusion | agents eliminate the static software artifact | "does not eliminate software", "the old software engineering is not ending" |

The tables of generations, the human roles, the evidence and the roadmap are unchanged. A citation of the paper needs version 2. A citation of the post's framing, "the end of software engineering", cites a claim the author withdrew.

## Limits of the paper

This section is the note's own assessment.

- The paper is a single-author preprint, and the evidence is other people's results.
- Three of the four data points are old, vendor-reported or a measure of popularity: a 2024 benchmark figure, a vendor pilot with no stated method in the paper, and a count of GitHub stars.
- The premise of the complexity argument, a fixed human capacity against a model capacity that grows without a limit, is stated and not tested.
- The paper's best evidence, EvoClaw, limits its stronger claims. Sustained work is where agents fail.
- The roadmap dates are forecasts and are not sourced.

## How the paper relates to tsk

This section is the note's own inference.

- **The control plane.** The paper quotes LangChain: agentic engineering "maintains long-term memory across agents, and manages state and traceability across the full software delivery lifecycle". That describes what the ledger records: missions, tasks, decisions, and a continuation entry for each pause.
- **The human roles.** The paper's roles in each stage match the levels of automation in [supervisory-engineering-and-the-record-of-supervision.md](supervisory-engineering-and-the-record-of-supervision.md). Stage I is the consensual level, stage II the monitored level, and stages III and IV the full level. The paper starts from the product side and names an auditor. Vella starts from human factors and names a supervisor.
- **A baseline for M-TSK-SCALE.** The EvoClaw drop from over 80% to at most 38% is a published baseline for sustained work. An experiment in M-TSK-SCALE can measure the same drop on a run that tsk coordinates, and test whether the ledger narrows it.
- **The four causes.** Context drift is the problem that thread continuation entries address. Incomplete verification is the problem of the review stage in [adversarial-and-sceptical-review-agents.md](adversarial-and-sceptical-review-agents.md). Error propagation and technical debt awareness have no tsk record today.
- **The auditor has no object.** The paper gives humans the role of "outcome auditor" and defines no record of the audit. This is the gap that the audit history of supervision names.

## Not established

- The contents of the cited works: EvoClaw (arXiv 2603.13428), the LangChain blog post and the Lingma paper.
- Whether the 93% pilot result has a stated method.
- Why the author withdrew the headline claim. The paper gives no reason.

## References

- Zhenfeng Cao, `Agentic Software: How AI Agents Are Restructuring the Software Paradigm`, arXiv:2606.05608, v1 on 4 June 2026 and v2 on 10 June 2026. https://arxiv.org/abs/2606.05608 (both versions read as PDF text).
- Rubén Domínguez Ibar, LinkedIn post on the paper. https://www.linkedin.com/posts/rubendominguezibar_a-paper-on-arxiv-is-titled-the-end-of-software-share-7522622476661452800-NnTB/ (read through a summariser, behind a login wall).
- Cited by the paper, not read for this note: R. Kumar and P. Ramagopal, `Agentic Engineering: How Swarms of AI Agents Are Redefining Software Engineering`, LangChain Blog, April 2026, https://www.langchain.com/blog/agentic-engineering-redefining-software-engineering. G. Deng and others, "EvoClaw: Evaluating AI Agents on Continuous Software Evolution", arXiv:2603.13428. Y. Ma and others, "Lingma SWE-GPT", arXiv:2411.00622. Y. Wang and others, `Agents in Software Engineering: Survey, Landscape, and Vision`, arXiv:2409.09030. Nous Research, Hermes Agent, https://github.com/NousResearch/hermes-agent.
