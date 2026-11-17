# LangChain: From tasks to goals

Harrison Chase's post states that people now give agents goals in place of tasks, and
lists six requirements of a goal. tsk's mission model matches that change, and two of the
six: the actor writes its own task list, and the ledger keeps decisions across runs. tsk
has part of each of the other four: tool controls, an inbox, feedback, and triggers with
stopping rules.

Status: a blog post, "From tasks to goals", by Harrison Chase of LangChain, in his
"Harrison's In the Loop" series. Dated 2026-11-16, read 2026-11-17. Raised by Jim on
2026-11-17: the post "sets out the principles behind tsk".

Source: [From tasks to goals](https://www.langchain.com/blog/from-tasks-to-goals). The page
was fetched as raw HTML and converted to text with a Python HTML parser, and the whole
article was read as text. Quoted phrases were matched against that text. The claims are
the author's own. The post links two related posts, "Goal Engineering: How to Design Goals"
(2026-11-14) and "New context engineering challenges in late 2026" (2026-11-02). Neither
was read.

## What it is

A short essay of principles, with no product announcement and no data model. Its claim,
about agents: "we're moving from giving them tasks to giving them goals".

Its examples of tasks: "Write this email", "Draft this blog", "Fix this bug". Its
examples of goals: "Reduce my coordination burden while keeping important commitments on
track", "Maintain a blog", "Make this product successful".

The post divides the move into a shift from specifying an action to specifying an
outcome, and a shift from a bounded assignment to an ongoing responsibility.

## The principles, distilled

Each section of the post ends with a "Practical consideration".

| Principle | Claim | Practical consideration |
|---|---|---|
| The agent breaks a goal into tasks | The user states a goal. The agent breaks it into tasks, does some of them, and updates the task list. The task ideas need to vary | A way to see the tasks, "e.g. a to-do list", and a prompt for varied ideas |
| Tool calls are way less known | A task implies its tools. The tools a goal needs are not known in advance | "more controls over what tools an agent can or cannot call" |
| Inbox to ask questions | A background agent sends a person a request for clarification, "whether through Slack or a dedicated UI" | A mechanism for a background agent to ask for clarification |
| Way to get feedback | Progress is measured by a metric or by a person's feedback. "its never wise to blindly optimize a metric" | Design goals so that feedback is easy to give, and give the agent a way to receive it |
| Ongoing work needs triggers (and stopping rules) | A heartbeat, a recurring cron, a one-off cron or an event stream starts the agent. A rule stops, pauses or ignores a trigger. "Sometimes doing nothing is the right action." | Crons and event triggers, and a prompt that states when to act |
| Memory lets feedback compound | With a task, the user supplies the context each time. With an ongoing responsibility, decisions and feedback from earlier runs apply. "rejecting a blog topic should inform future topic selection" | Retain and use decisions and feedback across runs |

The conclusion: "You're delegating decisions about what work to do and when to do it."
The person's role "shifts from specifying each task to setting direction, defining
boundaries, and reviewing progress".

## Mapping to tsk

The tsk terms are defined in
[ubiquitous-language.md](../../domain/ubiquitous-language.md), and the reasons for the
mission model are in [mission-model.md](../../domain/mission-model.md).

### A goal is a mission with an objective

The post's goal is tsk's [Mission](../../domain/ubiquitous-language.md#mission): what an
actor receives, with an [Objective](../../domain/ubiquitous-language.md#objective) that is
its definition of done. tsk already does this.

- **From an action to an outcome.** tsk took this shift from doctrine. Mission-model.md
  records that a task "should be phrased as an objective to meet, not as an activity, so
  progress is measurable", and that mission command issues "the purpose and desired end
  state, rather than a detailed sequence". A mission record has a purpose and objective
  outcomes, and a person does not write the sequence of steps.
- **From a bounded assignment to an ongoing responsibility.** An objective in tsk is "a
  fixed end point, or a measure that changes over time". "Reduce our error rate" is the
  ubiquitous language's own example of the second, which is the shape of "Maintain a
  blog". An administrative mission has no end point and stays `in-progress`. The post's
  ongoing responsibility is this case.
- **Where tsk differs.** In the post, each goal is given to one agent. In
  tsk, the objective is a field of the mission, and the mission is separate from the
  [Actor](../../domain/ubiquitous-language.md#actor) and the
  [Thread](../../domain/ubiquitous-language.md#thread) that work it. Any actor can take
  the work, and a thread can resume it as a different actor. Mission-model.md notes that a
  mission with no end point "is closer to a description of an actor than to a unit of
  work". The [Post](../../domain/ubiquitous-language.md#post) placeholder is the tsk term
  for a standing position that outlives whoever fills it. It is not modelled.

### The actor writes the plan

The post's first principle, that the agent breaks a goal into tasks, is tsk's
[Plan](../../domain/ubiquitous-language.md#plan). tsk already does this. A plan "belongs to
the actor executing the mission, not to the one who wrote the briefing". A briefing may
list specified tasks, and the actor adds the tasks the work implies and rewrites the plan
during execution. The plan is the mission's task records, so `tsk task list`
shows it. This is the post's "to-do list" that a person reads.

No tsk record or command covers the requirement for varied task ideas. A prompt in the
harness meets it, under the
[tsk and ksobr boundary](../../domain/mission-model.md#the-tsk-and-ksobr-boundary).

tsk also delegates: a task given to a different actor becomes a mission of its own, with
its own objective. Tasks are scale free, so a goal
breaks into tasks, and a task can become a goal for another actor.

### Controls on tools are named and not enforced

tsk names tool controls as text, and no component enforces them. A briefing names
Constraints and Execution constraints ("Permitted files. Attempt limit. Budget."), and a
mission record has constraints and out of scope items. The Post placeholder would have a
scope of authority. [openappa.md](openappa.md) and
[copilot-dynamic-workflows.md](copilot-dynamic-workflows.md) record two mechanisms that
enforce limits of this kind, and neither is linked to tsk.

### The inbox is a question record, with no channel to a person

tsk does part of this. A [Question](../../domain/ubiquitous-language.md#question) is a
record: `tsk question add` asks it, `tsk question answer` records the answer and who gave
it, and `tsk question list` shows the open questions across every mission. A background
robot can write one and continue with other tasks.

tsk sends no notification to a person, in Slack or in a dedicated UI, and an answer does
not start the robot again. M-TSK-COMMS-01, "Jim and the robots communicate through a
messaging app", is the mission for this, and it is not started.

### Feedback is recorded as decisions, and tsk checks no objective

tsk does part of this.

- **A metric.** An objective can be a measure that changes over time. Each objective
  outcome has a `met` flag. tsk checks no objective: a mission is done when its status is
  set to `done`.
- **Feedback from a person.** A [Decision](../../domain/ubiquitous-language.md#decision) is
  a record of a choice, with who decided and when. A person's correction of the work can
  be recorded as one. tsk has no record whose purpose is feedback on progress.
- **Feedback in the other direction.** A
  [Mission report](../../domain/ubiquitous-language.md#mission-report) is the actor's
  feedback to the person who reads it: what was done, what the briefing failed to give,
  and what the actor found wrong. The post does not describe this direction.

[claude-code-dynamic-workflows.md](claude-code-dynamic-workflows.md) records one way to
check an objective with independent verifying agents. It is not built.

### Triggers: tsk has an event queue, and the harness runs schedules

tsk does part of this.

- **Event stream.** The ledger has an external event queue: `tsk events append` adds an
  event, `tsk events read-new` reads the events past a watermark, and
  `tsk events advance-watermark` marks them processed.
- **Heartbeats and crons.** tsk has none of its own. A Claude Code scheduled task starts a
  session on a schedule, and the session joins its thread through its harness instance
  (ADR 0031). This matches Jim's position of 2026-10-16 in
  [tsk-market-position-analysis.md](tsk-market-position-analysis.md): durable and
  proactive running is a feature of the harness, and tsk supplies the semantic layer.
- **Stopping rules.** A thread can be paused or stopped. The unattended handoff pattern in
  [agent-context-self-regulation-and-unattended-handoff.md](../agent-context-self-regulation-and-unattended-handoff.md)
  writes a stop condition for `/goal`. tsk has no rule for a robot to do nothing on a
  trigger, and no rule that stops a repeated question. M-TSK-AUTO, "Robots keep working
  on missions until no task is left", is the mission for autonomous work, and it is not
  started.

### Memory is the ledger

tsk already does this. The [Ledger](../../domain/ubiquitous-language.md#ledger) has the
decisions, questions, tasks and thread continuations, and each new session reads them. The
post's example, a rejected blog topic that informs the next choice, is a decision record
on the mission. A [Thread continuation](../../domain/ubiquitous-language.md#thread-continuation)
records the state of a line of work for the next session, which can be a different actor
on a different machine.

The post's memory is per agent. tsk's memory is per mission and per thread, so any actor
reads it. The market position analysis records the gap: a live fact that applies
across missions, such as a release date that changes, has no record in tsk.

### The person sets direction and boundaries, and reviews progress

tsk already does this. A mission record has a purpose, objective outcomes, constraints,
out of scope items and decision authority. That is "setting direction, defining
boundaries". A mission report and `tsk mission list` are the review of progress.

## Comparison with tsk

| Principle in the post | tsk term | Does tsk do it |
|---|---|---|
| A goal, not a task | Mission with an Objective | Yes |
| An outcome, not an action | Objective as a checkable state, from doctrine | Yes |
| An ongoing responsibility | Administrative mission, and an objective that is a measure over time | Yes. A standing position, Post, is a placeholder |
| The agent breaks a goal into tasks | Plan, owned by the actor executing the mission | Yes |
| A visible to-do list | Task records, `tsk task list` | Yes |
| Varied ideas for tasks | None. Prompting, in the harness | No |
| Controls on tools | Constraints and Execution constraints, as text | Named, not enforced |
| An inbox for questions | Question records | Recorded. No channel to a person, no restart on an answer |
| A metric | Objective outcome with a `met` flag | Recorded. Not checked |
| Feedback from a person | Decision records | Recorded. No feedback record |
| Triggers | External event queue. Schedules in the harness | Partly |
| Stopping rules | Thread pause and stop | Partly. No rule to do nothing |
| Memory across runs | Ledger: decisions, questions, thread continuations | Yes, per mission and per thread |
| Setting direction and boundaries | Purpose, objective, constraints, out of scope, decision authority | Yes |
| Reviewing progress | Mission report, `tsk mission list` | Yes |

## Convergence and difference

The post and tsk agree:

- The unit of delegation is an outcome, not an action.
- An outcome can be an ongoing responsibility with no end point.
- The actor that does the work writes its own task list, and a person can read it.
- A decision recorded in one run applies to the next.
- The person's role is direction, boundaries and review.

They differ:

- In the post, the goal, the memory and the triggers are attached to one agent. tsk
  separates the mission (the work), the thread (the execution) and the actor (who does
  it), so a different actor on a different machine continues the work.
- tsk has a report from the actor to the person. The post describes feedback in one
  direction only, from the person or a metric to the agent.
- tsk delegates a task as a mission of its own, at any scale. The post has one level: a
  goal and its tasks.
- The post names tool controls, an inbox, metrics and triggers as requirements. tsk has
  part of each: constraints as text, question records, a `met` flag, and an event queue.

The post models none of tsk's dimensions
[Product](../../domain/ubiquitous-language.md#product),
[Delta](../../domain/ubiquitous-language.md#delta) or
[Scale](../../domain/ubiquitous-language.md#scale). It is a statement of principles for
agents, with no data model, so it adds no evidence for the three-part verdict in the market
position analysis.

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position, with a
  summary of this note.
- [langsmith.md](langsmith.md): LangSmith Trajectories, Engine v2 and Fine-Tuning.
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): the tsk terms
  used above.
- [docs/domain/mission-model.md](../../domain/mission-model.md): the doctrine behind an
  objective as an outcome.
