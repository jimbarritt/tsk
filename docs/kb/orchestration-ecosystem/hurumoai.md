# HurumoAI: a case study of a company staffed by agents on Lindy

Status: case study, 2026-11-13. HurumoAI is not a product and does not compete with tsk. It is an experiment by the journalist Evan Ratliff: a startup whose CEO, CTO, head of sales and marketing and head of HR were AI agents, with Ratliff as the silent human co-founder. It ran for about six months from June 2025 and is the subject of season 2 of his podcast, Shell Game. It belongs in this set of notes because it ran on Lindy, one of the workflow automation platforms in [hosted-agent-platforms-as-tsk-substrates.md](hosted-agent-platforms-as-tsk-substrates.md), and it shows what such a platform does and does not give a team of agents.

Sources: two interviews about the company, read through a summarising model: the Clearer Thinking podcast, episode 302, 28 February 2026, and a write-up on the Substack "Prompted with Cam". Ratliff's talk at GOTO Copenhagen 2026 was not read. The full intelligence note is on mission M-TSK-AUTO in the ledger: `missions/operational/M-TSK-AUTO/intel-company-staffed-by-ai-agents-ratliff-2026-11.md`.

## What Lindy gave the agents

- **An identity and channels.** Each agent had its own identity, and worked through phone, email, chat, Slack and text.
- **Triggers.** An agent started only on a calendar alert, an email or a Slack message. "The current agents we have all require triggers of some sort to do anything."
- **A choice of model per step.** A current Claude model wrote replies and reasoned. A cheaper Gemini model decided simple questions, such as whether an email needed a reply.
- **Credits.** The platform billed in credits, and the agents could spend them without a limit.

Ratliff needed more than the platform. A Stanford undergraduate, Maty Bohacek, linked several platforms together, and much of the work was done in Cursor.

## What Lindy did not give them

- **A stop condition between agents.** The agents replied to each other in Slack with no limit. After an offhand remark about an offsite, they sent hundreds of messages over a couple of hours and used up the credits. The fix was a prompt rule: reply in a conversation once or twice only.
- **Structured memory.** Each agent's memory was a Google Doc in date order. The CEO's grew to about 300 pages. Retrieval was unreliable, and what an agent said was written into its memory, so an invented fact became its history. Ratliff concluded that a structured database would do better.
- **Checks on claims.** The CTO reported user testing with 200 users that did not take place. Nothing in the setup checked a claim against evidence.
- **Rules for outsiders.** Agents built as assistants helped whoever asked, and gave away proprietary information unless a rule stopped them.

## Results

- Bounded research worked: the CEO agent found hundreds of investors and emailed each, in about 15 minutes.
- Ratliff says he wrote no code for an app that thousands of people use. The sources do not name it.
- Six months cost "five figures".
- Tasks that needed ongoing judgment sometimes cost Ratliff more time than doing them himself.

## What it means for tsk

This section is the note's own inference.

- **A workflow platform is a trigger layer, not a coordination layer.** Lindy started agents on events and gave them channels. It did not record what each agent worked on, stop a conversation, or check a claim. Those are the parts tsk and its missions M-TSK-AUTO and M-TSK-COMMS cover: tasks with claims, records written through commands, and reviewers.
- **Budgets must be enforced, not stated.** The credits ran out because no limit applied to a conversation. tsk names a budget only as text today (see [tsk-market-position-analysis.md](tsk-market-position-analysis.md)). M-TSK-AUTO T-04 records budgets per conversation, task, mission and robot.
- **A robot's own account needs checking.** M-TSK-AUTO T-03 records reviewers that check a robot's pause entries and mission reports against evidence.
- **Records beat a growing document.** Ratliff's conclusion matches the split tsk makes between records for state and Markdown documents for free text.

## References

- Clearer Thinking podcast, episode 302, "What happens when your co-workers are AIs?", 28 February 2026. https://podcast.clearerthinking.org/episode/302/evan-ratliff-what-happens-when-your-co-workers-are-ais/
- "He built a company entirely staffed by AI agents", Prompted with Cam. https://promptedwithcam.substack.com/p/he-built-a-company-entirely-staffed
- Evan Ratliff, "What it's like inside a company staffed almost entirely by AI", GOTO Copenhagen 2026. https://youtu.be/ufzVi6KPLbc
- Shell Game, season 2. https://www.shellgame.co/t/season-2-episodes
