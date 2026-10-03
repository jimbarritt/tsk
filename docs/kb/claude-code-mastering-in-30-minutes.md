# Mastering Claude Code in 30 minutes

Talk by Boris Cherny, a member of technical staff at Anthropic who created Claude Code.
Published by Anthropic on YouTube,
[youtube.com/watch?v=6eBSHbLKuN0](https://www.youtube.com/watch?v=6eBSHbLKuN0). A podcast
index dates it 22 May 2025 and lists 28 minutes. The transcript site titles it Code w/ Claude. Questions at the end come from the audience.

Source: a machine transcript from
[sozai.app](https://sozai.app/transcript/mastering-claude-code-30-minutes/), rated 95%
accurate by that site, and the outline on
[Podwise](https://podwise.ai/episodes/4254885). The video was not played. YouTube returned an
error to automated access. The transcript has errors. It renders the error-tracking tool Sentry as
"century". It lists a duration of 41:46, which does not match the 28 minutes on Podwise,
so this note gives no timestamps. Quoted phrases were matched against the transcript.

## Position

Claude Code is "a new kind of AI assistant". Earlier assistants completed "a line at a
time". Claude Code is "fully agentic": it is for "building features, for writing
entire functions, entire files, fixing entire bugs". It "works with every
single IDE, every terminal out there", locally, over remote SSH or over tmux. It is a power
tool, and Anthropic does not "try to guide you towards a particular workflow".

## Recommended order of adoption

| Step | Advice |
|---|---|
| 1. Set up | Run `terminal setup` for shift-enter newlines. `/theme` sets light, dark or colour-blind themes. `/install-github-app` adds the GitHub app announced that day, for mentioning Claude on an issue or pull request. Customise the allowed tools so common prompts are not asked each time. On macOS, enable dictation and speak prompts. |
| 2. Ask questions about the codebase | "The thing I recommend above everything else". New Anthropic hires do this on their first day. Onboarding "used to take about two or three weeks for technical hires, it's now about two or three days". |
| 3. Edit code | Claude has a small tool set: edit files, run bash commands, search files. It strings them together without being told. |
| 4. Plug in team tools | Bash tools and MCP tools. Tell Claude about the tool and how to use it, for example by running its `--help`. |
| 5. Use a workflow | Explore, plan, then ask for confirmation before writing code. Or give Claude a way to check its work and iterate. |
| 6. Give it context | `CLAUDE.md`, slash commands, mentioned files, and the configuration hierarchy. |
| 7. Learn the shortcuts | See [Key bindings](#key-bindings). |
| 8. Script it | The SDK, which is `claude -p`. |
| 9. Run sessions in parallel | The most advanced use. |

Do not start "by using fancy tools" or "by editing code". Start by asking questions. That
will "teach people how to prompt" and shows where the boundary lies: "what can be one-shotted,
what can be two-shotted, three-shotted", and what needs interactive mode.

## Codebase Q&A

- Claude Code does "a level deeper" than a text search. It finds examples of how a class is
  instantiated and used.
- It can fetch GitHub issues and look up their context.
- It reads Git history. Asked why a function has 15 oddly named arguments, it finds who
  added them, in which situation, and which issues the commits link to.
- Nothing in the system prompt tells it to read Git history. Cherny credits the model:
  "the model is awesome". Claude uses Git when told to.
- Every Monday, for the weekly stand-up, he asks what he shipped that week. Claude reads
  the log, which holds his username, and gives a read-out that he pastes into a document.
- Privacy: "we don't do any sort of indexing, so there's no remote database with your
  code, we don't upload it anywhere, your code stays local, we do not train generative
  models on the code". Because there is no index, there is no setup wait.

## Editing and workflow

- Asking for a 3,000-line feature in one step sometimes gives something "not at all the
  thing that you wanted". Ask Claude to "brainstorm ideas, make a plan, run it by me, ask
  for approval before you write code".
- "You don't have to use plan mode, you don't have to use any special tools to do this, all
  you have to do is ask Claude."
- "Commit, push, PR" is a phrase Cherny uses often. Claude reads the Git log for the commit
  format, commits, creates the branch, pushes and opens the pull request.
- Claude can check its own work with unit tests, Puppeteer screenshots or the iOS
  simulator, and then iterate. Given a mock and a request to build the web UI, two or
  three iterations "often it gets it almost perfect". The advice is to "give it some sort of
  tool that it can use for feedback".

## Context

| Mechanism | Behaviour |
|---|---|
| `CLAUDE.md` in the project root | Read at the start of every session. "Essentially the first user turn will include the Claude MD." Check it into source control and share it. |
| Local `CLAUDE.md` | Not checked in. For one person. |
| Contents | Common bash commands, common MCP tools, architectural decisions, important files. Keep it short, because a long file is "just going to use up a bunch of context". |
| `CLAUDE.md` in nested directories | Pulled in on demand when Claude works in that directory. |
| Enterprise root `CLAUDE.md` | A file shared across all codebases that a company manages for its users. |
| Slash commands | Files in `.claude/commands`, in the home directory or checked into a project. Anthropic runs a GitHub Action that runs a label-issues slash command, so issues are labelled without a person. |
| Mentioned files | Pulled into context. |

Cherny advises "taking the time to tune context", for example by running it through the
prompt improver, and to consider who the context is for, whether it loads every time or on
demand, and whether it is shared or personal.

### Configuration hierarchy

Configuration comes at three levels: a project, specific to a Git repository, which can be
checked in or kept personal; global configuration across all projects; and enterprise
policies, "a global config that you roll out for all of your employees". The hierarchy
applies to memory, slash commands, permissions and MCP servers.

- An enterprise policy file can pre-approve a command for every employee.
- It can block a command or a URL, and "an employee cannot override it".
- A checked-in MCP JSON file prompts anyone who runs Claude Code in the repository to
  install the shared servers. Anthropic's apps repository shares a Puppeteer server this
  way, so engineers can pilot end-to-end tests and screenshots without installing it.
- If unsure where to start: "start with shared project context". "you write this once, and
  then you share it with everyone on the team", and the team benefits as each person adds.
- `/memory` lists the memory files in use and edits them. Typing the pound sign to
  remember something lets the user choose which memory file receives it.

## Key bindings

| Key | Effect |
|---|---|
| Shift-Tab | Accept edits: switches to auto-accept mode. Bash commands still need approval. Edits can be undone later. |
| `#` | Tell Claude to remember something. It is added to `CLAUDE.md`. |
| `!` | Bash mode. The command runs locally and its output goes into the context window, so Claude sees it next turn. |
| `@` | Mention files and folders. |
| Escape | Stops Claude. "No matter what Claude is doing, you can always safely hit escape, it's not going to corrupt the session." |
| Escape twice | Jumps back in history. |
| `--resume`, `--continue` | Resume a session after it ends. |
| Ctrl-R | Shows the whole output, "the same thing that Claude sees in its context window". |

## SDK and parallel use

- The SDK is `claude -p`. A caller passes a prompt, allowed tools (which can include
  specific bash commands) and an output format of JSON or streaming JSON. It is the SDK
  that Claude Code itself uses. Anthropic uses it "in CI all the time", for incident
  response and in pipelines.
- Cherny describes it as a Unix utility: pipe `git status` in and select from the result
  with `jq`, or pipe in a large log from a cloud storage bucket, or the output of an
  error-tracking command line tool.
- Cherny describes himself as "a Claude normie" who usually runs one Claude and a few
  terminal tabs for different repositories.
- Power users "almost always" use SSH sessions and tmux tunnels into their Claude sessions.
  They keep several checkouts of one repository, or use Git worktrees for isolation.
  "We're actively working on making this easier to use." "You can run as many sessions as
  you want."

## Questions and answers

- **Hardest part to build**: making bash commands safe. Bash "can change system state in
  unexpected ways", and approving every command makes an engineer unproductive. Anthropic's
  approach has these parts: some commands are read-only, static analysis finds which
  commands can be combined safely, and "this pretty complex tiered permission system" lets
  allow lists and block lists apply at different levels.
- **Images**: Claude Code "is fully multimodal". An image can be dragged in, pasted in or
  given as a file path. Cherny gives it a mock and a Puppeteer server so it can iterate.
- **Machine learning use**: "about 80% of people at Anthropic that are technical use Claude
  Code every day", including researchers who use the notebook tool.

## Not covered

The transcript has no mention of subagents, hooks, compaction or effort settings. It
mentions plan mode once, to say it is not required. It mentions Git worktrees once, as an
isolation technique for parallel sessions. The talk dates from May 2025. Features added
since then are absent.

A LinkedIn post of 28 September 2026 describes a 15-minute video of Cherny. The post links
no video. Its list of topics includes plan mode, subagents and hooks. This talk has no
subagents or hooks, and plan mode appears once.

## Mapping to tsk

Facts about tsk are from its own documents. No design is implied.

| Talk | tsk |
|---|---|
| `CLAUDE.md` is read at the start of every session and shared through source control. | tsk's `CLAUDE.md` is project instructions that override the user's global one for task tracking. |
| Configuration at project, global and enterprise levels, and policies an employee cannot override. | [claude-code-mods.md](claude-code-mods.md) records five mod tiers: prepend, user, append, built-in and core. tsk has no permission model. |
| Give Claude "some sort of tool that it can use for feedback" and it iterates. | A tsk Objective is a checkable end state. |
| `claude -p` runs Claude in CI and pipelines. | M-BOOT-03's objective includes one unattended cloud routine run that produces a pull request, a run record and thread state. Not built. |
| Power users run several sessions with checkouts or Git worktrees, and tmux. | Mission Control, M-BOOT-06, is a tmux layout of Claude sessions, each in its own worktree that the user creates. |
| Plan first and "ask for approval before you write code". | A mission briefing contains a Plan section that the actor takes ownership of before any other action. |

## Sources

- [Mastering Claude Code in 30 minutes, Anthropic, YouTube](https://www.youtube.com/watch?v=6eBSHbLKuN0),
  channel and title verified through YouTube's embed metadata, not played
- [Transcript, sozai.app](https://sozai.app/transcript/mastering-claude-code-30-minutes/)
- [Episode outline and date, Podwise](https://podwise.ai/episodes/4254885)
- [LinkedIn post, 28 September 2026](https://www.linkedin.com/posts/ikekehal_i-made-this-15-minute-video-mandatory-for-activity-7510300562404335616-9dAe)

## Related

- [claude-code-mods.md](claude-code-mods.md): hooks and plugin tiers.
- [session-creation-and-environments.md](session-creation-and-environments.md):
  orchestrating agent sessions.
- [docs/domain/mission-briefing-template.md](../domain/mission-briefing-template.md): the
  Plan section of a briefing.
