# Copilot dynamic workflows

Status: public preview, announced 2026-10-01. Available on all Copilot plans, in Copilot
CLI, the GitHub Copilot app and the GitHub Copilot SDK.

Sources: the GitHub
[changelog entry](https://github.blog/changelog/2026-10-01-dynamic-workflows-in-copilot-cli-and-the-copilot-app/),
and the GitHub docs pages
[Dynamic workflows](https://docs.github.com/en/copilot/concepts/agents/dynamic-workflows)
and
[Using dynamic workflows](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/use-dynamic-workflows).
For comparison, the Claude Code page
[Dynamic workflows](https://code.claude.com/docs/en/workflows). Each page was read as raw
HTML. Quoted phrases were matched against the page text.

## What it is

"A dynamic workflow is a program that defines how a task is carried out." It combines
automated steps with the work of one or more agents, run in sequence, in parallel, or
both. "Everything is defined in code: the steps, when to involve agents, and how to use
their results. Agents handle the parts that need analysis or judgment."

GitHub separates it from two existing features by who defines the process:

| Feature | Who defines the process |
|---|---|
| Autopilot | "Copilot decides the next steps" |
| `/fleet` | "Copilot decides how to divide and coordinate the work" |
| Dynamic workflow | "The workflow author defines the steps, conditions, and handoffs" |

## Mechanics

| Aspect | Behaviour |
|---|---|
| Definition | Code inside a Copilot extension, an `extension.mjs` file. Copilot writes it on request, or a person writes it with Copilot's built-in authoring guidance. |
| Scope | A workflow Copilot writes belongs to the current session. Copying its directory to `~/.copilot/extensions/` makes it personal. Copying it to `.github/extensions/` shares it with a repository. A plugin distributes it through a marketplace. |
| Steps | Run commands, use tools, call services, run independent tasks in parallel, pass structured results between stages, have subagents verify each other's findings, and combine results into one answer. |
| Structured results | A workflow can ask an agent for a result in a set format. Copilot can ask the agent to correct the format. |
| Human input | A workflow can ask the user for input when the client supports it, and can pause at a checkpoint for review. |
| Start | A prompt that names the workflow, an extension's slash command, tool or hook, the SDK, a canvas in the Copilot app, or `copilot workflow run <name>` from a shell. |
| Headless run | `copilot workflow run` takes inputs as JSON through `--args`, writes the returned value with `--result-file`, and prints a JSON record of name, run ID, status and result with `--output-format json`. It shows no permission prompts, so permissions are granted before the run with `--allow-tool` or `--allow-url`. |
| Permissions | Subagents inherit the grants of the session that started them. A request outside those grants appears as a normal prompt, and the subagent waits. |
| Limits | Maximum concurrent agents, maximum total agents, maximum active running time, and an approximate maximum of AI credits. Set in the prompt, in the workflow code, or in personal settings, in that order of priority. |
| Stop at a limit | The concurrent limit makes agents wait. The other limits stop the run and keep its status and saved results. A resumed run needs a higher total, which counts the usage before the stop. |
| Credit limit | "This is an approximate maximum, not a hard ceiling." Usage is reported after it occurs, so work in progress can exceed the limit. |
| Monitoring | `/workflows` in the CLI, or a Workflows button in the app, shows active time, current phase, active and total subagents, and AI credits used. Runs can be paused, cancelled and resumed. |
| Resume | A paused run, or one stopped at a limit, reuses saved results from completed steps and subagents. "Work that was not saved may need to run again." A cancelled run cannot be resumed. |
| Sharing | Copying a workflow "copies the definition, not its run history or saved progress". |
| Telemetry | With OpenTelemetry on, each run or resume creates an `invoke_workflow` span with its agents' activity linked to it. |
| Schedule | `/every` and `/after` schedule a run in the current CLI session. |

## Examples GitHub gives

- Release checks with one agent assessing failures, then a pause for review.
- A parallel review of many changed files in a pull request.
- Unresolved review comments on merged pull requests, found by code, then two models
  asked whether each comment still needs action. "The workflow's code reports findings only when both
  agree."
- A sweep across many directories for a pattern, such as missing tests.
- Research, then a plan from the findings, then the change.

## Compared with Claude Code dynamic workflows

Claude Code has a feature with the same name. Both move the plan of a
multi-agent task into code.

| Aspect | Copilot | Claude Code |
|---|---|---|
| Definition | An extension, `extension.mjs` | A JavaScript script the runtime executes |
| Saved location | `~/.copilot/extensions/`, `.github/extensions/`, or a plugin | `~/.claude/workflows/`, `.claude/workflows/`, or a plugin. A saved workflow runs as `/<name>` |
| Start from a shell | `copilot workflow run <name>` | Through `claude -p` and a permission rule such as `Workflow(<name>)` |
| Mid-run human input | Supported when the client supports it | "No mid-run user input". A run pauses only for permission prompts and a usage-limit wait |
| Limits | Concurrent agents, total agents, running time, AI credits | Up to 16 concurrent agents by default, 1,000 agents per run |
| Resume | Saved results from completed steps | "Resumable in the same session". Saved results survive `claude --resume`, and in a cloud session they survive the VM being reclaimed |
| Bundled workflow | None documented | `/deep-research` |

## Comparison with tsk

| tsk term | Dynamic workflow |
|---|---|
| Plan | A tsk plan belongs to the actor and is rewritten as execution proceeds. A workflow fixes the plan in code. The path through it changes with inputs and findings, but the steps and rules stay the same. |
| Task | A workflow runs one task, or one stage of a task. It has no mission, objective, purpose or briefing. |
| Execution constraints | The briefing names "Permitted files. Attempt limit. Budget." as text. A workflow enforces agent counts, running time and an approximate credit budget. |
| Objective | A tsk objective is a checkable end state. A workflow returns a result. The workflow defines no success criterion. |
| Verification | Agents that cross-check each other, and code that reports only when two models agree, are inference-based checks inside a deterministic check in code. |
| Thread continuation | A run resumes from saved results within the scope where those results are kept. Sharing a workflow does not include its run state. tsk's continuation entry is written to the ledger and read by any later actor. |
| Report | A run returns a value and a status. It has no account of what the inputs failed to give. |
| Actor | Subagents have no identity or memory outside the run. |
| Unattended run | `copilot workflow run` with `--output-format json` returns a run ID and a status, which is part of the run record that M-BOOT-03 names. |
| Telemetry | The `invoke_workflow` span is a per-run trace. tsk has no telemetry. |

## Not established

- Whether a run's saved results persist after the Copilot session ends. The docs say
  only that a shared definition does not include them.
- What a checkpoint pause looks like in the extension API.
- How a workflow behaves in the Copilot cloud agent. The docs name the CLI, the app and
  the SDK only.
