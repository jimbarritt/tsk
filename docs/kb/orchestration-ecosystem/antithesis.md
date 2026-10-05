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
dated 2025-05-01. Both pages were fetched as raw HTML and read as text. Quoted phrases
were matched against that text. Claims are Antithesis's and Formance's own and were not
run independently.

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

## Comparison with tsk

| tsk term | Antithesis |
|---|---|
| Ledger | The word only. tsk's ledger is a git branch holding missions, threads and continuation state. Formance's ledger holds accounts, balances and financial transactions. |
| Objective | A tsk objective is a fixed end point or a measure over time, stated in a mission briefing. An Antithesis property is an assertion checked across every timeline of a run. |
| Concurrent writers | Several actors write the tsk ledger. `tsk ledger push` fetches `tsk/ledger` and builds on its latest state before it pushes. Two `SessionStart` hooks can run `tsk thread session-start` for one event, and the binary claims the event with an exclusive file create. These are interleavings of the kind Antithesis injects faults into. |
| Reproduction | tsk's tests run each interleaving the test author writes. Antithesis generates interleavings and faults, and replays a failing one deterministically. |
| Property catalogue | The agent skill writes a starting set of properties from a system's architecture. tsk has no equivalent artefact. |

## Not established

- Price, and whether a single developer or open-source project can use it.
- How a system that calls an external service, such as a model API, runs inside the
  deterministic environment.
- Whether the Formance work is current. The inbound message says "currently working
  with Formance". The post is dated 2025-05-01.
