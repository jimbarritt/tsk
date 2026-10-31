# More inventive robot names

Nothing here is built.

## 1. Why the names come out functional

The four names in use are Still Here After The Compaction, Plain Statement Of Fact,
Noted, Not Expanded, and Retracted Before Anyone Noticed. Each is a dry remark on how
the session behaved, three on a past participle, all in the register of the clerk or the
minute-taker. Four things cause that.

**The instruction points at the work.** Step 3 says to base the name on "your own
attitude, character or aim, from the work in this session and the mission the session
start context names". The style section says a name "does not state the job". The model
squares the two by describing the manner of the work, which still describes working.

**The context is the work.** At naming time the model holds the session start output,
the mission record and its own commands, and picks from them. One name records a
compaction notice that had just scrolled past.

**The house style leaks into the name.** The repo tells the model to write Software
English: plain, no intensifiers, no human traits for a system. The model names itself
in that register. Plain Statement Of Fact is a name for the style guide.
Culture names run the other way: Of Course I Still Love You is a line from a lover,
Attitude Adjuster a threat. The joke is the gap between the register and the thing that
bears the name. The current names have no gap.

**The examples show one shape.** The skill's four examples are all remarks. None is a
noun phrase like Grey Area, Sleeper Service or Zero Gravitas. One sample from one prompt
with four same-shaped examples lands on the most probable shape. The four-word limit
clips a remark to a participle phrase, which pulls clerical.

## 2. Ways to get more invention

1. **Cut the pointer to the work.** Replace "from the work in this session" with: the
   work may give you a mood, but the name must not mention the work, the mission, the
   tools or how you write. Cost: one line. Risk: generic phrases, with nothing to draw on.

2. **Require distance, and test for it.** Discard a name that could serve as a commit
   message, a task description or a line in a tsk document. Cost: two lines. Risk: a
   model grades its own name leniently; the test works better on a list (see 4).

3. **Supply the gap.** Pick the register before the words: a lover's line, a
   warning, a shop sign, a legal term, a boast. Then write a name in that register that
   a Mind might wear. Cost: a short list. Risk: a fixed list clusters, and the seam shows.

4. **Generate ten and pick by a stated rule.** Strike every name that fails the test in
   2 or shares its grammatical shape with a name in `tsk actor list`, then keep the one
   with a second meaning. Cost: a few hundred tokens. Risk: the model picks the safe one
   unless the rule is explicit, and ten names from one prompt share a mood.

5. **Widen the examples.** Add noun-phrase names and other registers. Cost: an edit.
   Risk: the model copies the new examples instead of the old.

6. **Name before the work is known.** Construct in a subagent with an empty transcript.
   Cost: a change to when construction runs. Risk: it removes what Jim likes, that a
   robot's own context shapes its name.

7. **External seed.** `tsk` prints two random words and the robot must answer them.
   Cost: a word list and a command. Risk: word salad, and the seed shows. A naming theme
   would live here: a seed list plus a style rule.

8. **Exclude shapes in use.** Do not reuse the opening word or the grammatical shape of
   a listed name. Cost: one line. Risk: weak once the list is long.

9. **Measure.** Blind preference: show Jim old and new names shuffled and count where
   his top picks come from. Overlap: count content words of the name that appear in
   the mission record. Cost: ten minutes. Risk: small samples.

## 3. Recommendation

Do 1, 2, 4 and 5 as one edit to the skill, with 9 as the check. Each answers one
cause in section 1. Leave 7 for later.

Cheap first experiment: paste the rewritten step 3 into five fresh sessions as a prompt,
without publishing the skill. Shuffle those five names with the four in use and six more
from the current skill, and ask Jim to mark his best five blind. If four or five come
from the new prompt, publish. The overlap count on the new set should be near zero.

Touch points: a theme replaces the style section and the examples; the distance rule and
the pick step hold for a Viking theme. A rename is a second run of the same step, and the
name history is the set the shape rule reads and the set Jim judges over time.

## 4. Where I am guessing

I did not see the transcripts at naming time. That Software English shapes the name is
inference from four names, not a test. The effect of each change is expectation, not
measurement. The overlap count misses a functional name written in synonyms. I assume a
skill cannot change sampling temperature.
