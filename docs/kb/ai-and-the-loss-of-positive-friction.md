# AI and the loss of positive friction

Status: research note, 2026-09-27. Source: John Cutler,
["TBM 441: AI, the Loss of Positive Friction, and What to Do About It"](https://cutlefish.substack.com/p/tbm-441-ai-the-loss-of-positive-friction),
The Beautiful Mess, 2026-09-24. Raised by Jim as relevant to tsk, and to its Product
dimension in particular.

## The article

Product work used to move information through stages by hand. Each hand-off made
someone "process, think about, discuss, reshape, restate" it. Cutler calls that effort
positive friction: it is where pruning, discretion and judgement happened.

AI removes it. Feedback becomes an LLM summary, the summary becomes a spec, and the spec
becomes code, with little human attention at any step. What goes with it: the distinction
between stages, traceability back to the original customer context, and the
focus that held feature sprawl back. His name for the anti-pattern: "AI flattening
layered on AI flattening, with no thread back to reality."

The stages he says get flattened, in order:

| Stage | Cutler's definition |
|---|---|
| Signals | What is observed, recorded, written, or measured |
| Insights | What we infer from those signals |
| Models | How we believe things work |
| Options | Possible paths, configurations, or interventions |
| Choice | The selected option and the resulting commitment |
| Intent | The future state or direction being pursued |
| Actions | What actors do to advance the intent |

His ten remedies are listed with the mapping below. In all of them, human judgement and
accountability stay at the framing and commitment stages.

## Cutler's stages against tsk

| Stage | Nearest tsk term | Fit |
|---|---|---|
| Signals | Product, Intelligence | Named, not modelled. `vision.md` places "signals coming from customers" and observability in the Product dimension, from Jim's pitch transcript of 2026-07-05. The domain model has no object for them. Intelligence is "the general term for input context", with its subtypes "not yet defined". |
| Insights | none | No object. An inference can appear in a briefing's Intelligence section, as prose. |
| Models | Product, Product capability | Partial. Product describes "what the product does or should do for its users". A belief about how users or a market behave has no object. |
| Options | Navigation, Path | Close. Navigation holds parallel and abandoned routes, and an abandoned route "produces navigational knowledge". Path records the abandoned routes behind a delta. |
| Choice | Mission, ADR | Close. A mission is the commitment made at delegation. An ADR records a choice and its reason. |
| Intent | Objective | Close. An objective is a checkable state. `mission-model.md` grounds a briefing's Purpose and Objective in commander's intent, from mission command. |
| Actions | Task, Thread, Delta | Close. A task is the unit of work, a thread its execution, and a delta the change it makes. |

tsk models the last four stages closely. It has almost nothing for the first three.

## Cutler's remedies against tsk

| # | Remedy | tsk today |
|---|---|---|
| 1 | Keep original feedback atomic, each item linked to its source | No feedback object. Path gives traceability downward, from a delta to its commits and decisions, not upward to a customer signal. |
| 2 | Keep both atomicity and coherence | Present, for changes: atomic deltas cluster into a composite delta, the same entity at another zoom level. Not for signals, which have no object. |
| 3 | At each transformation, "make it clear what is being carried forward, what changed, and why" | Present, at delegation. A briefing is written when a task becomes a mission, "because the receiving actor does not have the holder's context". A mission report records "what the briefing failed to give the actor". A thread continuation records what's next at each pause. |
| 4 | Separate the underlying work from its framing for each audience | Present. "The same mission renders differently for a human and for a cloud agent": Mission and Mission briefing are separate terms. |
| 5 | Label every AI-generated artefact by degree of AI contribution | Partial. A continuation's written-by field names the binding that wrote it, and commit attribution names the agent. No artefact has a degree-of-contribution label. |
| 6 | Do not stack AI flattening on AI flattening | Present, at the delivery end. Under the Delta Gate, "done" follows from a verified production state, not a status set by hand. A closed card is never the evidence for a healthy capability. |
| 7 | Do not put everything into a tool because the tool can process it | Partial. Ledger and Artefact are split by definition, and the tsk/ksobr boundary test places each line. No rule governs what enters the ledger. |
| 8 | Treat code analysis as one source of evidence | Present. Product and Artefact are separate terms: "Naming the files 'the product' would complect the two." System health is measured in production, not read from the code. |
| 9 | Keep each issue bounded: "whose need it represents, why it matters, what problem we're addressing" | Present for missions: a briefing states Purpose, Objective, and Out of scope. `product-and-scale-theory.md` requires a story to trace to a user goal. |
| 10 | Prefer depth over breadth | No rule. Waypoint and the always-fully-functional constraint govern how a change ships, not how many ship at once. |

## What this means for tsk's product side

tsk closes the loop at the delivery end. The Delta Gate and System health check a
capability against production, so tsk has a thread back to reality once a change ships.
It also builds positive friction into delegation, through the briefing and the report.

It has no object for the input end of the loop: signals, insights, and models. The
vision names signals: `vision.md` places customer signals and observability in the
Product dimension. The domain model has not caught up.
`product-and-scale-theory.md` records the same gap from the other direction: "A
separate document on the user domain model is referenced by the source material but not
yet written." A story traces to a user goal, but nothing in tsk records the observations
the user goal was inferred from. Cutler's first two remedies, atomic, traceable signals
kept coherent, describe what that object needs.

The second gap is remedy 5. tsk records which actor wrote a continuation entry, but not
how much of an artefact an agent produced.

## Related

- [product-and-scale-theory.md](product-and-scale-theory.md): Story card, Product
  capability, Delta Gate and System health, and the unwritten user domain model.
- [background-theory.md](background-theory.md): Navigation, Delta, and Path.
- [docs/domain/ubiquitous-language.md](../domain/ubiquitous-language.md): every tsk term
  in the tables above.
- [docs/domain/mission-model.md](../domain/mission-model.md): mission command and
  commander's intent.
