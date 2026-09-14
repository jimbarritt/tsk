# Background theory

Supporting material behind the Navigation and Delta terms defined in
`docs/domain/ubiquitous-language.md`. This is theoretical grounding and open research
direction, not settled design. Source: `2026-02-21T17-57-25Z-tsk-navigation-delta-domain-model_v1.md`,
copied into `.inbox/jims-kb-tsk-domain/`.

## Core insight

The deployment event, the delta, and the path that produced it are co-equal. The path
explains the delta; the delta is what the path produced. Planning and code are not
separate concerns joined by an integration between them: both are first-class from the
start, unlike tools where a ticket is primary and code is attached as evidence.

## Step size and correlated random walks

A theoretical basis for delta sizing, from the mathematics of a correlated random walk:
each step is biased toward the previous direction.

- A large step covers ground quickly but accumulates directional error; the cost of
  correcting it scales faster than the step size does. Important territory can be
  passed through unnoticed.
- A small step gives tight feedback on direction. Each delta becomes a course
  correction.
- The right step size depends on the terrain: larger steps are safe in well-known,
  low-complexity territory; smaller steps become necessary, not out of caution, in
  unknown, high-complexity territory, because the feedback loop is itself the
  navigation instrument.

Not yet incorporated: simulations and a thesis on correlated random walks, to be
retrieved as theoretical grounding.

## The noun/verb tension

"The artefact is a noun. The path to it is all verbs." The thing being built exists as
a noun; the act of building it, committing, refactoring, deploying, deciding, is all
verbs. Most tools collapse this distinction; this model holds it explicitly.

The sculpture analogy: building is not assembly from nothing, but revealing a shape
already latent in the material. The domain tends to reveal itself the more it is
worked.

## Theoretical connections

Proposed as different views of one underlying model:

- **Continuous delivery and small batches**: follow directly from the atomic delta as
  the unit of deployment, not a practice added on top.
- **Throughput, per Goldratt and DORA**: visible as the length of path between
  waypoints. A long path means work-in-progress inventory and lower throughput.
- **Real options**: a branch in the path is an option held until the last responsible
  moment. Walking a short way down a branch to evaluate it prices the option; it is not
  wasted work.
- **Evolutionary architecture**: fitness functions act as cairns on the path.
  Architecture gets discovered through building, not designed upfront.
- **Kent Beck's TDD and Tidy First**: the transformation and behavioural-delta
  distinction maps directly onto it. Safe, reversible transformations are the grain of
  movement.
- **Correlated random walks**: the theoretical basis for step sizing relative to
  terrain complexity, above.

## Illustrative stories

Not yet encoded as domain tests.

1. **The simple case**: a single route, a single atomic delta. The baseline the model
   must never complicate.
2. **The coordinated release**: a composite delta across multiple services with
   ordering constraints. A partial failure leaves a known intermediate state.
3. **The parallel routes**: two agents working at once, independent until convergence.
   Open question: do they share a waypoint?
4. **The abandoned path**: a route started, the approach found wrong, a backtrack. The
   path holds value as history even though it produced no delta.

## Core constraint: always fully functional

The system stays fully functional at every point, even if that functionality is
minimal. A delta is valid only if it moves the system from one fully functional state
to another; the system is never left broken or partial in production, even
temporarily. This extends the walking-skeleton principle to every point on the route,
not only the start: there is always a skeleton, however minimal, and it always walks.

Mechanisms that keep a composite delta's individual atomic deltas each fully
functional:

- **Expand and contract**: add the new capability first, safely, then remove the old
  shape later, safely. There is never an invalid intermediate.
- **Feature flags**: the code deploys, and the system stays fully functional, while the
  behaviour stays hidden. The deployment delta is complete; revealing the behaviour is
  a separate delta.
- **Blue/green and canary releases**: the transition between states never leaves a user
  in a broken experience.

## Next steps (from the source session, unresolved)

- Formalise the domain model using domain-driven-design aggregates, entities, and
  value types.
- Encode the illustrative stories above as domain tests.
- Retrospectively analyse a real project's session log to extract actual delta
  structure and validate the model against it.
- Retrieve the correlated random walk simulations and thesis referenced above.
