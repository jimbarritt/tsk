# OpenAPPA

Status: preview and RFC, version 0.30.0 (2026-09-30), MIT licence, Rust. By Archestra Inc.
The first commit is dated 2026-07-15. The paper was accepted to the NeurIPS 2026 Workshop
on Agents in the Wild.

Sources: a shallow clone of [archestra-ai/OpenAPPA](https://github.com/archestra-ai/OpenAPPA)
at commit `5060566` (2026-10-01), and the arXiv abstract page. From the clone: `README.md`,
`CLAUDE.md`, `summary.md`, `CHANGELOG.md`, the documentation in `website/content/docs/`,
`bench/corp/` and `marketplace/`. Quoted phrases were matched against those files. The
website itself and the full paper were not read. Benchmark figures are Archestra's own,
as reported, and were not run. The repository says its "config and wire surfaces may break
without shims".

## What it is

OpenAPPA is an information-flow policy engine for LLM agents. It answers one question
before every tool call: "is this data allowed to go to this destination?" The engine tracks
the sensitivity and trust of everything an agent has read, and checks each proposed call
against that record before the call runs. APPA stands for Agentic Permissions Policy
Algebra.

The README states the decision is deterministic: the engine "decides from the event log
alone and makes no network or file calls, so the same log always gets the same decision".
Policy is declarative TOML in a file named `appa.toml`.

The engine stays outside the agent loop, so a prompt injection cannot change a policy
rule. Any judgement that is not algebraic is in an external annotator, authority or
sanitizer that the policy names.

## Concepts

| Concept | Meaning |
|---|---|
| Trajectory | An agent's work on one conversation or task, including its tool calls. OpenAPPA keeps a security label and the policy configuration for each trajectory. |
| Label | Audience and Trust. A label can become more restrictive as the agent works and cannot become less restrictive. |
| Audience | Who may access the data in the session. Reading data for a smaller audience restricts where the agent can send data later. An audience can be as narrow as the members of one Slack channel. |
| Trust | How far the data can be trusted. It follows who wrote the text. Text from an outsider lowers it, and tools that require trusted input then stop being allowed. |
| Effect | An action already done, such as sending an email. Effects accumulate in the trajectory. A policy can require a recorded effect, or block an action after one is recorded. |
| Attention | An approval required for one call. An approval clears the requirement for that call only. |
| Tool contract | The rule for one tool: `delta` (how its output changes the label), `requires` (what the session must satisfy) and `effects` (what is recorded on success). |
| Remedy plan | When a call is blocked, the options the policy allows: clean data with a sanitizer, get approval from an authority, accept a narrower audience, withhold the result, or isolate the read in a subagent. |
| Authority | A person, an approval service or an LLM evaluator that can approve one blocked action. Approval does not loosen the label. |
| Sanitizer | Cleans data before the agent receives it or before a tool does. |
| Annotator | Classifies a tool call and sets its contract. It can be a script, a service or a model. |
| Battery | A reusable policy for a set of tools, such as Slack or the Claude Code built-ins. |
| Subagent read | A child trajectory reads sensitive data in a separate context and returns only what the policy allows. The parent sets the return requirements before the child starts. |

Contract matching: the engine "uses the first explicit contract whose argument selectors
match". A policy can contain one wildcard entry for undeclared tools. "Without a wildcard,
a call with no matching contract is refused before execution."

Rule order: the root file's rules, top to bottom, then each included battery in order. The
first match wins. A root rule can override a battery rule without editing the battery.

### Reserved mark and approvals

`requires = { attention = ["sre-signoff"] }` makes each call need a fresh approval. An
authority lists the marks it may give under `permits.attention`. The built-in `hitl`
authority is a human approval handler. `blocked` is the reserved mark that no authority can
permit: a tool that requires it has no remedy.

A battery example, `approve-small-payment`, "approves payments of USD 100 or less. Larger
payments remain blocked."

### Required ordering

`effects` and `requires.effects` express order. In the documentation's example, a database
migration requires that `backup.completed` was recorded and excludes a second migration
while one is allowed and not finished.

## Integration

| Host | How |
|---|---|
| Claude Code | `appa plugin install claude-code` deploys the runtime binary, registers lifecycle hooks (`PreToolUse` and `PostToolUse`) in the user's Claude Code settings, adds an `appa` MCP server, installs the `/appa-guide` skill and the `clappa` launcher, and includes the `claude-code` battery. A session started with `clappa` is protected and fails closed when the runtime is down. |
| kagent | A documented integration. |
| Python SDK, custom harnesses | The runtime embeds in-process, or runs as a sidecar that checks each call. |
| Archestra LLM proxy | Archestra's 1.4 release candidate implements OpenAPPA for Claude Code, Claude Desktop, Cursor, Codex, OpenCode, Copilot CLI, n8n and other agents that call a model through its proxy. The repository's `summary.md` records the integration design: native Rust bindings, with events kept in PostgreSQL. |

The `marketplace/` directory distributes two package kinds: plugins, which install support
for an agent host, and batteries, which supply provider policies. Batteries exist for Slack,
GitHub, Linear, Notion, Sentry, PostHog, PagerDuty, Databricks, Cloudflare, Google
Workspace, Grain, Hugging Face, LaunchDarkly, Microsoft Learn, Monday, xmemory and Archestra.

Coding agents get further features: file taint tracking (experimental, opt-in), isolated
file processing (experimental, opt-in), shell and native file tool rules, subagent return
checks and protected sessions.

## Testing and maintaining policy

- `appa describe --check` checks that the configuration loads.
- `appa replay` checks scripted tool calls against the decisions a test expects, without
  running the agent's tools. The documentation recommends making both a required CI check.
- The test set should cover permitted work and forbidden flows, "so a change cannot pass
  merely by blocking everything".
- An agent can report a confusing block with the `yell` tool. A separate maintenance agent
  can read the reports, propose a change to `appa.toml`, and test it. A person reviews the
  change. "A report does not authorize a policy change."

## How success is scored in the benchmark

Bench-Corp compares OpenAPPA with Microsoft FIDES and Claude Code auto mode on 20 corporate
workflow scenarios. It "scores each run strictly based on observable tool side effects
(files created, emails sent)" and "does not score conversation text or rely on LLM judges".
Each `scenario.toml` holds the user prompt, the enabled systems and two kinds of
ground-truth check: `utility` checks, which say the work happened, and `security` checks,
which say a violation did not. For a wire transfer, the utility check names the request
file, the amount and the beneficiary account. The security check names the same transfer
made without the secondary approver's authority. These checks belong to the benchmark. The
product does not read them.

## Reported results

Archestra reports:

- No scored attack succeeded against OpenAPPA in 1,320 evaluations, 600 from Bench-Corp and
  720 from AgentThreatBench.
- Task completion of 88 to 90% in Bench-Corp, against 37 to 45% for the FIDES
  configurations evaluated. FIDES attack success was 28 to 35%.
- The README table: task completion 89% for OpenAPPA, 90% for Claude auto mode and 41% for
  FIDES. Attacks that succeeded: 0%, 10% and 31%.
- Without subagent isolation, completion fell from 88.0% to 56.5%. Without guided recovery
  it fell to 35.0%. The documentation says the features interact.
- The paper abstract reports 64.2 to 91% utility across 6,600 controlled episodes.

## Stated limits and comparisons

- OpenAPPA checks where data can flow. It does not label actions as destructive,
  irreversible or out of scope. The documentation recommends running it beneath Claude
  Code's auto mode, which checks those properties.
- The policy language "does not express time windows and event counts directly". Those
  checks need code outside it.
- It does not accept policy written in natural language. Auto mode and Codex auto-review do.
- File taint tracking is "not yet a supported security boundary".
- The documentation compares OpenAPPA with Cedar, OPA, Dogwood, Claude Code auto mode and
  Codex auto-review.

## Not documented

- How a success criterion for a task is chosen or encoded in a production deployment.
- Any record of a mission, objective, plan or task. The unit is the trajectory.
- How the state of a trajectory passes to another actor, beyond the checked subagent return.
- Who appoints an authority, or how an authority is revoked.
- Any measure of a human reviewer's workload as policy blocks accumulate.
- The full paper's proofs, the website, and the contracts reference beyond the sections
  read.

## Comparison with tsk

| Aspect | OpenAPPA | tsk |
|---|---|---|
| Unit | A trajectory and its tool calls. | A Mission, delegated with a briefing. |
| What it governs | Whether a flow of data may go to a destination. | Who does what work, and how it is recorded. |
| Constraints | A tool contract per tool, checked before the call runs. Deterministic. | A briefing's Constraints and Execution constraints sections name them. The Execution constraints section lists "Permitted files. Attempt limit. Budget." No component enforces them. |
| Approval | Authority: a person, a service or an LLM, scoped by `permits`. A fresh approval for each call. `blocked` has no remedy. | Post, a placeholder, holds a scope of authority. Escalation to a post. Not designed. |
| Thresholds | A battery authority approves payments of USD 100 or less. | A spend above a threshold is outside a post's authority. Not designed. |
| Success | Not part of the product. The benchmark scores observable side effects against `utility` checks. | An Objective: a checkable end state, or a measure over time. |
| Ordering | `effects` and `requires.effects` require a recorded action before a later one. | Delta Gate: a capability is delivered only when its delta deploys and the system is healthy. |
| Delegation | A subagent is a child trajectory. The parent sets return requirements before it starts. The return crosses a checked channel. | A mission is delegated to another actor with a briefing. The actor writes a report. |
| Feedback on the rules | An agent calls `yell` on a confusing block. A maintenance agent proposes a policy change. A person reviews. | The mission report records what the briefing failed to give. Feedback only. |
| State | An event log per trajectory, rebuilt on resume. SQLite, with PostgreSQL in the Archestra integration. | An append-only log per actor. SQLite as a disposable cache. Not built. |
| Testing the rules | `appa replay` tests, a required CI check. | An Objective is checkable. No test harness for briefings. |
| Substrate | Agent-agnostic through hooks, an SDK and a proxy. | The domain model names no substrate. Independence is untested. |
| Product, Delta, Scale | Not modelled. | Four dimensions. Not built. |

## Convergence and difference

OpenAPPA and tsk agree on two points:

- Someone other than the agent sets the rules on its behaviour.
- A parent states requirements before a child starts. OpenAPPA's parent sets the return
  requirements. tsk's delegator writes a briefing.

Three more directions are shared, and tsk's design has none of them: an authority with
scoped permissions that approves a blocked action, a component that enforces the rules, and
tests that run the rules against permitted and forbidden cases. The first depends on Post.

They differ on five:

- OpenAPPA governs data flow per tool call. tsk governs delegated work per mission.
- OpenAPPA has no work record. tsk defines Mission, Objective, briefing, report and
  continuation.
- OpenAPPA's rules are TOML contracts that an engine enforces. tsk's constraints are text in
  a briefing, and no component enforces them.
- OpenAPPA states no success criterion. tsk defines every mission by one.
- OpenAPPA enforces, and its approvals are per call. tsk's Post would hold authority for a
  position that outlasts a call.

## Sources

- [archestra-ai/OpenAPPA](https://github.com/archestra-ai/OpenAPPA), commit `5060566`:
  `README.md`, `CLAUDE.md`, `summary.md`, `CHANGELOG.md`, `website/content/docs/` (how-it-works,
  contracts, batteries, coding-agents, claude-code, validation, self-improving-policies,
  evaluation, openappa-vs-auto-mode, openappa-vs-cedar, openappa-vs-opa,
  openappa-vs-dogwood), `bench/corp/`, `marketplace/`
- [APPA: Recoverable Information-Flow Control for Real-World LLM Agents](https://arxiv.org/abs/2607.24625),
  abstract page only

## Related

- [tsk-market-position-analysis.md](tsk-market-position-analysis.md): tsk's position, and
  the OpenAPPA section that links here.
- [../spicedb.md](../spicedb.md): a relationship-based permission store, set against the
  permissions a post would hold.
- [underlying-energy-constraints-of-running-a-factory.md](underlying-energy-constraints-of-running-a-factory.md):
  posts as an escalation target, and token spend as a budget decision.
- [openai-software-factory.md](openai-software-factory.md): risk classes and human approval
  inside OpenAI.
- [docs/domain/mission-briefing-template.md](../../domain/mission-briefing-template.md):
  Constraints and Execution constraints.
