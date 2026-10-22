# Claude Code dynamic workflows and ultracode

Status: a shipped Claude Code feature, read 2026-10-22. The page carries no date. It
names Claude Code versions from v2.1.216 to v2.1.305. Available on all paid plans, with
Anthropic API access, and on Amazon Bedrock, Google Cloud's Agent Platform and Microsoft
Foundry. On Pro it is switched on from the Dynamic workflows row in `/config`.

Raised by Jim, 2026-10-22, who wrote "ultramode". The page's name for it is
`ultracode`.

Sources: the Claude Code page
[Orchestrate subagents at scale with dynamic workflows](https://code.claude.com/docs/en/workflows),
fetched and read in full on 2026-10-22. Pages it links to were not read: model
configuration (when ultracode is available), the settings reference, agent view, the
subagent concurrency limit and the Agent SDK reference. Nothing was run in a workflow for
this note. Claims are the page's own.

## What it is

"A dynamic workflow is a JavaScript script that orchestrates many subagents at once.
Claude writes the script for the task you describe, and a runtime executes it in the
background while your session stays responsive."

The page names four ways to run a multi-step task:

| | Subagents | Skills | Agent teams | Workflows |
|---|---|---|---|---|
| Who decides what runs next | Claude, turn by turn | Claude, following the prompt | The lead agent, turn by turn | The script |
| Where intermediate results live | Claude's context window | Claude's context window | A shared task list | Script variables |
| What is repeatable | The worker definition | The instructions | The team definition | The orchestration itself |
| Scale | A few delegated tasks per turn | Same as subagents | A handful of long-running peers | Dozens to hundreds of agents per run |
| Interruption | Restarts the turn | Restarts the turn | Teammates keep running | Resumable in the same session |

The page's core statement: "A workflow moves the plan into code." Claude's context holds
only the final answer. The code can also apply a repeatable quality pattern, such as
independent agents that review each other's findings before they are reported.

## Ultracode

Three routes to a workflow:

| Route | Effect |
|---|---|
| Ask in your own words ("use a workflow") or put the keyword `ultracode` in a prompt | Claude writes a workflow for that one task. The keyword only changes how the work is structured. The agents' tool calls get the same permission checks and sandboxing |
| `/effort ultracode`, or `claude --effort ultracode` (which also sets effort to `xhigh`), or the `ultracode` setting | For the whole session, Claude plans a workflow for each substantive task. One request can become several workflows in a row: "one to understand the code, one to make the change, and one to verify it" |
| Run a saved or bundled workflow by name | `/deep-research <question>` is the one bundled workflow. A saved workflow runs as `/<name>` |

The page states the cost of the session setting: "each request uses more tokens and takes
longer than the same request without a workflow. On a subscription plan those tokens draw
on your usage limits." With ultracode on, three checks do not apply:

- the `Large workflow` warning does not appear;
- the session's concurrent subagent limit is not enforced for subagents started with the
  Agent tool;
- in auto permission mode, the first workflow launch is not put to you for approval.

Where the keyword works: only in a prompt a person types (the interactive prompt, an IDE
panel, a Remote Control client, or an Agent SDK application that marks input as human).
It does not start a workflow from `claude -p`, from unmarked Agent SDK input, from a
scheduled task prompt, or from a webhook payload or pull request comment relayed into the
conversation. In those runs a permission rule `Workflow` or `Workflow(<name>)`, auto
mode, bypass mode, a `PreToolUse` hook or the host approves the launch.

## Mechanics

| Item | Finding |
|---|---|
| Script | Plain JavaScript with top-level `await`. `agent()` starts one subagent, `pipeline()` runs one per item in a list, `parallel()` runs a set at once. `phase()`, `log()` and the global `args` are also available. `import()` fails before the run starts |
| Determinism | `Date.now()`, `Math.random()` and a no-argument `new Date()` throw, so that a relaunched run repeats the same calls. Pass a timestamp in through `args` |
| Structured output | A `schema` on `agent()` makes the subagent return JSON of that shape. Validation retries five times, set by `MAX_STRUCTURED_OUTPUT_RETRIES` |
| Isolation | The runtime runs the script apart from the conversation. The script has no filesystem or shell access of its own. Agents read, write and run commands. An example migrates each file "in its own isolated copy" |
| Limits | 16 concurrent agents by default (1 to 256 with `CLAUDE_CODE_WORKFLOW_MAX_CONCURRENT_AGENTS`, v2.1.269 or later), 4,096 items in one `parallel()` or `pipeline()`, 1,000 agents in a run |
| Mid-run input | None. "A run pauses on its own only for agent permission prompts and a usage-limit wait. For sign-off between stages, run each stage as its own workflow" |
| Resume | In the same session, from `/workflows` with `p`. Completed agents return saved results. The first agent whose prompt differs from the previous run, and every agent after it, runs again. A failed agent and every agent started after it run again |
| After leaving | A backgrounded session continues the run. An exit with `Move to background and exit` carries it over. A session resumed with `claude --resume` replays saved results when asked to relaunch. In a cloud session the results are saved with the conversation history, so they survive the VM being reclaimed |
| Usage limit | A run waits for the reset and continues (v2.1.271 or later), on an interactive claude.ai session, when the limit resets within 24 hours, up to two waits. Elsewhere the affected agent fails |
| Stalls | An agent with no output for the stall window restarts from the same prompt, up to five times, then fails. `stallMs` or `CLAUDE_ASYNC_AGENT_STALL_TIMEOUT_MS` sets the window |
| Script on disk | Every run writes its script under the session's directory in `~/.claude/projects/`, and Claude receives the path |
| Saved workflows | `.claude/workflows/` in the project, shared with the clone, or `~/.claude/workflows/`. A plugin can ship them in a `workflows/` directory at its root, namespaced by the plugin name, such as `/acme-tools:release-audit` |
| Size guideline | `workflowSizeGuideline`: `small` under 5 agents, `medium` under 10 (default), `large` under 50, `unrestricted`. Advice to Claude, not a cap |
| Cost | `Large workflow` warning above 25 agents or a projected 1.5 million tokens. An agent's model follows the subagent order, and a script can name a model for a stage. Matching agents share a prompt cache for 5 minutes by default, 1 hour with `subagentPromptCacheTtl` |
| Switch off | `/config`, `"disableWorkflows": true`, `CLAUDE_CODE_DISABLE_WORKFLOWS=1`, or managed settings. Ultracode then disappears. No managed setting disables ultracode alone |

## Compared with tsk

| tsk term | Dynamic workflow |
|---|---|
| Mission, objective | None. A workflow has a task and a returned value. The page does not describe a success criterion |
| Plan | The script fixes the plan. A tsk plan belongs to the actor and changes as work proceeds. Ultracode makes Claude write a new script for each substantive task, so the plan is rewritten per task, and no ledger records it |
| Task | A tsk task is a record with a status. A workflow can run one task, or one stage of one |
| Verification | The page's own example is adversarial review of findings by independent agents. tsk does not check a mission's objectives: a mission is done "when its status is set to `done`" |
| Execution constraints | tsk's briefing names attempt limit and budget as text. A workflow enforces agent counts and stall handling. Ultracode switches off the size warning and the concurrent subagent limit |
| Thread continuation | Resume works inside a session, or from a session's saved history. The script is a file on one machine's `~/.claude/projects/`. A tsk continuation entry is in the ledger, and any later actor on any machine reads it |
| Actor | Subagents carry no identity beyond the run. Whether a workflow's subagents count as the session's actor, for a tsk thread binding, is not established |
| Handoff and sign-off | No mid-run input. The page advises one workflow per stage, with the sign-off between them. tsk's thread pause and resume sit at the same boundary |
| Event-driven start | The keyword does not work from a scheduled task, webhook payload or pull request comment. A tsk external event that starts work has to use a named workflow with a permission rule |

## Using it with tsk

Four combinations follow from the page. None was tested.

1. **A workflow as the executor of a tsk task.** A tsk actor takes a task with
   `tsk task set ... in-progress`, then runs the task as a workflow with the `ultracode`
   keyword or a saved workflow. The mission's objectives and constraints go in as `args`.
   The final answer returns to the actor, which records the outcome with `tsk task set`,
   `tsk decision add` and a continuation entry. Intermediate results stay in script
   variables and never reach the ledger unless an agent writes them.
2. **A workflow that checks objectives.** tsk holds objectives as records with a `met`
   flag and checks none. A saved workflow could read a mission with
   `tsk mission list <id> --json`, send one verifying agent per objective, and set `met`
   only for those two independent agents confirm. This is the page's adversarial review
   pattern applied to tsk's checkable objective. It would run as an actor action and not
   as part of the `tsk` binary.
3. **A workflow shipped in the tsk plugin.** The plugin root `plugin/` can carry a
   `workflows/` directory. A workflow there runs as `/tsk:<name>` for every user of the
   plugin. This is a delivery route for the combinations above, with no change to the
   binary.
4. **A continuation entry that names the run.** When a thread pauses with a workflow
   unfinished, the entry could name the script path under `~/.claude/projects/`. This
   helps the same actor in the same session or a resumed one. A different actor on
   another machine cannot read that path, so the entry has to carry the stage reached and
   what remains in text.

Cost and control, from the page:

- A session with ultracode on removes the size warning and the concurrent subagent limit,
  the two checks that otherwise bound a run. A briefing's "Budget" has no enforcement in
  Claude Code. For a tsk mission with a budget, turn ultracode off or choose a named
  workflow. A size guideline of `small` or `medium` is advice only.
- Event-driven runs need a permission rule for `Workflow` or `Workflow(<name>)`. A
  session that a tsk external event starts needs the rule.

## Position

Consistent with the position Jim recorded on 2026-10-16: the mechanics of durable and
proactive running moves into harnesses, and tsk keeps the semantic layer. Claude Code now
supplies the orchestration (a script, a runtime, resume, a usage-limit wait) and tsk
supplies the model of the work (mission, objective, constraint, report, continuation
across actors). The two combine at the task: tsk says what is to be done and when it
counts as done, and a workflow carries out one task with many agents.

No change to the three-part verdict in `tsk-market-position-analysis.md`.

## Not established

- Whether a workflow's subagents can run `tsk` commands under default permissions, and
  whether a thread binding applies to them. They run in the session or in isolated
  copies, and the binding is per session and per code worktree.
- Whether a plugin's `workflows/` directory works for the tsk plugin as the page states.
- When ultracode is available on a plan. The page links to a model configuration page
  that was not read.
- The token cost of a tsk mission run with ultracode. No run was made.
- Whether a saved workflow can read `tsk` records the way combination 2 assumes.
- The date the feature shipped. The page has no date, and the Copilot note of 2026-10-05
  already cites it.
