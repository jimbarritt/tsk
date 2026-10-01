# AI and the loss of positive friction

Source: John Cutler,
["TBM 441: AI, the Loss of Positive Friction, and What to Do About It"](https://cutlefish.substack.com/p/tbm-441-ai-the-loss-of-positive-friction),
The Beautiful Mess, 2026-09-24. Quoted phrases were matched against the page's raw text.
The page's reader comments are not part of the argument and are not used here.

## The argument

Cutler describes a product workflow in six steps: a customer call, synthesising
insights, deciding what to do, doing it, shipping it, and measuring impact. Before AI,
each step passed information on by hand. Someone had to "process, think about, discuss,
reshape, restate, copy-paste, 'migrate'" it. Cutler calls this "positive friction". Each
hand-off was a chance to apply a "forcing function": "to focus conscious attention on
something, to avoid automating your thinking, to pay attention, discern, say 'no' or
'what if.'"

With AI, each step can run without that attention. A transcript becomes an LLM summary,
the summary becomes a spec, and the spec becomes code. Cutler lists what is lost: the
chances for "thinking, pruning, shaping, designing, judgement-applying", and material
that is "original, atomic, as written by a human", which becomes pointers. Breadth also
grows: five customer calls become 80 opportunities, 80 tickets and 400 tasks.

Cutler names the "biggest anti-pattern": "AI flattening layered on AI flattening, with
no thread back to reality." He observes that "it is the edges where the work happens".

## Ontology

Cutler found that artefacts mix atomic pieces of information, each with a job. He lists
twelve:

| Element | Definition |
|---|---|
| Signals | What is observed, recorded, written, or measured |
| Insights / Interpretation | What we infer from those signals |
| Models / Hypotheses | How we believe things work and connect, including assumptions |
| Options | Possible paths, configurations, or interventions |
| Choice | The selected option and resulting commitment |
| Intent | The future state or direction being pursued |
| Actions | What actors do to advance the intent |
| Mechanical Change | The specific thing that is modified |
| Effects / Impact | The resulting change |
| Actors | Who observes, interprets, chooses, acts, or is affected |
| Materials | Systems, tools, technologies, artifacts, and infrastructure |
| Constraints | Conditions that limit, shape, or enable the system |

Cutler describes these as a graph. Every edge is a transition point.

## Ontology mapped to tsk

| Element | tsk term | Fit |
|---|---|---|
| Signals | Product | Named, not modelled. The Product dimension includes observability and signals from customers. No domain object represents a signal. |
| Insights / Interpretation | none | No object. |
| Models / Hypotheses | Product capability | Partial. A product capability describes what the product does for its users. A belief about how users or a market behave has no object. |
| Options | Navigation, Path | Close. Navigation holds parallel and abandoned routes. Path records the abandoned routes behind a delta. |
| Choice | Mission | Close. A mission is the commitment made at delegation. |
| Intent | Objective | Close. An objective is the checkable end state a mission pursues. |
| Actions | Task, Thread | Close. A task is the unit of work and a thread is its execution. |
| Mechanical Change | Delta | Close. A delta is a change to the artefacts. |
| Effects / Impact | System health | Partial. System health is measured in production against acceptance criteria. The effect on a customer has no object. |
| Actors | Actor | Close. |
| Materials | Artefact | Partial. An artefact is what a mission builds. Tools and infrastructure have no object. |
| Constraints | Constraints in the mission briefing | Close. A briefing lists constraints on the objective. |

tsk models options, choice, intent, actions, change and actors. It has no object for
signals, insights, or models.

## Principles

Cutler lists eleven principles.

| # | Principle | tsk |
|---|---|---|
| 1 | Keep original feedback atomic, and preserve its path back to the source | Absent. Path traces a delta to its commits and decisions. Nothing traces work back to a customer signal. |
| 2 | Preserve both atomicity and coherence | Present for deltas: atomic deltas compose into a composite delta at a higher zoom level. Absent for signals. |
| 3 | Add positive friction when content is copied and recontextualized. At each transformation, make it clear what is carried forward, what changed, and why | Present at delegation. A mission briefing is written when a task passes to another actor. A mission report records what the briefing failed to give. A thread continuation records the next step at each pause. |
| 4 | Separate the underlying job from its presentation to different audiences | Present. A briefing renders one mission for a specific actor. |
| 5 | Label AI-generated content by degree of AI contribution | Partial. A thread continuation names the actor that wrote it. No artefact records the degree of AI contribution. |
| 6 | Avoid compounding AI flattening | Present at delivery. Under the Delta Gate, a capability is delivered only when its delta is deployed and the system is healthy. A closed story card is not evidence of delivery. |
| 7 | Do not let AI capability turn a tool into a dumping ground | Partial. The ledger and the artefacts are separate. No rule governs what enters the ledger. |
| 8 | Treat code analysis as one source of evidence | Present. Product and Artefact are separate terms. System health is measured in production, not read from code. |
| 9 | Use the tracker for bounded team intent: "whose need it represents, why it matters, what problem we're addressing" | Present. A mission briefing states purpose, objective, and out of scope. A story traces to a user goal. |
| 10 | Go for depth over breadth in product improvements | Absent. No rule limits how many changes ship at once. |
| 11 | The tracker has two jobs: team-visible intent, and personal planning and decomposition | Partial. The ad hoc step of task scope is never recorded as a task. A thread continuation is per actor. No rule says which decomposition enters the ledger. |

## Gaps

- Signals, insights and models have no domain object. The Product dimension names
  customer signals and observability, and the user domain model that grounds user goals
  is not written. Principles 1 and 2 give the requirements for a signal object: atomic,
  linked to its source, and composable.
- No artefact records the degree of AI contribution (principle 5).
- No rule limits breadth (principle 10).
- No rule separates team-visible intent from personal decomposition in the ledger
  (principle 11).

## Related

- [docs/domain/ubiquitous-language.md](../domain/ubiquitous-language.md): definitions of
  the tsk terms above.
- [product-and-scale-theory.md](product-and-scale-theory.md): Product capability, Delta
  Gate, and System health.
