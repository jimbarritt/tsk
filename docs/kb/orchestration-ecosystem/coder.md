# Coder

Coder is a self-hosted platform that runs AI coding agents in Terraform-defined workspaces. Its core is open source. Its governance features for model access and agent policy need a paid Premium licence.

Status: core under the AGPL v3.0, with a Premium licence for enhanced support, custom deployments and AI governance. Captured 2026-11-19 from a summary that Jim pasted. Checked the same day against the Coder pages listed under Sources. Claims are Coder's own and were not run here.

Sources, read on 2026-11-19:

- [Quickstart](https://coder.com/docs/tutorials/quickstart), [Docker install](https://coder.com/docs/install/docker) and [validated architectures](https://coder.com/docs/admin/infrastructure/validated-architectures).
- The blog post [AI development infrastructure for hybrid human and agent teams](https://coder.com/blog/ai-development-infrastructure-for-hybrid-human-and-agent-teams), 9 December 2025.
- The blog post [Secure agentic AI, now production ready](https://coder.com/blog/secure-agentic-ai-now-production-ready), 3 February 2026.
- The [coder/coder repository](https://github.com/coder/coder).

The summary also cites InfoQ (May 2026) and Contrary Research. Those two were not read.

## What Coder is

Coder is a self-hosted platform for running AI coding agents and cloud development environments on infrastructure that a team controls. It works with any cloud, IDE, operating system, Git provider and identity provider.

## How Coder works

- A workspace is defined in Terraform. It connects through a WireGuard tunnel and shuts down when idle.
- Agents and developers use the same workspace infrastructure.
- Coder Agents is a native coding agent. Its loop runs in the control plane on the self-hosted infrastructure, not in the workspace and not in a vendor cloud. A workspace has no LLM API keys.
- It accepts any model: Anthropic, OpenAI, Google, Bedrock or a self-hosted model.
- Third-party agents, such as Claude Code and Codex, also run in workspaces.
- A person uses a web UI. A script or CI calls a REST API.

## Governance features

Announced 9 December 2025.

| Feature | What the sources state it does |
|---|---|
| Coder Tasks | An execution engine for long-lived, low-interaction jobs: code review, docs, issue solving and test authoring. It has a full API and notifications. |
| AI Bridge | Central access, authentication and observability for all model providers. It consolidates prompt logs, usage patterns and token consumption. |
| Agent Boundaries | Policy controls that work as a firewall for an agent. Allow lists cover network destinations, permitted tools and internal systems. |

The blog post of 3 February 2026 states that, from Coder 2.30, AI Bridge and Agent Boundaries belong to the AI Governance Add-On for Premium customers. A later update on that page states that AI Governance is included with every newly purchased Premium licence. The page also states that the two features were renamed in April 2026: AI Bridge is AI Gateway, and Agent Boundaries is Agent Firewall.

Neither post states the licence of Coder Tasks. The February post mentions an additional allowance of Agent Workspace Builds for Coder Tasks, which suggests a paid allowance. That is unconfirmed.

## Licence

The repository is AGPL-3.0. It also has a `LICENSE.enterprise` file and an `enterprise` directory. The repository page does not state the terms of that directory.

## Install

```
curl -L https://coder.com/install.sh | sh
coder server
```

- **Single machine**: the quickstart requires 2 or more CPU cores and 4 GB or more of RAM. The Docker install requires at least 2 CPU cores and 4 GB of free memory.
- **Database**: `coder server` with no flags uses a built-in database, which the documentation calls proof of concept. Production needs PostgreSQL 13 or later.
- **Docker**: a documented Linux install, either `docker run` with the Docker socket mounted or a `compose.yaml` that includes PostgreSQL.
- **Access URL**: the server needs a `CODER_ACCESS_URL` that clients can open. Coder's own tunnel is for proof of concept.
- **Sizing for agents**: the pages give no CPU, memory or disk figures for the server of a small install, and none for a workspace that runs an agent. The validated architectures start at 1,000 users.
- **ARM**: no page read mentions arm64. Support is unconfirmed.

## Suitability of Coder

- It suits teams that need audit, identity and network control over agents: platform teams and regulated enterprises.
- A solo setup is a weaker fit than these teams.
- It hosts agents in Terraform-defined workspaces. It is not a general-purpose agent runtime.

## Comparison against tsk

| Question | Coder | tsk |
|---|---|---|
| What is the unit of work? | A workspace that runs an agent. A Coder Task is an execution job. | A mission, with tasks, decisions and questions as ledger records. |
| Where is state about the work stored? | Not described in the sources, apart from prompt logs and usage in AI Bridge. | The ledger, a Dolt database, with thread continuation entries. |
| Which layer? | Where an agent runs, and the policy around it. | What the agent works on, and what a later actor reads to continue. |
| Which agent? | Any: its own, Claude Code, Codex. | Any harness that calls `tsk`. |

- **Task**: a Coder Task is a job to run. A tsk task is a record of work to do. The word "task" names a job in Coder and a record in tsk.
- **Substrate**: Coder is a candidate place to run tsk, as the Dust, Dify, n8n and StackAI cases are in [hosted-agent-platforms-as-tsk-substrates.md](hosted-agent-platforms-as-tsk-substrates.md). It was not tested. A test needs a workspace with `tsk` and `dolt` installed, a nexus attached and a robot signing key set.
- **Policy**: Agent Boundaries is most similar to the OpenAPPA section of [tsk-market-position-analysis.md](tsk-market-position-analysis.md). The two were not compared line by line.

## Not checked

- Whether Coder Tasks persists a record after a workspace is deleted, which would compare with thread continuation.
- Whether Coder Tasks is under the AGPL or needs Premium.
- Whether Coder runs on arm64.
- The memory and CPU that a workspace needs for one Claude Code agent.
- Whether a workspace can read from and push to a private Git repository, such as the nexus.
- The InfoQ and Contrary Research pages that the summary cites.
