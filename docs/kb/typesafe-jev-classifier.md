# TypeSafe AI's Jev classifier

Research note on Jev, a non-generative "decision model" from TypeSafe AI, and its use in
LangChain's agent harness and evaluation tooling. Raised by Jim, 2026-09-20, from the
agentic engineering news feed and
[typesafe.ai](https://typesafe.ai/), where Jim is on the waitlist.

This sandbox's egress proxy blocks `typesafe.ai`, `docs.typesafe.ai`, `www.langchain.com`
and `docs.langchain.com` directly, so nothing below is read from those pages themselves.
Everything comes from `WebSearch` result snippets over secondary and tertiary coverage,
plus one primary source read directly: the merged LangChain pull request that ships the
integration (`github.com/langchain-ai/langchain/pull/40542`), not blocked because it is a
different domain (`github.com`) from the two above.

## What Jev is

TypeSafe AI calls Jev a "System One Model": a model class built to make fast, structured
decisions rather than generate text. The name references Daniel Kahneman's System 1,
fast and intuitive, as distinct from System 2, slow and deliberate, the mode an LLM's
free-text generation sits closer to. Jev itself is named after William Stanley Jevons, on
the expectation that machine intelligence follows the same path Jevons described for
coal: efficiency gains increasing demand rather than reducing it.

TypeSafe AI is a San Francisco lab, founded by former OpenAI researcher Diogo Almeida,
that came out of stealth on 15 September 2026 with $40M in seed funding. Jev launched in
early access the same day. Access is through a waitlist at `typesafe.ai`; API keys are
issued through `console.typesafe.ai`.

## The API

One endpoint, `POST https://api.typesafe.ai/v1/systemone`, the early-access model
referenced as `jev-latest`. A caller submits application state plus one or more
questions, defined against a fixed schema, and gets back typed answers rather than
prose. Three question primitives:

| Primitive | Question shape | Returns |
|---|---|---|
| `Choice` | Pick one option from a defined set | The choice, per-option probabilities, a confidence value |
| `Score` | Rate the state against ordered descriptive levels | A score, with probabilities and confidence |
| `Noul` | A yes/no proposition | The probability the answer is yes |

Some secondary coverage (including LangChain's own blog, by search-snippet paraphrase)
calls the third primitive "Boolean". `Noul` is TypeSafe's own name for it, confirmed
across independent write-ups; not a typo found in only one source.

Each question is evaluated in parallel and in isolation against the same state, so
adding more questions barely changes latency, and one question's answer cannot leak
context into another. Because an answer can only be one of the schema's defined values,
an invalid or hallucinated answer is not possible in the way free-text generation allows
it. TypeSafe reports 70 to 500 ms end-to-end latency, at $0.042 per million input
tokens, with Jev averaging 67.8% agreement with reference probabilities on the
benchmark it reports this figure against (not independently checked here).

## The LangChain integration

`langchain-typesafe`, a first-party package, merged into `langchain-ai/langchain` as
pull request #40542, authored by Hunter Lovell (`@hntrl`). It implements the System One
HTTP contract directly with `httpx2`, rather than wrapping a separate TypeSafe SDK, to
keep native types and avoid transitive dependencies. Its main export,
`TypeSafeClassifier`, is a LangChain `Runnable`: state and questions go in through
`.invoke()`, typed answers, probabilities, confidence scores, usage metadata and a
request ID come back. Credentials resolve from explicit arguments or the
`TYPESAFE_API_KEY` and `TYPESAFE_BASE_URL` environment variables. Traces and token usage
record into LangSmith like any other LangChain component.

Two harness patterns LangChain describes building on top of this, in "Building a Harness
with Jev":

- **`ModelRouterMiddleware`**: a request router, backed by Jev, classifies task
  complexity and picks which model handles it, so simple work does not pay for a large
  model's reasoning.
- **`AutoModeMiddleware`**: a tool gate, backed by Jev, screens a proposed tool call for
  risk before it executes, and can block it. The same auto mode pattern already in
  Claude Code, Codex and Cursor, described here as reusable middleware.

## Jev-as-a-Judge

A separate LangChain piece, "Jev-as-a-Judge", applies the same classifier to agent
evals: Choice, Score or Noul questions about a run's behaviour, scored directly as typed
output instead of an LLM judge generating free text to be parsed. LangChain's benchmark,
run through LangSmith, reports:

- Jev's per-case sample variance 92 to 913 times lower than the LLM judges it compared
  against, i.e. a more consistent verdict on the same case run repeatedly.
- Jev and Claude Sonnet 4.6, used as an LLM judge, reached the same binary verdict on
  every case in the benchmark. Jev did so far cheaper: roughly $0.34 for the full run of
  evaluator calls, against $0.39 for one GPT model tested, $2.90 for another, and $28.17
  for Claude Sonnet 4.6 as judge.

Not independently checked here: the benchmark's case count, task domain, or whether
"the same binary verdict on every case" holds outside the specific cases tested.

## Why this belongs in tsk's own knowledge base

Jev is one data point in a wider convergence: a classifier sitting in front of, or
alongside, an LLM for cheap, structured decisions inside an agent's control loop, the
same shape as Claude Code's own auto mode classifier
(`docs/kb/claude-code-mods.md` covers a related but different Claude Code mechanism, the
hooks surface, not this classifier-in-the-loop pattern). Not yet cited in
`docs/kb/orchestration-ecosystem/tsk-market-position-analysis.md`; this note stands on
its own until that document is widened to include it.

Whether tsk itself has a use for a classifier like this, for example scoring thread
state, gating a risky action, or judging whether a mission's objective is met, is not
decided here. This is a research note, not a proposal.

## Open, not settled here

- Nothing here is read from a TypeSafe or LangChain page directly. All of it is
  `WebSearch` snippet coverage over the primary sources, except the merged GitHub pull
  request, which was read directly.
- The 67.8% reference-probability agreement figure and the 92 to 913 times variance
  figure are TypeSafe's and LangChain's own reported benchmarks, not verified against
  an independent run.
- Pricing, rate limits and general-availability timing beyond the waitlist are not
  covered by anything found.

## Sources

- [github.com/langchain-ai/langchain/pull/40542](https://github.com/langchain-ai/langchain/pull/40542),
  read directly: the merged `langchain-typesafe` integration.
- `WebSearch` snippet coverage over
  [langchain.com/blog/building-a-harness-with-jev](https://www.langchain.com/blog/building-a-harness-with-jev),
  [langchain.com/blog/jev-agent-evals-langsmith](https://www.langchain.com/blog/jev-agent-evals-langsmith),
  and [typesafe.ai/blog/introducing-system-one-models-and-jev](https://typesafe.ai/blog/introducing-system-one-models-and-jev),
  none read directly, blocked by this sandbox's egress proxy.
- `WebSearch` snippet coverage over third-party write-ups, none read directly: Forkast,
  DataCamp, TrueFoundry, refix.ai, and others returned by search, cross-checked against
  each other for the `Noul` primitive name and the TypeSafe AI company background.

## Related

- [tsk-market-position-analysis.md](orchestration-ecosystem/tsk-market-position-analysis.md):
  tsk's market position against other systems in this space. Does not yet cite Jev.
