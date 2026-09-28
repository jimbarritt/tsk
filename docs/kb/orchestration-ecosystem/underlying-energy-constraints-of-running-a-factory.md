# Underlying energy constraints of running a factory

Source: Steve Yegge, ["Seats and Sunsets"](https://yegge.ai/essays/seats-and-sunsets/).
"Fuel" is Yegge's word for tokens; it is kept inside quotations only.

## Token cost

Figures from Wheelhouse, Yegge's private harness for his game Wyvern, run on Claude Max
accounts:

- "Today, I burn through an entire week of Fable, one whole account, in 2 to 4 hours."
- "By two weeks ago I had 21 Claude Max accounts", adding "two new Claude Max accounts
  each week".
- Sustaining Wheelhouse at its peak would take "55 Claude Max accounts", "around
  $12,000/month". He "stopped at 21".
- "almost all the lights are off right now, [...] because I can no longer afford the
  tokens."

## Model tier

- "if you want to run a real-world factory, you need Fable-tier for at least some roles,
  or your factory will quickly eat itself."
- The distinguishing property is caution, not raw intelligence: Fable shows "modest
  wisdom"; "OpenAI models have essentially no wisdom or caution at all."
- "Fable is the only cautious model" and "Fable cares about seats more than other
  models" are, in the essay, "equivalent claims. Seats have caution built into them."

## Trust and tokens

- "If models cannot trust, they must verify. [...] This, friends, costs tokens. Lots of
  tokens."
- In a plain session, a model spends "O(context)" establishing trust in four unknowns:
  the actor, the environment, the authority, and the consequences of a bad action.
- "With a seat, all those free variables are bound."
- "Trust accumulates slowly [...] But trust doesn't erode, it just breaks. A single lie
  destroys all the truths at once, globally."
- Over-fencing is distrust written as policy: each fence adds cost and reduces output.
- "Fuel is what distrust costs you. [...] Seats are trust you paid for once and cached."

## Posts as an escalation target in tsk

Not designed. A [post](../../domain/ubiquitous-language.md#post) binds the four unknowns
above: the actor, the environment, the authority, and the consequences.

- A human or an agent can hold a post.
- A post's permissions are commensurate with the post. Holding the post grants them.
  This is the shape of an approval workflow in an organisation: a spend above one
  threshold needs a manager, above a higher threshold a director.
- An agent escalates a decision outside its post's authority to another post, over a
  comms medium such as Slack. The agent addresses the post, not its holder.
- Whether a task is worth its token cost is a spend decision of this kind. Above a
  threshold, it is outside a given post's authority.
- Open: the thresholds, which post holds each, and how an agent determines when to
  escalate.

## Related

- [docs/domain/ubiquitous-language.md](../../domain/ubiquitous-language.md): Post and
  Actor.
- [tsk-market-position-analysis.md](tsk-market-position-analysis.md), under Seats: the
  doctrine mapping behind the name "post".
