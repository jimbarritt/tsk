# Dreaming: offline memory consolidation in agent harnesses

Status: research note, read 2026-11-01. In 2026 "dreaming" names one thing in the agent community: a job that rewrites an agent's memory between sessions. Anthropic, OpenClaw and Letta document it. It is a capability of the harness that runs the agent, and tsk does not depend on it.

Raised by Jim, 2026-11-01, who hears people say that agents "dream". The question arose while reading about supervision: [supervisory-engineering-and-the-record-of-supervision.md](supervisory-engineering-and-the-record-of-supervision.md).

Sources: vendor documentation read by fetch, third-party articles, and the papers listed under References. A source marked "search summary" was seen only as a search result. A source marked "read" was fetched.

In 2026 the agent community uses it for one thing: an offline job that reads past session transcripts and a memory store, then merges, prunes and rewrites the memory between sessions. This note found no source for who first used the word for agent memory. The marks below show which sources were read and which are search summaries.

## Where the word appears in 2026

- **Anthropic Managed Agents "Dreams".** The official documentation describes a dream that "reads an existing memory store alongside past session transcripts, then produces a new, reorganized memory store". The inputs are one memory store and 1 to 100 past sessions. The output is a separate store, and "the input store is never modified". It is an asynchronous job that takes minutes to a few hours, in research preview, with access by request ([Dreams docs](https://platform.claude.com/docs/en/managed-agents/dreams), read). The announcement date conflicts: the Anthropic blog post is dated 2026-05-19 ([blog](https://claude.com/blog/new-in-claude-managed-agents), read) and SiliconANGLE reports it on 2026-05-06 ([article](https://siliconangle.com/2026/05/06/anthropic-letting-claude-agents-dream-dont-sleep-job/), read). This note does not resolve the conflict.
- **OpenClaw "Dreaming".** The documentation describes a "background memory consolidation system" in `memory-core`, with three phases named light, REM and deep. Only the deep phase writes `MEMORY.md`. It runs on a cron schedule, by default at 03:00, and it writes a diary file, `DREAMS.md` ([OpenClaw docs](https://docs.openclaw.ai/concepts/dreaming), read). A search summary dates its general availability to release v2026.4.5. The docs show `enabled: true` as the default, and a search summary describes it as opt-in. The conflict is unresolved.
- **Letta Code "Dreaming".** The documentation states that "Dreaming uses background subagents to review recent conversations". It is set with `/sleeptime`, and the memory is stored in git ([Letta Code docs](https://docs.letta.com/letta-code/memory), read).
- **Claude Code "auto dream".** A third-party article dated 2026-03-25 describes a background sub-agent that consolidates memory files in four phases, runs only after 24 hours and more than five sessions, and can be started with `/dream` ([article](https://www.implicator.ai/anthropic-adds-auto-dream-to-claude-code-fixing-memory-decay-between-sessions), read). The article states that Anthropic had announced nothing and that no documentation existed. The Claude Code memory page that this note read does not mention it. The name belongs to the community in the sources found.

## Older sources behind the word

The 2026 uses borrow the word. No source found states that a vendor chose it from these papers. The link is the commentators' analogy.

- **Sleep and memory in neuroscience.** Wilson and McNaughton (1994) found that place cells that fired together during a task fired together again in later sleep ([PubMed 8036517](https://pubmed.ncbi.nlm.nih.gov/8036517/), search summary). Diekelmann and Born (2010) review the memory function of sleep ([Nature Reviews Neuroscience](https://www.nature.com/articles/nrn2762), bibliographic record only). This is the closest source to the agent-memory sense.
- **Replay.** Lin (1992) introduced experience replay ([DOI 10.1007/BF00992699](https://doi.org/10.1007/BF00992699), record only). Shin and colleagues (2017) use a generator to replay data for earlier tasks, inspired by the hippocampus ([arXiv:1705.08690](https://arxiv.org/abs/1705.08690), read).
- **Wake-sleep.** Hinton, Dayan, Frey and Neal (1995) train a generative model with alternating wake and sleep phases ([Hinton's page](https://www.cs.toronto.edu/~hinton/absps/ws.htm), search summary). DreamCoder (Ellis and others, 2020) applies wake-sleep learning to a library of abstractions and trains on "imagined and replayed problems" ([arXiv:2006.08381](https://arxiv.org/abs/2006.08381), read).
- **Imagined rollouts.** In World Models (Ha and Schmidhuber, 2018) an agent is trained "entirely within a hallucinated dream" and the policy is transferred to the actual environment ([arXiv:1803.10122](https://arxiv.org/abs/1803.10122), read). Dreamer (Hafner and others, 2019) learns behaviours from trajectories imagined in a learned world model ([arXiv:1912.01603](https://arxiv.org/abs/1912.01603), read). This sense is about planning and does not rewrite a memory.
- **Image generation.** Google's DeepDream (2015) is unrelated to memory. The dates came from a secondary source and were not verified.
- **Sleep-time compute.** Letta's April 2025 term is "sleep-time", not "dreaming" ([Letta blog](https://www.letta.com/blog/sleep-time-compute), read; [arXiv:2504.13171](https://arxiv.org/abs/2504.13171), read). The paper precomputes inferences about a context to cut the cost per query. It does not rewrite a memory store. The Letta blog describes a sleep-time agent that edits memory blocks in the background, so its function overlaps with the 2026 uses.

## What the process changes, and what it records

No weights change in any agent-memory use found. The process rewrites text. Anthropic describes it as "a synthesis pass over the inputs, not an editor applied to the text of the store".

Records of the change differ:

- **Anthropic.** The input store stays unchanged and the output is a separate store, so a person reviews it and uses or discards it. Every change to a memory creates an immutable version, kept for 30 days, with an endpoint that redacts content ([Memory docs](https://platform.claude.com/docs/en/managed-agents/memory), read).
- **OpenClaw.** The previous `MEMORY.md` is saved before an accepted rewrite. `DREAMS.md` records the counts of entries added, merged and superseded, with short diff highlights.
- **Letta Code.** Memory updates are committed to git. An optional setting makes a second agent review the update before it applies.
- **Claude Code auto dream.** No record is documented. The third-party article reports that contradicted facts are deleted with "no flag, no comment".

None of these records a typed reason for each edit. They record the result: a version, diff counts or a commit. This bears on the idea of an [audit history of supervision](supervisory-engineering-and-the-record-of-supervision.md), because dreaming is a process that changes a robot's memory without a human reading each change.

## Risks the sources name

- **Silent deletion.** A GitHub issue, seen only through a search summary, reports that auto dream deleted 23 memory files in about a day with no record of what or why. It is a single unverified report.
- **Transcripts not masked.** The OpenClaw documentation states that secret values in transcripts are not masked before consolidation.
- **Injected content that persists.** Among the sources read, only OpenClaw removes candidates with untrusted provenance before consolidation.
- **Cost.** Anthropic states that cost grows with the number and length of the input sessions, and that a failed dream produces a partial output store.
- **Weak instructions.** Anthropic states that instructions aimed at specific lines "generally produce no change".

## How dreaming relates to tsk

This part is the note's own inference. tsk's memory is the ledger: typed records written by commands, with each change recorded in the ledger history. A dreaming process would be a robot that rewrites records or documents without a person reading the change. Anthropic's split of an input store from a reviewable output store is the nearest model to tsk's own habit of writing locally and publishing at set points.

## Not established

- Who first used "dreaming" for agent memory, and when. Reddit, Hacker News, and the memory features of Mem0, Zep, Cursor and OpenAI were not searched.
- Whether Claude Code records anything about an auto dream run, and whether Anthropic documents the feature. The Claude Code changelog was not read.
- The announcement date of Anthropic's Dreams, which the blog and the press give differently.
- Whether OpenClaw's Dreaming is on by default.
- Stickgold's work on sleep and memory, which a review would cite. No source for it was verified.

## References

- Anthropic, Managed Agents Dreams. https://platform.claude.com/docs/en/managed-agents/dreams (read).
- Anthropic, Managed Agents memory. https://platform.claude.com/docs/en/managed-agents/memory (read).
- Anthropic blog, "New in Claude Managed Agents", 2026-05-19. https://claude.com/blog/new-in-claude-managed-agents (read).
- SiliconANGLE, 2026-05-06. https://siliconangle.com/2026/05/06/anthropic-letting-claude-agents-dream-dont-sleep-job/ (read).
- Claude Code memory documentation. https://code.claude.com/docs/en/memory (read in part, no mention of auto dream).
- Implicator, 2026-03-25, on Claude Code auto dream. https://www.implicator.ai/anthropic-adds-auto-dream-to-claude-code-fixing-memory-decay-between-sessions (read). Second guide: https://claudefa.st/blog/guide/mechanics/auto-dream (read).
- OpenClaw, Dreaming. https://docs.openclaw.ai/concepts/dreaming (read). Release timing: https://thursdai.news/companies/openclaw (search summary).
- Letta Code memory documentation. https://docs.letta.com/letta-code/memory (read). Letta blog on sleep-time compute, 2025-04-21. https://www.letta.com/blog/sleep-time-compute (read).
- Lin and others, "Sleep-time Compute: Beyond Inference Scaling at Test-time", arXiv:2504.13171. https://arxiv.org/abs/2504.13171 (read).
- Wilson and McNaughton, "Reactivation of hippocampal ensemble memories during sleep", Science 265(5172):676-679, 1994. https://pubmed.ncbi.nlm.nih.gov/8036517/ (search summary).
- Diekelmann and Born, "The memory function of sleep", Nature Reviews Neuroscience 11(2):114-126, 2010. https://www.nature.com/articles/nrn2762 (bibliographic record only).
- Hinton, Dayan, Frey and Neal, "The wake-sleep algorithm for unsupervised neural networks", Science 268:1158-1161, 1995. https://www.cs.toronto.edu/~hinton/absps/ws.htm (search summary).
- Lin, "Self-improving reactive agents based on reinforcement learning, planning and teaching", Machine Learning 8:293-321, 1992. https://doi.org/10.1007/BF00992699 (record only).
- Shin, Lee, Kim and Kim, "Continual Learning with Deep Generative Replay", arXiv:1705.08690. https://arxiv.org/abs/1705.08690 (read).
- Ellis and others, "DreamCoder", arXiv:2006.08381. https://arxiv.org/abs/2006.08381 (read).
- Ha and Schmidhuber, "World Models", arXiv:1803.10122. https://arxiv.org/abs/1803.10122 (read).
- Hafner, Lillicrap, Ba and Norouzi, "Dream to Control: Learning Behaviors by Latent Imagination", arXiv:1912.01603. https://arxiv.org/abs/1912.01603 (read).
- Google DeepDream, 2015, dates from Know Your Meme only. https://knowyourmeme.com/memes/google-deepdream (search summary, unverified).
- Bug report on silent deletion by auto dream, via a mirror. https://claudeissues.com/issue/47959-auto-dream-deletes-memory-files-without-user-consent-23-files-lost-in-one-day (search summary).
