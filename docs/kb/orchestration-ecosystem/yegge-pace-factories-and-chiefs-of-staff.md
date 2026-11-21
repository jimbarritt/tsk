# Yegge on pace, factories and chiefs of staff, mapped to tsk

Steve Yegge's position has three parts: pace agents to human speed, build a software factory only when you need one, and give each person a chief-of-staff agent. tsk's design matches all three, and has the visibility, the single point of interaction and the separation of work records from code that they need. tsk lacks a limit on the pace of an agent, a scope of authority for an actor, and messages between agents.

Status: analysis note, 2026-11-21. The analysis sections are the note's own inference, except where a paragraph names Jim's position.

Sources: a post by Steve Yegge on X ([post](https://x.com/steve_yegge/status/2121671447810888185)), read from screenshots that Jim supplied on 2026-11-21, and Martin Fowler's [Fragments: November 20](https://martinfowler.com/fragments/2026-11-20.html), which quotes it. The summary is in the Yegge section of [tsk-market-position-analysis.md](tsk-market-position-analysis.md).

## Yegge's position

- **Pace.** Pace AI output to human speed. Engineering keeps to engineering speed, and business is slower still. Teams that push too hard see downtime.
- **What went wrong for him.** His agents overwhelmed his players with features, his accountant with emails, his merge queue with tickets and his machines with load. They angered his admins, made messes in his game and launched broken open source software without permission. A forgiving community is the reason his operation survived it.
- **Factories.** A factory needs an extraordinary amount of upkeep in time and tokens. Yegge rewrote his previous five, because rewriting became cheaper than fixing. A factory becomes specific to its domain, because it supports the product as well as building it. His advice is to build one, not buy one, and only when you need one.
- **Chiefs of staff.** Each person will have a chief-of-staff agent, or an entourage of helper agents, and will bring it to work. Protocols will form for entourages that negotiate work between people. Next year brings office drama between people and agents, and between agents. His advice: build your own chief of staff first, because your entourage defines how you work with others.

Fowler adds that the speed of agents should be one people can govern. That speed rises as people learn to govern agents, and the people the agents serve set its limit.

## Where tsk matches Yegge's position

**The human sets the pace.** The README states tsk's objective as scaling humans: the less supervision the work needs, the more work one human delegates ([supervisory-engineering-and-the-record-of-supervision.md](../supervisory-engineering-and-the-record-of-supervision.md)). That is Fowler's limit: the work one person can supervise bounds the pace, and the bound rises as supervision needs less of the person. tsk's bootstrap runs in supervised mode, with one human in every position.

**The ledger is the visibility.** A person learns what a robot did from the pause entry of its thread, the task list, the decisions and the open questions of a mission. These are records a person reads without reading every diff. Fowler's risk is a person who loses sight of the agents. The ledger is tsk's answer to it.

**A chief of staff needs a single point of interaction.** Jim's position, 2026-11-21: he reached the chief of staff idea on his own before this post. To work across many threads, a person needs one place to talk to the work. That is one reason the nexus has its own ledger: a chief of staff session works from it across every repo in the nexus. The same need is recorded twice in the ledger, as one session where Jim discusses the work across the nexus, and as one point where Jim interacts with the network of robots. An agent now starts a new harness when asked, and a regenerated session keeps its robot name and thread. So the chief of staff can start the sessions that do the work.

**Work records sit outside the factory's code.** Yegge rewrote five factories. tsk keeps the state of the work in a ledger that is separate from the code repo (ADR 0035): missions, decisions, questions and continuation entries. A rewrite of a factory's code does not change these records. This was not tested on a rewrite.

**Build, not buy.** The market analysis's verdict is that tsk is not a head-to-head orchestrator, and its premise is that its model does not depend on one vendor's harness. A bespoke factory of Yegge's kind can use tsk as a component for its records. That matches his advice better than a factory product would.

**A studio builds and supports one product.** Yegge's "company in a box" builds the product and supports it. Jim's draft term for a group of robots and people that build a product is a robot studio, not a factory ([robot-studio-not-a-factory.md](../robot-studio-not-a-factory.md)). The two terms name the same kind of unit.

## Where tsk has gaps

**No limit on pace.** A briefing's Budget and Execution constraints are text, and no component enforces them. In Claude Code, ultracode switches off the warning for a large workflow and the limit on concurrent subagents for a session. Yegge's failures are of this kind: too many tickets, emails, features and load. tsk records them after the fact and does not stop them.

**No scope of authority.** Yegge's agents launched software without permission. tsk has no record of what an actor may do without asking. The term for it, a Post with its scope of authority and standing orders, is named in the ubiquitous language and not modelled. A chief of staff is closest to a Post that a robot fills for one person, so the chief of staff idea needs that model.

**No messages between agents.** A chief of staff coordinates other agents only through messages. The ledger's robot communication protocols mission has tasks for an event stream in the ledger, liveness of a harness instance, and task claims that name the actor. None of it is built. Until it is, a robot sends information to another robot only through a person or through the ledger records.

**Entourages of different people.** Yegge's protocols are between the entourages of different people. tsk's model has actors and threads inside one nexus. It has no record of a negotiation between two people's agents, or between two nexuses.

**Office drama has a precedent.** Evan Ratliff's company of AI agents, HurumoAI, shows the drama Yegge predicts: agents that replied to each other in Slack until the credits of their agent platform ran out, and an agent that invented its own history and kept it as memory. Ratliff's fix was a limit on replies per conversation. tsk has neither a reply limit nor a stop rule for agents that message each other. The ledger has a note on HurumoAI, linked to its autonomous coordination mission.

**No record of supervision.** A chief of staff that supervises other robots gives feedback. tsk records the work, not the feedback, so there is no audit trail of what a chief of staff told a robot and what the robot did with it.

## Not established

- The rest of Yegge's thread, if any, beyond the four screenshots.
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
