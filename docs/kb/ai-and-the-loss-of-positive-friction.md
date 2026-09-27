# AI and the loss of positive friction

Source: John Cutler,
["TBM 441: AI, the Loss of Positive Friction, and What to Do About It"](https://cutlefish.substack.com/p/tbm-441-ai-the-loss-of-positive-friction),
The Beautiful Mess, 2026-09-24.

## The argument

Positive friction is the effort a hand-off between stages of product work demands. At
each stage, someone has to "process, think about, discuss, reshape, restate" the
information. Pruning, discretion and judgement happen there.

AI removes the hand-offs. Feedback becomes an LLM summary, the summary becomes a spec,
and the spec becomes code, with little human attention at any step. The losses: the
distinction between stages, traceability back to the original customer context, and
the focus that limits feature sprawl. Cutler names the anti-pattern "AI flattening
layered on AI flattening, with no thread back to reality."

Human judgement and accountability stay at the framing and commitment stages.

## Stages

| Stage | Definition |
|---|---|
| Signals | What is observed, recorded, written, or measured |
| Insights | What is inferred from signals |
| Models | How things are believed to work |
| Options | Possible paths, configurations, or interventions |
| Choice | The selected option and the resulting commitment |
| Intent | The future state or direction pursued |
| Actions | What actors do to advance the intent |

## Stages mapped to tsk

| Stage | tsk term | Fit |
|---|---|---|
| Signals | Product | Named, not modelled. The Product dimension includes observability and signals from customers. No domain object represents a signal. |
| Insights | none | No object. |
| Models | Product capability | Partial. A product capability describes what the product does for its users. A belief about how users or a market behave has no object. |
| Options | Navigation, Path | Close. Navigation holds parallel and abandoned routes. Path records the abandoned routes behind a delta. |
| Choice | Mission | Close. A mission is the commitment made at delegation. |
| Intent | Objective | Close. An objective is the checkable end state a mission pursues. |
| Actions | Task, Thread, Delta | Close. A task is the unit of work, a thread its execution, and a delta the change it makes. |

tsk models options, choice, intent and actions. It has no object for signals, insights,
or models.

## Remedies mapped to tsk

| # | Remedy | tsk |
|---|---|---|
| 1 | Keep original feedback atomic, each item linked to its source | Absent. Path traces a delta to its commits and decisions. Nothing traces work back to a customer signal. |
| 2 | Keep both atomicity and coherence | Present for deltas: atomic deltas compose into a composite delta at a higher zoom level. Absent for signals. |
| 3 | At each transformation, state what is carried forward, what changed, and why | Present at delegation. A mission briefing is written when a task passes to another actor. A mission report records what the briefing failed to give. A thread continuation records the next step at each pause. |
| 4 | Separate the underlying work from its framing for each audience | Present. One mission renders as a different briefing for each actor. |
| 5 | Label every AI-generated artefact by degree of AI contribution | Partial. A thread continuation names the actor that wrote it. No artefact records the degree of AI contribution. |
| 6 | Do not stack AI flattening on AI flattening | Present at delivery. Under the Delta Gate, a capability is delivered only when its delta is deployed and the system is healthy. A closed story card is not evidence of delivery. |
| 7 | Do not put everything into a tool because the tool can process it | Partial. The ledger and the artefacts are separate. No rule governs what enters the ledger. |
| 8 | Treat code analysis as one source of evidence | Present. Product and Artefact are separate terms. System health is measured in production, not read from code. |
| 9 | Keep each issue bounded: whose need, what it is for, what problem | Present. A mission briefing states purpose, objective, and out of scope. A story traces to a user goal. |
| 10 | Prefer depth over breadth | Absent. No rule limits how many changes ship at once. |

## Gaps

- Signals, insights and models have no domain object. The Product dimension names
  customer signals and observability, and the user domain model that grounds user goals
  is not written. Remedies 1 and 2 give the requirements for a signal object: atomic,
  linked to its source, and composable.
- No artefact records the degree of AI contribution (remedy 5).
- No rule limits breadth (remedy 10).

## Related

- [docs/domain/ubiquitous-language.md](../domain/ubiquitous-language.md): definitions of
  the tsk terms above.
- [product-and-scale-theory.md](product-and-scale-theory.md): Product capability, Delta
  Gate, and System health.
