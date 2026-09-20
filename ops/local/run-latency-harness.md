# Latency harness

A harness for validating runs: it measures what a turn costs, under conditions
you set, from numbers the harness did not compute. Script:
[`run-latency-harness.py`](run-latency-harness.py).

Built to answer one question, "why do replies feel slower", but the shape is
general. Any question of the form "does X change what a turn costs" fits it:
set X as a condition, hold everything else, run both sides, read the fields.

## Methodology

### Do not use a stopwatch

A stopwatch measures the thing you are holding, not the thing you want. Every
number here comes from a field the CLI already records.

From the result JSON (`claude -p --output-format json`):

| Field | What it holds |
|---|---|
| `duration_ms` | The whole turn |
| `ttft_ms` | Time to first token |
| `duration_api_ms` | Time inside the API call |
| `usage.output_tokens` | Tokens written |
| `usage.output_tokens_details.thinking_tokens` | Reasoning tokens |
| `total_cost_usd` | Cost of the turn |

From the session transcript, `~/.claude/projects/<cwd-slug>/<session-id>.jsonl`:
each `stop_hook_summary` entry holds `hookInfos`, one `durationMs` per hook
command. The result JSON carries no hook timing, so per-hook cost is only
readable here. The harness pre-assigns each run's session ID with
`--session-id`, so it reads the right transcript without guessing.

### Hold context length constant

Each run is a fresh headless session, `claude -p "<prompt>"`, one session per
run. Context length dominates latency. Measuring inside one long interactive
session confounds every comparison, because the context grows as the session
runs.

### Controls

- **Neutral working directory.** Runs happen in a temporary directory, not in
  the repository. A repository's own hooks would otherwise run on every turn,
  and one of them fetches over the network, which varies per run.
- **`--setting-sources user`.** Project settings enable the plugin and register
  this repository's hooks. Both are the thing under test or noise around it.
  User settings hold auth and model configuration, and stay constant.
- **Shuffled run order.** Running every A first and every C last turns a slow
  period in the API into an apparent difference between conditions. The harness
  shuffles the full run list against a seed.
- **A discarded warm-up per condition.** The first run of a session claims a
  container. That cost belongs to no condition.
- **A pinned model**, so a fallback mid-experiment does not read as an effect.

### Conditions

| Condition | Plugin | Output style |
|---|---|---|
| A | not loaded | default |
| B | loaded, `--plugin-dir` | default (not selected) |
| C | loaded, `--plugin-dir` | selected, `--settings '{"outputStyle":"..."}'` |

`B - A` isolates the plugin's hooks. `C - B` isolates the output style.

Before `swe` 0.9.1, its output style carried `force-for-plugin: true`, so
loading the plugin applied the style whatever `outputStyle` said, and "plugin
on, default style" was not reachable through settings. B needed a staged copy
of the plugin with `output-styles/` deleted to get that condition at all. From
0.9.1 the flag is gone: the style is plain-selectable, so B and C differ only
in the `--settings` flag, confirmed live (loading the plugin with no
`outputStyle` override leaves its style inactive; adding the override selects
it, `is_error: false`, and the reply visibly follows the style's rules).

## Running it

```bash
python3 ops/local/run-latency-harness.py --dry-run            # print the plan, run nothing
python3 ops/local/run-latency-harness.py                      # 3 conditions x 3 lengths x 5 = 45 runs
python3 ops/local/run-latency-harness.py --repeats 3 --lengths short,long
python3 ops/local/run-latency-harness.py --conditions A,C --model opus
```

Each run writes one JSON object to `--out` (default `latency-results.ndjson`),
and the script prints a median, mean and range per condition and prompt length.
Rows keep `result_head`, the first 400 characters of the reply, so a reader can
confirm a condition did what it claims.

Runs cost money. 45 runs at a few pence to a few tens of pence each is a real
spend. Use `--dry-run` first, and `--repeats` to size it.

## First experiment

Against `swe` 0.7.0, using the staged-copy design for condition B, since
`force-for-plugin` was still in force. Kept as the record that led to the
upstream fix; see the second experiment below for the current release.

**Question.** Replies feel slower. Three candidates were proposed: the
plugin's Stop hook, its `PostToolUse` check on `Write|Edit`, and its output
style raising reasoning tokens. The prior reading was that the Stop hook skips
its reply check under Claude Code and so is not the cost, leaving the output
style as the candidate.

**Pilot run.** One repeat per condition, short prompt, model `sonnet`,
2026-09-20. Five minutes of API time.

| Condition | `duration_ms` | `ttft_ms` | Output tokens | Thinking tokens | Hook ms | Cost USD |
|---|---|---|---|---|---|---|
| A | 2220 | 1351 | 45 | 0 | 0 | 0.036 |
| B | 4223 | 1266 | 41 | 0 | 2037 | 0.036 |
| C | 4303 | 1118 | 30 | 0 | 2040 | 0.166 |

**The Stop hook is a real per-turn cost on a plain chat reply.** `B - A` is
2003 ms, which the hook's own recorded 2037 ms accounts for. The prior reading
held that the hook is not the cost. Both readings can stand: the hook may
well skip its reply check, and it still spends its full two second cap on
every turn. What it does internally and what it costs in wall clock are
separate facts, and only the second one reaches the person waiting.

**The output style's latency cost is not established.** `C - B` is 80 ms at
one repeat, which this pilot cannot separate from noise.

**Reasoning tokens give the style no support here.** All three conditions
recorded zero thinking tokens. The pilot used a short prompt, so this says
nothing about a prompt long enough to provoke reasoning.

**The style has a cost, in money.** C cost 0.166 USD against A's 0.036. The
style adds a system-prompt section, and a fresh session writes it to cache
rather than reading it from cache. A long-lived session pays this once, not
per turn, so it does not follow that the style is slow.

**The style shortens replies.** 30 output tokens against 45 for the same
prompt. Fewer tokens is less time to write, which works against the style
being the source of slowness.

### What this pilot does not settle

One repeat per cell. Medium and long prompts were not run, and reasoning
tokens are most likely to appear there. The full design, 5 repeats across 3
lengths, is what separates an 80 ms difference from noise.

## Second experiment: after the 0.9.1 upgrade

`swe` upgraded from 0.7.0 to 0.9.1: the Stop hook removed entirely, alongside
`force-for-plugin` on the output style. B no longer needs the staged-copy
design; see Conditions above. Basic test, one repeat per condition, short
prompt, `sonnet`, 2026-09-20.

| Condition | `duration_ms` | `ttft_ms` | Generation after `ttft` | Hook ms | Cost USD |
|---|---|---|---|---|---|
| A | 2165 | 1187 | 978 | 0 | 0.036 |
| B | 2110 | 1091 | 1019 | 0 | 0.036 |
| C | 3106 | 2114 | 992 | 0 | 0.040 |

**The Stop hook's cost is gone.** `hook_ms_total` reads zero for every
condition, and `B - A` is now 55 ms, within run-to-run noise at one repeat.
Confirms the fix in jimbarritt/claude-plugins#6: with no Stop hook registered,
none of the ~2 second cost from the first experiment remains.

**`B` genuinely leaves the style unselected.** Its reply ("lets you check out
multiple branches ... simultaneously in separate folders") reads like A's,
not C's ("A git worktree lets you check out multiple branches from one
repository into separate directories at the same time" — terser, no
"simultaneously"). Confirms the harness measures what it claims to.

**`C`'s extra cost sits entirely in `ttft_ms`, not generation.** `C - B` on
`duration_ms` is 996 ms; on `ttft_ms` it is 1023 ms; on generation time after
the first token it is -27 ms, noise. Whatever the style costs, it costs before
the first token, not per token after. This fits a system-prompt section the
model reads before starting to write, not a slower write once started.

**Cache creation tokens do not explain the gap by themselves.** A, B and C
recorded 6896, 7027 and 7998 `cache_creation_input_tokens`: C's style content
is a real but modest addition, not the order-of-magnitude difference the cost
spike in the first experiment showed. That spike does not reproduce here.

### What this basic test does not settle

One repeat per cell, short prompt only, same session-container cache state
across all three runs rather than a cold one. Whether the `ttft_ms` gap holds
at more repeats, and whether it holds at medium and long prompts where the
model has more to plan before the first token, is unmeasured.

## Third experiment: the full design

5 repeats, 3 lengths, 3 conditions, 45 runs, `sonnet`, 2026-09-20, 0 errors.

| Condition | Length | `duration_ms` | `ttft_ms` | Cost USD | Output tokens |
|---|---|---|---|---|---|
| A | short | 2223 | 1119 | 0.0356 | 44 |
| B | short | 3083 | 2121 | 0.0361 | 44 |
| C | short | 2184 | 1228 | 0.0400 | 37 |
| A | medium | 5087 | 3832 | 0.0383 | 300 |
| B | medium | 5661 | 4269 | 0.0389 | 309 |
| C | medium | 6213 | 3943 | 0.0426 | 285 |
| A | long | 16623 | 15042 | 0.0466 | 1112 |
| B | long | 14678 | 13021 | 0.0470 | 1098 |
| C | long | 13912 | 12670 | 0.0498 | 985 |

Medians. Full spread, one row per run, is in
[`latency-results-0.9.1.ndjson`](latency-results-0.9.1.ndjson).

**The `ttft_ms` gap from the basic test does not replicate.** `C - B` on
`ttft_ms` is -893 ms (short), -326 ms (medium), -351 ms (long): C's
time-to-first-token is lower than B's at every length, the opposite direction
from the single-repeat result above. `B - A` is +1002 ms (short), +437 ms
(medium), -2021 ms (long): no consistent direction there either. At 5
repeats, the spread within a single condition and length (for example, `A`
short ranged 2081 to 3284 ms) is often larger than the gap between
conditions. This design, at this sample size, cannot tell the earlier
apparent effect apart from ordinary run-to-run noise in the API. The basic
test's finding stands as what it was: one data point, not a result.

**Cost orders `A < B < C` consistently, at every length.** Not close calls:
short 0.0356 / 0.0361 / 0.0400, medium 0.0383 / 0.0389 / 0.0426, long 0.0466 /
0.0470 / 0.0498. `C`'s premium over `B` runs 10.8% (short), 9.5% (medium),
6.0% (long): shrinking as a fraction of the total as the prompt grows, which
fits a roughly fixed per-turn addition (the style's own content in the system
prompt) landing on top of a base cost that scales with the prompt. `C` also
writes fewer output tokens than `A` or `B` at every length (37 against 44,
285 against 300 and 309, 985 against 1112 and 1098), so the extra cost is not
from writing more; it is an input-side cost, which fits the same reading.

**Read together: a real, small, consistent cost. No measured latency cost.**
The style has a per-turn cost, visible in tokens billed. Whether it has a
latency cost is not established at this sample size, and the one measurement
that looked like one, the basic test's `ttft_ms` gap, went the other way here.

## Files

- [`run-latency-harness.py`](run-latency-harness.py): the harness.
- [`latency-results-0.9.1.ndjson`](latency-results-0.9.1.ndjson): the third
  experiment's raw output, one row per run, kept as the record backing that
  section above.
- `--out` defaults to a path the caller chooses. A run kept only for its own
  sake, not written up here, has no reason to land in the repository.
