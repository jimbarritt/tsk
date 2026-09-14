# tsk viability against beads

Date: 2026-07-01 (re-examination), distilled 2026-09-14.

## Status

Accepted as the current strategic position. Provisional: blocked by the
token-saving experiment referenced below, which had not run as of this writing.

## Context

Beads (`gastownhall/beads`, v1.0.4 at the time, roughly 25k GitHub stars) is a graph
issue tracker built for agent work. It matured substantially after tsk's initial
design and independently converged on much of tsk's architecture: Dolt as a
version-controlled substrate, a daemon in single-writer mode over Unix sockets, an
`issues.jsonl` interchange format that is explicitly not the source of truth (the
inverse of tsk's design, where the NDJSON event log is the source of truth and SQLite
is a disposable cache, but the same two-layer instinct), an MCP package, and
persistent-memory features (`bd remember`, `bd prime`). It is positioned as a
plan-replacement and a token/context-economy play, the same positioning tsk holds.

This raised the direct question: is tsk still worth pursuing, and is symbiosis with
beads still possible, or is the space now purely competitive?

## Where beads has converged, and where it hasn't

The overlap is architecture and positioning, not conceptual model.

- **Converged**: Dolt, daemon, single-writer, Unix sockets, the plan-replacement
  pitch, the token/context-economy claim, MCP, persistent memory.
- **Not converged**: beads stays inside the [Navigation](../domain/ubiquitous-language.md#navigation)
  dimension, with a discrete epic-to-story-to-sub-task hierarchy via dotted IDs, which
  is exactly the artificial-tier boundary tsk's [Scale](../domain/ubiquitous-language.md#scale)
  dimension exists to dissolve. Beads has no model of
  [Product](../domain/ubiquitous-language.md#product) (the thing being built), no
  first-class [Delta](../domain/ubiquitous-language.md#delta), and no continuous,
  fractal Scale.

Yegge's own framing of beads ("forensics, the why of your project, joined against the
what/where/how of your git commits") gestures at a join between intent and change,
loosely Product joined against Delta, but treats it as a join between two systems,
not a modelled domain construct in its own right. It is the closest beads' framing
comes to reaching outside Navigation.

## Three-part viability verdict

1. **tsk as a research programme: yes.** Beads' rise is evidence that structured
   navigation context beats markdown plans for agents at scale, validating half of
   tsk's underlying claim. It says nothing about whether adding Product, Delta, and
   continuous Scale produces further measurable value. Beads is therefore a
   credible, widely adopted baseline to measure the other three dimensions against,
   and this track continues regardless of the product outcome.
2. **tsk as a head-to-head agent issue tracker: no.** That category has an
   incumbent with distribution, maturity, an evangelist, and most of tsk's
   architecture already. Entering it late and alone loses, and would be a category
   error: it would confine tsk to the one dimension, Navigation, that beads already
   owns.
3. **tsk as a product differentiated by the full four-dimension model: open.** This
   is the central bet, and what the (separately scoped, not yet run) token-saving
   experiment exists to test. Whether Product, Delta, and Scale add value an agent
   or buyer will reward is unproven. Beads existing makes that experiment cheaper
   and more legible, since it is now a control. The product decision waits on that
   experiment rather than being made now.

Caveat in tsk's favour: beads' star count likely overstates independent, load-bearing
adoption, since Gas Town/City is largely beads' own primary consumer. "Beads has won"
overstates the case. The category is also visibly unsettled elsewhere (Linear's
"issue tracking is dead" framing, the Block convergence). The product path is not
closed, it is not one to walk through on the Navigation axis.

## Symbiosis vs competition

Technically symbiotic, not a partner to build strategy on. The layers are
compatible: tsk could use beads as its Navigation substrate, or as the baseline it
measures against. But beads is an expanding project with momentum and an evangelist;
expanding incumbents tend to absorb adjacent value rather than leave room for a
symbiote. Treat beads as a swappable substrate and a research baseline, not an ally.
tsk's thesis is the unification of all four dimensions, not any single one, so it
does not collapse if beads later absorbs another dimension.

## Net position

Do not stop tsk. Do not ship tsk as a head-to-head tracker. Do not plan around
beads' goodwill. Run the token-saving experiment to de-risk the four-dimension claim,
keep the research track moving regardless of the product outcome, and let the
experiment's data decide the product question.

## Related

- [docs/domain/ubiquitous-language.md](../domain/ubiquitous-language.md): the
  Navigation, Delta, Product, and Scale dimensions referenced throughout.
- The token-saving experiment referenced above has not yet been designed or run as
  of this writing; it is not tracked in the M-BOOT mission tree, which is scoped to
  bootstrapping self-hosting rather than to this product decision.
