# Coder

Coder is a self-hosted, open source platform that runs AI coding agents in
Terraform-defined workspaces, with central logging for model providers and an allow list for the network
destinations and tools of an agent.

Status: open source under the AGPL v3.0, with a Premium licence for enhanced support and
custom deployments. Captured 2026-11-19 from a summary that Jim pasted. The sources the
summary cites were not read for this file.

Sources, as the summary names them: coder.com/docs, the coder.com blog (December 2025
announcement), InfoQ (May 2026) and Contrary Research. Every claim below is from the
summary and is a claim of Coder or of a reporter, not tested here. Read the cited pages
before a decision relies on a claim.

## What Coder is

Coder is a self-hosted platform for running AI coding agents and cloud development
environments on infrastructure that a team controls. It works with any cloud, IDE,
operating system, Git provider and identity provider.

## How Coder works

- A workspace is defined in Terraform. It connects through a WireGuard tunnel and shuts
  down when idle.
- Agents and developers use the same workspace infrastructure.
- Coder Agents is a native coding agent. Its loop runs in the control plane on the
  self-hosted infrastructure, not in the workspace and not in a vendor cloud. A workspace
  has no LLM API keys.
- It accepts any model: Anthropic, OpenAI, Google, Bedrock or a self-hosted model.
- Third-party agents, such as Claude Code and Codex, also run in workspaces.
- A person uses a web UI. A script or CI calls a REST API.

## Governance features

Announced December 2025.

| Feature | What the summary states it does |
|---|---|
| Coder Tasks | An execution engine for long-lived, low-interaction jobs: code review, docs, issue solving and test authoring. It has a full API and notifications. |
| AI Bridge | Central access, authentication and observability for all model providers. It consolidates prompt logs, usage patterns and token consumption. |
| Agent Boundaries | Policy controls that work as a firewall for an agent. Allow lists cover network destinations, permitted tools and internal systems. |

## Install

```
curl -L https://coder.com/install.sh | sh
coder server
```

Production needs PostgreSQL 13 or later and an external access URL.

## Suitability of Coder

- It suits teams that need audit, identity and network control over agents: platform
  teams and regulated enterprises.
- A solo setup is a weaker fit than these teams.
- It hosts agents in Terraform-defined workspaces. It is not a general-purpose agent
  runtime.

## Comparison against tsk

| Question | Coder | tsk |
|---|---|---|
| What is the unit of work? | A workspace that runs an agent. A Coder Task is an execution job. | A mission, with tasks, decisions and questions as ledger records. |
| Where is state about the work stored? | Not described in the summary, apart from prompt logs and usage in AI Bridge. | The ledger, a Dolt database, with thread continuation entries. |
| Which layer? | Where an agent runs, and the policy around it. | What the agent works on, and what a later actor reads to continue. |
| Which agent? | Any: its own, Claude Code, Codex. | Any harness that calls `tsk`. |

- **Task**: a Coder Task is a job to run. A tsk task is a record of work to do. The word "task"
  names a job in Coder and a record in tsk.
- **Substrate**: Coder is a candidate place to run tsk, as the Dust, Dify, n8n and
  StackAI cases are in
  [hosted-agent-platforms-as-tsk-substrates.md](hosted-agent-platforms-as-tsk-substrates.md).
  It was not tested. A test needs a workspace with `tsk` and `dolt` installed, a nexus
  attached and a robot signing key set.
- **Policy**: Agent Boundaries is most similar to the OpenAPPA section of
  [tsk-market-position-analysis.md](tsk-market-position-analysis.md). The two were not
  compared line by line.

## Not checked

- Whether Coder Tasks has a record that persists after a workspace is deleted, which would
  compare with thread continuation.
- The licence terms of Premium, and what the AGPL v3.0 requires of a deployment.
- Whether a workspace can read from and push to a private Git repository, such as the
  nexus.
