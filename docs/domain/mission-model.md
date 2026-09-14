# Mission model: rationale

Term definitions are in
[docs/domain/ubiquitous-language.md](ubiquitous-language.md#mission). This file
holds why the model is shaped this way. Source: 2026-09-14 bootstrap design session.

## The central tension: step vs task

- A **step** is a movement on the route, independent of people and mostly of time,
  leaving a trail behind, similar to a log entry. Not adopted as a unit in tsk.
- A **task** is a request to do something (for example: go to the shop and buy
  bread).
- **Threads** belong to the domain of doing.

tsk separates the structural domain of maps, plans, and deltas from the domain of
execution.

## Doctrine reference

Research from US and NATO joint doctrine (JP 1, JP 3-0) informed the mission model.
Levels of war (strategic, operational, tactical) classify actions by which level of
objective they serve, not by any property of the action itself. Terms, in nesting
order:

- **Campaign**: a series of related military operations aimed at a strategic or
  operational objective, within a given time and space.
- **Major operation**: a series of tactical actions coordinated in time and place
  under a common plan and a single commander.
- **Mission**: what an organisation is directed to do, derived through mission
  analysis.
- **Objective**: a decisive and attainable goal toward which every operation is
  directed. A state, not an action, so it is checkable.
- **Task**: a defined action or activity assigned to an individual or organisation,
  imposed by an appropriate authority.
- **Function**: the broad, enduring role an organisation exists to perform. A
  separate branch, not a level in the same chain.

Supporting points that carried into tsk's model:

- A task should be phrased as an objective to meet, not as an activity, so
  progress is measurable. Doctrine's example: "conduct air defence suppression" is
  an activity; "suppress opposing air defence" is an objective.
- A task assigned to a subordinate unit becomes that unit's mission. Mission and
  task are the same object seen from two levels; this is the scale-free property
  tsk's task model relies on.
- Mission analysis separates specified tasks, implied tasks, and the essential task.
  tsk keeps these as flags on entries in one task list, not as separate sections.
- Objectives relate horizontally as well as vertically, through lines of operation
  and lines of effort. Not yet used in tsk's model; recorded for when a horizontal
  relationship between objectives is needed.
- **Mission command** (from the German *Auftragstaktik*) issues a commander's
  intent, the purpose and desired end state, rather than a detailed sequence,
  because detailed plans do not survive contact with reality. This underlies why a
  mission briefing states Purpose and Objective rather than a prescriptive plan.
- Doctrine is advisory, not directive: policy sets objectives, strategy applies ways
  and means, doctrine is codified good practice. tsk's mission model follows this
  posture, prescribing structure, not the work itself.

One operations-research paper (not doctrine) models campaign objectives as **axes**,
each a totally ordered set of objectives with precedence constraints. This matches
the bootstrap sequence's own missions 1 to 6, which must run in order.

## The tsk and ksobr boundary

tsk defines the meta-structure of a mission. ksobr enriches and customises it. The
harness is the envelope; the mission briefing sits inside it.

Test for placing a line: would it still make sense if a human, not an agent,
received the mission? If yes, it is tsk domain. If it only makes sense because a
harness executes it, it is ksobr domain. The military draws the same line: an
operations order states the mission and intent; how the unit operates lives in
standing operating procedures, referenced rather than restated. See
[docs/domain/ubiquitous-language.md](ubiquitous-language.md#ksobr) for the current
table, and note ksobr's own domain modelling is not yet settled, tracked as a task
under the nexus and territory mission, not as tsk's own ubiquitous language.

## Open

- Missions need dependencies of their own, not only tasks. Every mission in the
  bootstrap sequence holds a "blocked by" field at the mission level; the
  now-superseded plan format placed blockers at task level only.
- Where standing constraints (model selection, subscription limits) live. They
  apply to every mission in a sequence; attaching them to each one duplicates them.
  By the tsk/ksobr boundary test, and by doctrine's own practice of referencing
  standing operating procedures instead of restating them, these belong to ksobr,
  referenced from a mission rather than written into it.
- The mission object's full field list is not yet fixed beyond what the mission
  briefing template already specifies.
