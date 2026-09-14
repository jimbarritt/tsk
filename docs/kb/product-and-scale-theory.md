# Product and Scale: background theory

Supporting material behind the Product, Story card, Product capability, `Delta Gate`,
System health, and Scale terms defined in `docs/domain/ubiquitous-language.md`. This
holds reasoning and grounding, not itself a new decision.

## Why story card, capability, and delta are kept apart

Most tools complect all three into a single ticket: the opinion (card status: done)
gets confused with the fact (delta deployed, system healthy). Keeping them separate
answers a question otherwise almost unanswerable: "we played that story two sprints
ago, is the capability healthy in production right now?"

| Concept | Dimension | Nature | Lives in |
|---|---|---|---|
| Story card | Navigation | Ephemeral planning token | The path: a conversation, a sprint |
| Product capability | Product | Persistent description of what the product does | The product model, for as long as the capability exists |
| Delta | Delta | The only mechanism of state transition | The event log: immutable facts about what changed |

This follows the no-complecting principle (Rich Hickey, *Simple Made Easy*): keep
different things explicitly separate rather than braided together, even when they
overlap in content.

## Opinion vs fact

| | Card closed | Delta deployed and healthy |
|---|---|---|
| Nature | Opinion | Fact |
| Source | Human gesture | Production event |
| Queryable now? | Only historically | Yes, at any moment |
| Can regress? | No, card stays closed | Yes, health changes |
| Trustworthy? | Sometimes | Always |

## Deltas as health-state transitions

```
Healthy   → [delta] → Healthy    (capability added or improved)
Healthy   → [delta] → Unhealthy  (change fails, a regression)
Unhealthy → [delta] → Healthy    (remediation, fix or rollback)
```

A delta's success is measured not by whether it deployed, but by whether the system
is healthy afterwards.

## Resilience patterns as health maintenance

Patterns from *Release It!* (Michael Nygard) maintain composite system health under
real-world conditions, ensuring partial unhealthiness does not cascade:

- **Circuit breaker**: isolates an unhealthy component, preventing cascade.
- **Bulkhead**: contains failure so composite health degrades gracefully.
- **Timeouts, retries, fallbacks**: maintain sufficient health when parts degrade.

These are direct expressions of the always-fully-functional constraint from the
Navigation and Delta model (see `docs/kb/background-theory.md`): the system stays
fully functional at all times, even if minimally.

## Story granularity has no tiers

Stories, like deltas and Navigation, are fractal. A large capability decomposes into
smaller, independently deliverable capabilities. Each level is the same kind of thing;
there are no artificial tier boundaries (no "epic vs story vs sub-task"). Zoom level
determines the view, not the type.

- A large capability may require many composite deltas across many cycles.
- A small story may map to a single atomic deployment delta.
- A capability is not delivered until its relevant deltas are deployed and the system
  is healthy against its acceptance criteria.

## Stories trace to user goals

A story that cannot be traced to a user goal is a candidate for removal. Stories are
not technical tasks; they are descriptions of user-meaningful capability, grounded in
user goals and the activities users perform to meet them. A separate document on the
user domain model is referenced by the source material but not yet written.

## Sources

- `2026-02-22T08-35-10Z-tsk-product-stories-system-health.v2` (v2 supersedes v1;
  sharpened separation of card, capability, delta), from the 2026-02-22 tsk domain
  model session, catalogued in Jim's knowledge base under the `tsk` folder.
- `2026-02-22T09-30-45Z-tsk-domain-model-provenance-log.v1`, same session: attributes
  the ideas above almost entirely to Jim (LR-Origin), with a small number of
  dialogue-emerged formulations (for example, `Delta Gate` and
  "opinion vs fact") and a few Claude elaborations (for example, connecting Nygard's
  resilience patterns to the health model). Relevant if this material is published
  formally.
