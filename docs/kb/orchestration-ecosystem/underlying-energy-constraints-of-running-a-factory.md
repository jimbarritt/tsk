# Underlying energy constraints of running a factory

Status: reference, started 2026-09-21. Raised by Jim from Steve Yegge's essay
["Seats and Sunsets"](https://yegge.ai/essays/seats-and-sunsets/), the same essay behind
the [Post](../../domain/ubiquitous-language.md#post) entry in the ubiquitous language.

"Fuel" is Yegge's word for tokens. It is kept where he is quoted; "tokens" is used
elsewhere.

## What Yegge reports

Numbers, from running Wheelhouse, his private harness for Wyvern, on Claude Max
accounts:

- "Today, I burn through an entire week of Fable, one whole account, in 2 to 4 hours."
- "By two weeks ago I had 21 Claude Max accounts, and Fable 5.1 has been consuming fuel
  faster every week." He "had to add two new Claude Max accounts each week to keep up
  with its thirst."
- Sustaining Wheelhouse at its peak, "at current pricing, which as I said, keeps going
  up", would take "55 Claude Max accounts", "around $12,000/month". He "stopped at 21"
  and is "now focusing on fuel efficiency".
- The result: "almost all the lights are off right now, and my $25k 512GB Mac Studio is
  sitting idle at home, because I can no longer afford the tokens."

## What Yegge says about Fable

- "Claude Fable 5, broadly, [is] the only model worth a shit in the entire industry right
  now. You need that level of intelligence to do anything useful. Weaker models are just
  going to be a headache."
- "The Astras and Opus 5s of the world are great as personal assistants, and they can
  help you with almost any individual coding task. But if you want to run a real-world
  factory, you need Fable-tier for at least some roles, or your factory will quickly eat
  itself."
- On what sets it apart, wisdom rather than intelligence: Fable shows "modest wisdom",
  which he rates "on par with a middle-schooler". "OpenAI models have essentially no
  wisdom or caution at all."
- "Fable seems to like the concept of a seat much more than other models do. Fable
  itself came up with the name 'seat' and helped me flesh out the concept over several
  months."
- "Fable concluded last night that my two claims (1) 'Fable is the only cautious model'
  and (2) 'Fable cares about seats more than other models' are in fact equivalent claims.
  Seats have caution built into them."

## The relationship he draws between trust and tokens

- "If models cannot trust, they must verify. So if they catch you (or an instrument, or
  another system) in a lie, the whole foundation they are working on switches from law
  into unreliable eyewitness testimony, which they must then verify for themselves before
  continuing. This, friends, costs tokens. Lots of tokens."
- In a plain session, "it takes O(context) for them to figure out whether they can trust
  your environment. By default, when they awaken, and you ask them to do something, they
  are dealing with a bunch of unknowns: Unknown actor (you), unknown environment, unknown
  authority, and most of all, unknown consequences if they perform a bad action. So they
  spend tokens verifying that it all looks legit, before they can act."
- "With a seat, all those free variables are bound. The seat has accrued a set of
  truths, linearly, as you flesh out the seat's definition... Which means they are not
  spending tokens covering their asses by reverifying everything."
- "Trust accumulates slowly, with truths accumulating only as fast as you can discover
  and write them down... But trust doesn't erode, it just breaks. A single lie destroys
  all the truths at once, globally."
- On his own factory's over-fencing: "Each refusal we added was the system saying, 'We
  can't trust this particular configuration because it caused an incident.' Each fence
  was accumulating cost and reducing the work output, all because the system decided not
  to trust itself anymore."
- His conclusion: "Fuel is what distrust costs you. Fences are distrust written down as
  policy. Seats are trust you paid for once and cached, so nobody has to re-derive it at
  4am." So "the thing I've been calling a fuel crisis is in large part a trust
  calibration problem."

## Jim's read

This matches Jim's own intuition: he is needed more when using Sonnet. And he cannot
afford to run the factory at any scale.

## The tsk angle

Jim's idea, recorded in his terms, not yet designed:

tsk's model of collaboration with humans means that humans could fill
[posts](../../domain/ubiquitous-language.md#post). The conditions: the agents understand
what the post is and when they should escalate, and there is a comms medium, Slack for
example. Given those, tsk provides a solution to this constraint.

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md), under Seats: the
  seat concept mapped against tsk's Actor and Thread continuation, and the doctrine
  split behind the name "post".
- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): the Post
  placeholder, and Actor.
- [docs/vision.md](../../vision.md): tsk "is about collaboration between humans and
  agents, and between humans and humans, not agent orchestration alone", the line the
  tsk angle above rests on.
