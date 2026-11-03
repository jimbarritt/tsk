# Robot studio, not factory: Rob West on the factory metaphor

Status: research note, read 2026-11-03. Rob West argues that the factory metaphor fits the production parts of software work and misleads for the rest, which is design. Jim's draft name for a group of robots and humans that build software together is **robot studio**.

Raised by Jim, 2026-11-03, who read the article and asked what to call a collection of robots and humans collaborating to create software, if not a factory.

Sources: Rob West, "The Agentic Software Factory is the Wrong Metaphor", 19 September 2026 ([article](https://robwest.info/articles/the-agentic-software-factory-is-the-wrong-metaphor/)), read through a summarising model, so the quotations are short. The works it cites were not read.

## The argument

**A metaphor contains a theory.** A factory turns inputs into outputs through a repeatable process. Improvement in a factory means higher throughput, less variation and more automation. A leader who adopts the metaphor optimises utilisation, standardises inputs, counts outputs, and treats a specification as raw material.

**Software already has a factory.** West cites Jack Reeves (1992): the source code is the design, and the compiler and linker do the building. Programming, testing and debugging are design activities. The build and deploy pipeline is the factory, and it is already automated. A coding agent works on the design side of that line, so it is closer to a designer than to a robot on an assembly line.

**Four places where the metaphor fails:**

1. A factory starts once the drawing exists. A ticket is not an engineering drawing, and the metaphor hides the design work.
2. Requirements are hypotheses, not raw materials. Building produces information that changes what was wanted.
3. A factory removes variation. Design gains from different approaches, compared and selected.
4. A factory needs a steady queue of precise work orders. That pushes teams to turn uncertainty into tickets too early. An agent then resolves the ambiguity with a plausible guess, and produces a well-tested implementation of the wrong thing.

**Where a factory is right.** Builds, environment provisioning, test runs, dependency upgrades, migrations, formatting, static analysis and some remediation are production work. West's position: build factories for those parts, and do not mistake the factory for software engineering.

**Agents as design collaborators.** The main value of an agent is cheap experimentation: a hypothesis, an implementation, a test and a revision, faster than a human. The scarce resources become understanding the domain, clarifying intent, choosing architecture, making trade-offs, verifying behaviour, and judging whether the result solves the customer's problem. West's closing phrase is "design engineering at machine speed".

**Evidence he cites.** The 2025 DORA report describes AI as "an amplifier" of an organisation's strengths and weaknesses. A DX report for the second quarter of 2026 found that AI accelerates output but not innovation.

## How the argument relates to tsk

This section is the note's own inference.

**The ledger records design work.** A mission's purpose, objectives, decisions and open questions are records of discovery, not of throughput. A continuation entry records where a line of thought stands. These fit West's design side.

**The task is the work order.** A task with a claim is the one factory-shaped record. West's fourth failure is the risk: a mission broken into tasks before its questions are answered. The model has a question record for that uncertainty, and nothing stops a task being written too early.

**The evidence fits the line.** In the large runs recorded in M-TSK-SCALE, the runs that worked had a reference before they started: Bun's test suite, GCC for the compiler. Those were production-shaped tasks. The browser run, the most design-shaped, did not build on its release day.

**An adaptable robot puts the line inside the task.** Jim's point, 2026-11-03: robots that can be programmed to build anything blur the factory distinction, unlike fixed robots on a car line. West's line is about whether the output is known before the work starts, not about how flexible the worker is. A general robot crosses that line many times a day: a migration in the morning, an architecture spike in the afternoon. So the line sits inside each task. A task either has a reference that exists before it starts, or it does not, and the two kinds need different supervision. That is the split between deterministic and inference checks in [adversarial-and-sceptical-review-agents.md](adversarial-and-sceptical-review-agents.md).

## The name: robot studio (draft, Jim, 2026-11-03)

Jim's draft name for a collection of robots and humans that create software together is **robot studio**. His reason: a studio implies the production of a thing, the product, which is the overarching goal. It fits West's claim that the work is design. The name is a draft and is not used in the README or the domain model yet.

The candidates considered, kept for reference:

- **Studio.** The design professions' word: a design studio, an architecture studio. Chosen as the draft.
- **Practice.** As in an architecture practice or a law practice: professionals who apply judgment to varied work.
- **Job shop.** From operations management, where it is set against a production line that makes one design at volume. A job shop makes varied items to order with general-purpose machines and skilled operators. It keeps the manufacturing lineage and meets West's objection in his own terms.
- **Workshop.** The craft lineage: people and tools together on varied work.
- **Lab.** Fits West's cheap experimentation and the objective of M-TSK-SCALE.
- **A military term,** such as unit, detachment or operation. It would match tsk's doctrine vocabulary of missions, objectives and actors.

## Not established

- The content of the sources West cites, including Reeves (1992), the DORA 2025 report and the DX report.
- Whether "robot studio" replaces "software factory" elsewhere in the repo. That is a decision for Jim.

## References

- Rob West, "The Agentic Software Factory is the Wrong Metaphor", 19 September 2026. https://robwest.info/articles/the-agentic-software-factory-is-the-wrong-metaphor/ (read through a summariser).
- Cited by the article, not read for this note: Jack Reeves, "What Is Software Design?", C++ Journal, 1992. Martin Fowler, "The New Methodology". Dave Farley, Modern Software Engineering. DORA, "State of AI-assisted Software Development", 2025. DX, "State of AI Impact in Engineering", Q2 2026. Software Engineering Institute, "Taking DevSecOps to the Next Level with Value Stream Mapping". BCG Platinion, "The Agentic Software Factory". Zach Lloyd (Warp), "We are now factory engineers, not product engineers", June 2026. Factory, "Factory 2.0: From coding agents to software factories", June 2026.
