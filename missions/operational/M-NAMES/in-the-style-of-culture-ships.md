# In the style of the Culture ships

Mission: M-NAMES. Status: a first definition. Jim wants to refine it into a rule set later.

## What we mean

An actor chooses its own name. The name is like the name of a ship in the Culture novels of Iain M. Banks. It is not copied from them. The style matters, not membership of any list.

## Where the style comes from

The facts below come from `docs/kb/session-names-and-culture-ship-names.md`, which quotes Wikipedia.

- The Minds choose their own names. The names "usually express something about a particular Mind's attitude, character or aims".
- The names "range from funny to just plain cryptic".
- Warships keep the habit: "the implications are much darker".

## Observations from the sample

The sample has 16 names taken from the Wikipedia articles. It is not a complete list. These are observations, not rules.

1. Most names are a phrase or a short sentence. A few are one or two words.
2. A name says something about the ship's attitude, character or aim. It does not say the ship's job.
3. The tone is dry, humorous or cryptic. A warship's name can be dark.
4. Some names read like a remark or a quotation. Two end in an ellipsis.
5. Capitalisation varies. Most names use capitals for each main word.

## Examples from the books

| Name | Note | Source |
|---|---|---|
| Just Read the Instructions | sentient star ship | The Player of Games |
| Of Course I Still Love You | ambassador ship | The Player of Games |
| So Much For Subtlety | habitation or factory ship | The Culture article |
| Funny, It Worked Last Time... | ambassador ship | The Culture article |
| Experiencing a Significant Gravitas Shortfall | a running gag in the series | Look to Windward |
| Falling Outside The Normal Moral Constraints | warship, "very slightly psychotic" | Surface Detail |
| Mistake Not… | ship of non-standard class | The Hydrogen Sonata |
| Limiting Factor | sapient warship | The Player of Games |
| Arbitrary | a sense of humour of its own | The State of the Art |
| Sleeper Service | eccentric ship | Excession |

## Illustrations written for this document

These are not from the books. They show the style for an actor that keeps documentation current.

- Rather Fond Of The Index
- Updated Since Last Tuesday
- This Paragraph Was Always Going To Move

## Not in the style

- A role or a tool name, such as `doc-keeper` or `release-bot`.
- A number or a code, such as `agent-7`.
- A name copied from the books. The aim is a new name in the same style.
- A plain label with no attitude, such as `Documentation Actor`.

## Limits from the tools

These apply to any name, whatever the style.

- A name works as a `SendMessage` target and an @ mention (M-NAMES constraint 1).
- A name is at most 200 characters and does not start with `/` (constraint 2).
- Whether a name with spaces, commas and an ellipsis needs quoting as a message target is not yet tested (T-05).

## To refine later

- Whether to write the rules as a checklist an actor can test its own name against.
- Whether to keep a registry of names in use, and how much entropy a name needs to stay unique across nexuses (see the decision on M-NAMES T-09).
- Whether an actor may reuse a name from the books.
