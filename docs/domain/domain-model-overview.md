# Domain model: overview

Status: settled unless marked open. Term definitions are in
[ubiquitous-language.md](ubiquitous-language.md). Doctrine rationale is in
[mission-model.md](mission-model.md), which also covers the central tension
between step and task and the tsk/ksobr boundary test. Source: 2026-09-14
bootstrap design session.

## Decided

- Delegation is what makes a task a mission. A task becomes a mission at the
  point it is handed to a different actor from the one holding the parent
  mission, because the receiving actor lacks the holder's context and needs a
  briefing. Assignment alone is not the test: every task on an agent's own task
  list has an assignee, the agent itself, and none of them is a mission. Most
  tasks never become missions. A task has an ID, an objective, and a status,
  always, done by whoever holds the mission it belongs to. A mission is a task
  at the point of handoff; the receiving actor decomposes it into their own
  tasks.
- One task list exists per mission. Any summary table is a projection of it,
  never a second authored copy, which removes table/body drift at the root
  rather than by a rule to keep two copies in sync by hand.
- Threads are the execution sequence. A thread holds its own context and can be
  paused, suspended, and resumed, which is what lets an agent continue a set of
  tasks from one session to the next.
- Thread state and a run record are two separate artefacts. Thread state is
  forward-looking: which tasks are done, which is in progress, where to resume;
  it is tsk domain, because the next session may be a different actor. A run
  record is retrospective: what happened in one run, for evaluation; it is
  ksobr domain.
- A mission takes several runs. One mission per run does not mean one run per
  mission; thread state is what makes the next run continue rather than
  restart.
- Tasks are the units of work and nest as deep as needed. Every task must have
  an objective.
- Missions are the units handed to agents. A mission has an objective, which
  also serves as its definition of done, and breaks down into tasks.
- Tasks are scale-free: a task at a higher level becomes a mission for a
  sub-agent. The common element across levels is the objective.
- Mission is a first-class domain object, not only a link to a parent task; it
  can hold its own metadata.
- The document handed to an agent is a mission briefing.
- Task types from doctrine are kept as flags on entries in the one task list,
  not as separate sections: specified (in the briefing), implied (the actor
  added it), essential (the one specified task the mission fails without).
- An agent's first behaviour on receiving a mission is to write its own task
  list.
- Intelligence is the general noun for input context. It replaces the earlier
  terms research, knowledge base, and evidence. A whole mission can be
  intelligence gathering, for example a spike or a research assignment.
- Delta is not the grouping concept; it has felt incongruous in use in the plan
  format.
- tsk is probably a graph-shaped model underneath. Parked for later.
- An object model of the whole of tsk is wanted, but not yet started.

## The tsk and ksobr boundary: worked split

Test (from [mission-model.md](mission-model.md)): would it still make sense if
a human, not an agent, received the mission? If yes, it is tsk domain. If it
only makes sense because a harness executes it, it is ksobr domain. Applied to
a mission briefing's own fields:

| Field | Domain |
|---|---|
| Objective and its constraints | tsk |
| Decision authority | tsk |
| Purpose, derived from the parent objective | tsk |
| Intelligence links | tsk |
| Tasks | tsk |
| Permitted files | ksobr |
| Token budget, attempt limits | ksobr |
| Ways of working | ksobr |
| Report format | ksobr |
| Transcript capture and run records | ksobr |

A run's outcome (done or blocked) is a state change on a task, so it is a tsk
event. An actor's account of the run is a run record, so it is ksobr.

## Open

- Decided in principle, fields not yet defined: a mission is administrative or
  operational, carried by its subdirectory rather than by a field (see
  [ubiquitous-language.md](ubiquitous-language.md#mission)). No separate
  campaign or major-operation object type is added above mission, even though
  doctrine nests campaign above mission: Scale, one of tsk's four dimensions,
  rejects fixed echelons, and doctrine only has them because armies use fixed
  command layers.
- Missions need dependencies of their own, not only tasks. Every mission in
  the bootstrap sequence holds a "blocked by" field; the plan format this
  superseded placed blockers at task level only.
- Where standing constraints live. Model selection and subscription limits
  apply to every mission in a sequence; attaching them to each one would
  duplicate them. By the tsk/ksobr boundary test, and because doctrine
  references standing operating procedures rather than restating them, these
  are ksobr domain.
- The mission object's full field list is not yet fixed beyond what the
  mission briefing template already specifies.
- Not decided: how this model relates to milestones, or to any grouping of
  tasks above a mission.
- A mission can defer a decision to a later mission. M-BOOT-01 does not decide
  the official data ref layout; M-BOOT-04 does. Decision authority is
  therefore a field whose value is sometimes "not yours, see mission {id}".
