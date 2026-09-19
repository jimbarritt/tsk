# Yegge's levels

A maturity ladder, from Steve Yegge, describing how an engineer's relationship to AI
tooling deepens: from writing code unaided, through an agent inside an IDE, through a
single CLI agent, through hand-managing several agents at once, to building an
orchestrator. Recorded here as framing for tsk, not as a feature description of Yegge's
own tools.

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
Yegge retells the same ladder in this interview, roughly a month later, alongside his
view that large organisations absorb the resulting productivity gains worse than small
teams do.

## The ladder, verbatim

This sandbox's egress proxy blocks `steve-yegge.medium.com` directly, so this is not
read from the page. Jim supplied the passage below verbatim from "Welcome to Gas Town",
which settles the two stages an earlier draft of this file had marked unconfirmed, and
corrects a wrong guess at what stage 8 was: it is the orchestrator, not hand-managed
agents, which sits at stage 7.

> First, you should locate yourself on the chart. What stage are you in your
> AI-assisted coding journey?
>
> Stage 1: Zero or Near-Zero AI: maybe code completions, sometimes ask Chat questions
>
> Stage 2: Coding agent in IDE, permissions turned on. A narrow coding agent in a
> sidebar asks your permission to run tools.
>
> Stage 3: Agent in IDE, YOLO mode: Trust goes up. You turn off permissions, agent gets
> wider.
>
> Stage 4: In IDE, wide agent: Your agent gradually grows to fill the screen. Code is
> just for diffs.
>
> Stage 5: CLI, single agent. YOLO. Diffs scroll by. You may or may not look at them.
>
> Stage 6: CLI, multi-agent, YOLO. You regularly use 3 to 5 parallel instances. You are
> very fast.
>
> Stage 7: 10+ agents, hand-managed. You are starting to push the limits of
> hand-management.
>
> Stage 8: Building your own orchestrator. You are on the frontier, automating your
> workflow.
>
> If you're not at least Stage 7, or maybe Stage 6 and very brave, then you will not be
> able to use Gas Town. You aren't ready yet. Gas Town is an industrialized coding
> factory manned by superintelligent robot chimps, and when they feel like it, they can
> wreck your shit in an instant. They will wreck the other chimps, the workstations, the
> customers. They'll rip your face off if you aren't already an experienced
> chimp-wrangler. So no. If you have any doubt whatsoever, then you can't use it.

Eight stages, not ten. The "stage 10" an earlier draft of this file flagged from an
aggregated source does not belong to this ladder; it was a different, later Yegge essay
being conflated with this one, and this passage confirms the top rung is stage 8.

## A second telling, from the interview

Jim also supplied the equivalent passage from the Pragmatic Engineer piece, Yegge
retelling the same ladder in an interview roughly a month later. Also verbatim, also
not read from the blocked URL:

> agent. I put them all on a spectrum just to show what's going on. Here's the levels:
>
> Level 1: no AI
>
> Level 2: Coding agent in your IDE, permissions turned on
>
> Level 3: Coding agent in IDE, "YOLO mode." Your trust is going up.
>
> Level 4: you're starting to not look at the diffs anymore, but at what the agent is
> doing. You're not reviewing as much, you're letting more of it through, and you're
> really focused on the conversation with the agent.
>
> Level 5: your approach is: "I just want the agent and I'll look at the code in my IDE
> later, but I'm not coding with my IDE".
>
> Level 6: several agents. You're bored because your agent's busy and you want to do
> something, so you fire up another agent, then another. And you find yourself just
> multiplexing between them, and you can't "leave" [you start to get addicted to using
> more agents.]
>
> Level 7: 10+ agents, managed by hand. This is where you typically say "oh gosh, I've
> made a mess! I accidentally texted the wrong agent and didn't realize. How do I
> coordinate all these agents? What if Claude Code could run Claude Code?"
>
> Level 8: you build your own orchestrator to coordinate more agents".

The shape is the same eight rungs, but the two tellings diverge in the middle, not the
ends. Stages 1 to 3 and 7 to 8 match closely across both. In between, "Welcome to Gas
Town" describes an agent physically widening on screen and diffs scrolling past
unread; the interview instead describes attention shifting from the diff to the
conversation, then splitting across several agents out of boredom, to the point of
naming it an addiction. Same person, same ladder, two different aspects of climbing it
emphasised depending on the telling. Worth having both rather than collapsing them into
one paraphrase.

Where tsk sits on this ladder is the question stage 8 poses directly: building the
orchestrator, not running one someone else built. Gas Town is Yegge's own answer to that
stage. tsk's mission and thread model is a different one, aimed at the same rung.

## Related

- [tsk-market-position-analysis.md](orchestration-ecosystem/tsk-market-position-analysis.md):
  cites Yegge as Beads' author and the source of tsk's "forensics, the why of your
  project" framing. This file adds a second, independent point of contact with his
  thinking.
- [vision.md](../vision.md): tsk's own four-dimension thesis, the shape this ladder is
  framing evidence for, not a citation of.
