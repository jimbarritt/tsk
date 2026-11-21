# Yegge on pace, factories and chiefs of staff, mapped to tsk

Steve Yegge's position has three parts: pace agents to human speed, build a software factory only when you need one, and give each person a chief-of-staff agent. tsk's design matches all three. Its missions give people and agents one shared briefing that a person understands, and it has the visibility, the single point of interaction and the separation of work records from code that the three parts need. A limit on the pace of an agent, a scope of authority for an actor, and messages between agents are planned and not yet built.

Status: analysis note, 2026-11-21. The two mapping sections are the note's own inference.

Sources: a post by Steve Yegge on X ([post](https://x.com/steve_yegge/status/2121671447810888185)), read on 2026-11-21, and Martin Fowler's [Fragments: November 20](https://martinfowler.com/fragments/2026-11-20.html), which quotes it. The summary is in the Yegge section of [tsk-market-position-analysis.md](tsk-market-position-analysis.md).

## Yegge's position

- **Pace.** Pace AI output to human speed. Engineering keeps to engineering speed, and business is slower still. Teams that push too hard see downtime.
- **What went wrong for him.** His agents overwhelmed his players with features, his accountant with emails, his merge queue with tickets and his machines with load. They angered his admins, made messes in his game and launched broken open source software without permission. A forgiving community is the reason his operation survived it.
- **Factories.** A factory needs an extraordinary amount of upkeep in time and tokens. Yegge rewrote his previous five, because rewriting became cheaper than fixing. A factory becomes specific to its domain, because it supports the product as well as building it. His advice is to build one, not buy one, and only when you need one.
- **Chiefs of staff.** Each person will have a chief-of-staff agent, or an entourage of helper agents, and will bring it to work. Protocols will form for entourages that negotiate work between people. Next year brings office drama between people and agents, and between agents. His advice: build your own chief of staff first, because your entourage defines how you work with others.

Fowler adds that the speed of agents should be one people can govern. That speed rises as people learn to govern agents, and the people the agents serve set its limit.

## Where tsk matches Yegge's position

**Missions pace the work to people.** A mission is the way an agent or a human receives instructions, and it contains the state of the work: purpose, objective, constraints, decisions and questions ([ubiquitous-language.md](../../domain/ubiquitous-language.md#mission)). A briefing renders the mission for each actor, for a person as well as for an agent. People and agents work from one shared definition of the work that a person can read and agree to. The shared understanding sets the speed of the work, which is Yegge's pacing to human speed. The mission record exists today. The briefing as a rendering of the record is planned and not yet built.

**The human sets the pace.** The README states tsk's objective as scaling humans: the less supervision the work needs, the more work one human delegates ([supervisory-engineering-and-the-record-of-supervision.md](../supervisory-engineering-and-the-record-of-supervision.md)). That is Fowler's limit: the work one person can supervise bounds the pace, and the bound rises as supervision needs less of the person. tsk's bootstrap runs in supervised mode, with one human in every position.

**The ledger is the visibility.** A person learns what a robot did from the pause entry of its thread, the task list, the decisions and the open questions of a mission. These are records a person reads without reading every diff. Fowler's risk is a person who loses sight of the agents. The ledger is tsk's answer to it.

**A chief of staff needs a single point of interaction.** To work across many threads, a person needs one place to talk to the work. In tsk that place is a session on the nexus's own ledger, which covers every repo in the nexus. An agent starts a new harness when asked, and a regenerated session keeps its robot name and thread. So a chief of staff session can start the sessions that do the work.

**Work records sit outside the factory's code.** Yegge rewrote five factories. tsk keeps the state of the work in a ledger that is separate from the code repo (ADR 0035): missions, decisions, questions and continuation entries. A rewrite of a factory's code does not change these records. This was not tested on a rewrite.

**Build, not buy.** The market analysis's verdict is that tsk is not a head-to-head orchestrator, and its premise is that its model does not depend on one vendor's harness. A bespoke factory of Yegge's kind can use tsk as a component for its records. That matches his advice better than a factory product would.

**A studio builds and supports one product.** Yegge's "company in a box" builds the product and supports it. The draft term in [robot-studio-not-a-factory.md](../robot-studio-not-a-factory.md) for a group of robots and people that build a product is a robot studio, not a factory. The two terms name the same kind of unit.

## What tsk has planned and not yet built

**A limit on pace.** Budgets are planned on the autonomous coordination mission (M-TSK-AUTO). Today a briefing's Budget and Execution constraints are text, and no component enforces them. In Claude Code, ultracode switches off the warning for a large workflow and the limit on concurrent subagents for a session. Yegge's failures are of this kind: too many tickets, emails, features and load. Until budgets are enforced, tsk records such failures after the fact.

**A scope of authority.** Yegge's agents launched software without permission. The Post, a standing position with a scope of authority and standing orders, is named in the ubiquitous language and planned for the point where agents run autonomously. It is not modelled yet. A chief of staff is closest to a Post that a robot fills for one person, so the chief of staff builds on that model.

**Messages between agents.** A chief of staff coordinates other agents through messages. The robot communication protocols mission (M-TSK-COMMS) has tasks for an event stream in the ledger, liveness of a harness instance, and task claims that name the actor. Until it is built, a robot sends information to another robot through a person or through the ledger records.

**Entourages of different people and teams.** Yegge's protocols are between the entourages of different people. In tsk a nexus is for a group of actors: a team has its own nexus, and a person has a personal one ([personal-and-team-territories.md](../../domain/personal-and-team-territories.md)). A nexus has a related-nexus list that links it to the nexuses of other people and teams, designed and not built ([ubiquitous-language.md](../../domain/ubiquitous-language.md#nexus)). The links between nexuses are where the entourages of different people and teams connect, and the planned communication layer sends the messages between them.

**A stop rule for agents that message each other.** Evan Ratliff's company of AI agents, HurumoAI, shows the drama Yegge predicts: agents that replied to each other in Slack until the credits of their agent platform ran out, and an agent that invented its own history and kept it as memory. Ratliff's fix was a limit on replies per conversation. The planned budgets on the autonomous coordination mission cover this case, and the ledger has a note on HurumoAI linked to that mission.

**A record of supervision.** A chief of staff that supervises other robots gives feedback. Supervisor robots, reviews of a robot's own account, and an audit history of the feedback a supervisor gives are planned (M-TSK-AUTO and M-TSK-IDEA). Today tsk records the work, not the feedback.

## Not established

- Any reply in Yegge's thread after the post itself.
- Whether a tsk ledger stays usable after a rewrite of a factory's code.
- Whether a chief of staff session can supervise robots in other sessions before the messaging between agents exists.

## References

- Steve Yegge, post on X, before 2026-11-20. https://x.com/steve_yegge/status/2121671447810888185
- Martin Fowler, "Fragments: November 20", 20 November 2026. https://martinfowler.com/fragments/2026-11-20.html
- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): the Yegge section, the Seats section and the three-part verdict.
- [supervisory-engineering-and-the-record-of-supervision.md](../supervisory-engineering-and-the-record-of-supervision.md): the objective of scaling humans, and the missing record of supervision.
- [robot-studio-not-a-factory.md](../robot-studio-not-a-factory.md): the robot studio term.
- [yegges-levels.md](../yegges-levels.md): Yegge's ladder from writing code to running an orchestrator.
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): Actor, Thread continuation and Post.
