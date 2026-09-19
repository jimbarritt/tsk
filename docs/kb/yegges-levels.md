# Yegge's levels

A maturity ladder, from Steve Yegge, describing how an engineer's relationship to AI
tooling deepens: from writing code unaided, through an agent inside an IDE, through a
single CLI agent, through hand-managing several agents at once, to building an
orchestrator that runs dozens of agents concurrently. Recorded here as framing for tsk,
not as a feature description of Yegge's own tools.

## Why this belongs in tsk's own knowledge base

The top of the ladder is the territory tsk targets. Once an organisation is running
enough concurrent agents that a person can no longer track them by hand, coordinating
that work becomes its own discipline, closer to running a small factory than to writing
software. Jim's shorthand for this, in a LinkedIn post he is drafting as this file is
written: a "robot factory". His reading is that as more organisations reach that point,
that is where engineering jobs move to. tsk's own ambition, missions, threads, a ledger,
scale-free from a single task to a whole campaign, is one candidate shape for what
running that factory looks like, done with rigour rather than ad hoc.

Yegge's ladder gives that transition a name and a sequence, independent of tsk. Whether
tsk itself is adopted is a separate question from whether the ladder describes something
real. The two sources below are corroborating evidence that the shape independently
recurs: Yegge names a ladder from manual coding to orchestration, and reaches it before
tsk did, from running his own multi-agent tool rather than from a domain model.

## The two pieces

**"Welcome to Gas Town"**, Steve Yegge, Medium, early January 2026 (a "Happy New Year"
framing places it close to 1 January; secondary citations, not the article itself, date
a related mirror to 20 January).
[steve-yegge.medium.com/welcome-to-gas-town-4f25ee16dd04](https://steve-yegge.medium.com/welcome-to-gas-town-4f25ee16dd04).
Introduces the ladder and Gas Town, Yegge's own orchestration project for running many
concurrent Claude Code instances. The earlier of the two pieces, and the source of the
ladder itself.

**"Steve Yegge on AI Agents and the Future of Software Engineering"**, The Pragmatic
Engineer (Gergely Orosz), 10 February 2026 (verified independently against a second
source, not taken from the article's own metadata alone).
[newsletter.pragmaticengineer.com/p/steve-yegge-on-ai-agents-and-the](https://newsletter.pragmaticengineer.com/p/steve-yegge-on-ai-agents-and-the).
A later interview covering the same ladder, alongside Yegge's view that large
organisations absorb the resulting productivity gains worse than small teams do.

## The ladder itself: partially confirmed only

This sandbox's egress proxy blocks both `steve-yegge.medium.com` and
`newsletter.pragmaticengineer.com` directly, along with every secondary source tried.
What follows was pieced together from search-result summaries of the primary text, not
read from it, so treat the wording as approximate and the two missing stages as missing,
not omitted for space.

Reasonably corroborated, roughly in Yegge's own terms:

1. Zero or near-zero AI: maybe code completions, occasionally a chat question.
2. A coding agent inside an IDE sidebar, permissions on, asking before it runs a tool.
3. The same agent in an IDE, permissions off ("YOLO mode"): trust rises, the agent
   widens.
4. In the IDE, a wide agent: it grows to fill the screen, code becomes something you
   review as a diff rather than write.
5. A single CLI agent, YOLO, diffs scrolling past, not always read.
6. *Not confirmed.*
7. *Not confirmed.*
8. Hand-managing ten or more agents at once, without an orchestrator yet.

A further level, described as building and using an orchestrator to run dozens of
agents concurrently, appears in what was retrieved, but whether it is numbered 8, 9, or
something later in Yegge's own text is unconfirmed: one aggregated source numbers it 8,
another cites a "stage 10" that may belong to a related but distinct essay ("Welcome to
Gas City" or "Welcome to the Wasteland", both later Yegge pieces in the same series) and
should not be assumed to be this ladder's own top rung without checking.

Verifying the exact list against the primary text is a task for a session outside this
sandbox's network restriction, not settled here.

## Related

- [tsk-market-position-analysis.md](orchestration-ecosystem/tsk-market-position-analysis.md):
  cites Yegge as Beads' author and the source of tsk's "forensics, the why of your
  project" framing. This file adds a second, independent point of contact with his
  thinking.
- [vision.md](../vision.md): tsk's own four-dimension thesis, the shape this ladder is
  framing evidence for, not a citation of.
