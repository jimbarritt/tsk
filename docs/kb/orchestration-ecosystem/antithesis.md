# Antithesis

Status: commercial product, read 2026-10-05. The documentation names no price. Access
starts with a request for a container registry and credentials.

Raised by an inbound message to Jim from Antithesis on 2026-10-05. The message names
two pages, and says the parallels "around transaction correctness and release
confidence" with Formance's ledger may be relevant to tsk.

Sources: the documentation page
[How Antithesis works](https://antithesis.com/docs/introduction/how_antithesis_works/),
and the blog post
[How Antithesis lets Clément Salaün of Formance sleep soundly at night](https://antithesis.com/blog/2025/formance/),
dated 2025-05-01. For Jepsen, the [jepsen.io](https://jepsen.io/) home and
[services](https://jepsen.io/services) pages, and the
[README](https://github.com/jepsen-io/jepsen/blob/main/README.md) and
[What's Here](https://github.com/jepsen-io/jepsen/blob/main/doc/whats-here.md) documents
in `jepsen-io/jepsen`. Every page was fetched as raw HTML or Markdown and read as text.
Quoted phrases were matched against that text. Claims are each vendor's own and were not
run independently. Jev facts come from
[typesafe-jev-classifier.md](../typesafe-jev-classifier.md), whose sources are search
result snippets and one LangChain pull request, not TypeSafe AI's own pages.

## What it is

A testing platform for stateful software. Antithesis treats a system as "a giant state
machine" with "some rare states that we don't want to happen", and testing as "exploring
a very large state space". An integration test "threads a single path through this
immense space".

## Mechanics

| Aspect | Behaviour |
|---|---|
| Environment | "everything is running in our fault-filled environment", including the software's dependencies, its client and its checkers |
| Inputs | Random inputs, "as though we're fuzzing" |
| Faults | "a wide range of faults, like network partitions or node kills" |
| Guidance | "an intelligent guidance component (it uses RL, but you can tell your boss it's AI)" steers the run towards new states |
| Multiverse | Each event can start a new timeline. One run produces "tens (or hundreds) of thousands of alternate universes". The tree of timelines from one run is that run's multiverse |
| Bug detection | Property-based testing. The customer states how the system ought to behave, for example "this application should always recover after a single node dies", as assertions in a test harness. Antithesis "scans the multiverse" for a timeline that breaks one |
| Reproduction | "The Antithesis environment is fully deterministic", so every bug found is reproducible |
| Agent support | "an agent skill that analyzes your system and generates a basic property catalog based on your system architecture", plus guides to properties for system shapes such as blockchains and key-value datastores |

## The deterministic environment

Sources: the documentation page
[The Antithesis environment](https://antithesis.com/docs/environment/the_antithesis_environment/),
and the blog post
[So you think you want to write a deterministic hypervisor?](https://antithesis.com/blog/deterministic_hypervisor/)
by Alex Pshenichkin, dated 2024-03-20.

The customer's containers run in one virtual machine under Antithesis's own hypervisor,
"the Determinator". It is a fork of FreeBSD's bhyve hypervisor, with much of bhyve's
standard function removed. "The unit of reproducibility is the state of the entire
system/experiment/workload as an interconnected whole, not any single process or
server within the system."

| Aspect | Mechanism |
|---|---|
| Definition used | Given an input, the same output, "with the underlying machine always passing through the same sequence of states" |
| Time | Every read of a time source inside the guest, such as TSC or HPET, returns a virtual time the hypervisor computes. Guest clock values are "a function of only the deterministic state and execution history of the guest system" |
| Clock source | Intel's Performance Monitoring Counters, instructions retired. Antithesis measured about one miscount per trillion instructions, and an interrupt that arrives dozens of instructions late, and built workarounds for both |
| Parallelism | Each hypervisor instance runs on one physical core. A 48 or 96 core machine runs that many VMs, each exploring a different part of the state space |
| Concurrency inside | The guest operating system schedules processes, so the software sees concurrency. Antithesis controls that scheduler and uses it to inject faults, such as thread starvation |
| Input and output | A custom use of the `VMCALL` instruction. The guest sends out data such as logs, and takes in commands and random seeds. Each point where the guest takes input is a possible branch, so the exploration forms an input tree. Interrupt injection was added later, to push an input at a chosen time |
| Exploration | The guest sees one linear history. Outside it, Antithesis sees every path visited, and picks new inputs or returns to earlier ones. It does not replay each path from the start. The post leaves out how |
| CPU | A simulated x86-64 Intel CPU with most Skylake extensions. The default clock speed is modulated, or "strobed", as a fault |
| Idle time | The simulation fast-forwards through idle periods. Code that sleeps runs faster than code that busy-waits |
| Kernel | Mostly a Linux 6.x kernel with `io_uring`. A customer can bring their own kernel |
| Memory | 10 GB, shared across the containers |
| Network | No connection to any computer outside the simulation. Containers reach each other by the names in `docker-compose.yaml` |
| Randomness | `/dev/random` and `/dev/urandom` are replaced with devices whose entropy comes from Antithesis |
| External services | AWS services and other third-party infrastructure "Need to be emulated with a stub or mock". Antithesis supplies mocks for many AWS services |
| Output | Standard output and standard error of each container's first process are captured. Files written under `$ANTITHESIS_OUTPUT_DIR` are captured too, and `.jsonl` files are parsed into structured events |
| Detection | `ANTITHESIS_OUTPUT_DIR` is always set inside Antithesis, so software can check for it |

Determinism also makes destructive analysis safe. A failing state can be dumped,
changed and rerun, because it can always be reproduced again.

## The Formance case

Formance is an EU-based provider of open-source financial infrastructure. It has five
core services: Ledger, Connectivity, Flows, Wallets and Reconciliation. The ledger holds
accounts and balances, and records transactions. Transaction IDs are integers that
"increase monotonically, with no gaps in the sequence".

1. A customer's cloud environment showed a gap: 17, 18, 19, 20, then 24. Formance's test
   suite could not reproduce it.
2. Three engineers traced it over more than a week to the way that customer combined a
   dry run parameter with the commit flow.
3. Formance used Antithesis already, but not on this part of the code. After the fix it
   added "a shim specifically targeting the transaction ID sequence" with assertions that
   the sequence was consistent.
4. The first run found the same gap again, from an unrelated cause. A batching queue at
   the storage layer could die, and the main process "would fail to see that the
   batching queue had died and would continue to hand out IDs without resetting the
   sequence".
5. The fix took one day, including a retest in Antithesis.

On release confidence, Salaün says Antithesis "lets us prove to ourselves that something
has been mitigated and will not show up anymore".

## Compared with Jepsen and Jev

Jepsen is a Clojure library for testing distributed systems, by Kyle Kingsbury (aphyr),
and the name of his company, Jepsen LLC. "A test is a Clojure program which uses the
Jepsen library to set up a distributed system, run a bunch of operations against that
system, and verify that the history of those operations makes sense." The two are
connected: Kingsbury gave a retrospective talk on testing distributed systems at the
Antithesis Resilience Meetup in New York in May 2026, listed on jepsen.io on 2026-09-17.

Jev is TypeSafe AI's classifier. It answers a typed question about a given state.

| Aspect | Antithesis | Jepsen | Jev |
|---|---|---|---|
| What it is | A commercial testing platform | An open-source Clojure library, plus paid analyses, training and consulting | A hosted model with one API endpoint, in early access |
| What it tests | A whole system, with its dependencies, client and checkers | A distributed system installed on db nodes | Not a test harness. It judges one state |
| Where it runs | Inside Antithesis's deterministic environment. The customer supplies containers | On db nodes over SSH from a control node: EC2 VMs, LXC containers, or real hardware | TypeSafe AI's API, `POST /v1/systemone` |
| Operations | Random inputs, and a guidance component trained with RL that steers towards new states | A *generator* gives operations to logically single-threaded *processes*, each with a *client* | None. The caller sends the state |
| Faults | Network partitions, node kills and others, injected across the whole environment | A *nemesis* process injects faults, also scheduled by the generator. Clock skew needs separate VMs | None |
| How a check is stated | An assertion in the test harness | A *checker* over the recorded history of operations. Elle checks transactional safety | A question against a fixed schema: `Choice`, `Score` or `Noul` |
| Result | A property broken in any timeline of the multiverse | A report, graphs and the history, under `store/<test-name>/<date>/` | A typed answer with probabilities and a confidence value |
| Reproduction | Deterministic replay of a failing timeline | The README describes no replay of a failing run | Not applicable |
| Who writes the checks | The customer. An agent skill writes a starting property catalogue | The test author, in Clojure | The caller, as questions |

Antithesis and Jepsen do the same kind of job: generate operations, inject faults, then
check a stated property against what happened. They differ in where the run happens.
Jepsen drives real nodes. Antithesis runs everything in one deterministic environment,
so a failing run replays exactly. Jev does a different job. It returns a probability
that a condition holds on one state, where Antithesis and Jepsen return a true or false
result from a check written in code.

## Comparison with tsk

| tsk term | Antithesis |
|---|---|
| Ledger | The word only. tsk's ledger is a git branch holding missions, threads and continuation state. Formance's ledger holds accounts, balances and financial transactions. |
| Objective | A tsk objective is a fixed end point or a measure over time, stated in a mission briefing. An Antithesis property is an assertion checked across every timeline of a run. |
| Concurrent writers | Several actors write the tsk ledger. `tsk ledger push` fetches `tsk/ledger` and builds on its latest state before it pushes. Two `SessionStart` hooks can run `tsk thread session-start` for one event, and the binary claims the event with an exclusive file create. These are interleavings of the kind Antithesis injects faults into. |
| Reproduction | tsk's tests run each interleaving the test author writes. Antithesis generates interleavings and faults, and replays a failing one deterministically. |
| Property catalogue | The agent skill writes a starting set of properties from a system's architecture. tsk has no equivalent artefact. |

## Not established

- Whether a Jepsen run is deterministic or can be replayed. None of the pages read
  states either.
- Price, and whether a single developer or open-source project can use it.
- Whether Antithesis supplies a mock for a model API. The environment has no outside
  network, and third-party services need a stub or mock, so a system that calls a model
  API runs only against one.
- Whether the Formance work is current. The inbound message says "currently working
  with Formance". The post is dated 2025-05-01.
