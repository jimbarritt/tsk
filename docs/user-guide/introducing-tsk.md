# Introducing tsk

tsk is a task and mission tracking system for software delivery work, built
for use by both people and agents.

## The problem with ticket systems

Ticket systems such as Jira share one problem: the ticket is their only
domain concept, and it comes from a support-system heritage. Stories and
epics do not change that heritage. Human organisations build labels,
milestones, projects, and initiatives on top of the ticket to approximate
the concepts their own work needs.

Existing tools represent only a temporal dimension: tickets move through
states over time, and that is the only axis in the model. None of them
represents the product itself. There is no feature catalogue or product map
built into the engine, queryable, referenced by a ticket. An agent asked
"what feature does this ticket change" has no model to answer from. A
product map built separately, in a document or a wiki page, sits outside
the ticket engine and goes out of date.

## Two domain concepts: product and navigation

Software delivery work has two distinct facets that a ticket system
conflates. The product is a static thing: a snapshot at a point in time. A
change to it is a delta.

Navigation is the process of moving from one state of the product to
another. Projects, initiatives, and roadmaps describe navigation.

tsk defines product, delta, and navigation as connected concepts in one
model, along with decisions such as architecture decision records. The
model gives both people and agents a single structure to query and update,
instead of one ticket primitive stretched to cover every kind of work.

## Sessions, context, and continuation

An agent session runs for a bounded amount of context. When context runs
out, the session stops or auto-compacts, and any state not yet recorded is
lost. tsk gives a session a way to record state before it ends, and gives
the next session a way to pick up the same work.

That state is the domain model itself: products, navigation, and the
connections between them. A repository, at session start, reads the ledger
which holds the persistent record of ongoing work.

A new session starts with no state from prior sessions. On startup, a
session checks whether it is registered to a thread. A thread tracks the
state of one piece of work. Once a session registers a thread, any later
session that resumes the thread reads that state and continues the same
work.

With this structure in place, an agent triggers further work without an
explicit instruction for it: for example, spawning a further session to
implement a requested feature, or verifying its own change once made.

## Threads, tasks, and missions

tsk gives an agent instructions the way a military order gives an
objective rather than a fixed procedure, because a fixed procedure cannot
anticipate every circumstance encountered while carrying it out.

Instructions fall on a scale by size:

- An ad hoc instruction gets carried out directly.
- A **task** sits in the ledger under a thread.
- A **mission** holds a briefing: an objective the agent breaks into its
  own tasks and implementation, any constraints that apply, and, on
  completion, a mission report describing the outcome, written back into
  the ledger.

A mission briefing states the outcome required, not the method: the
receiving agent works out the mechanism. A mission briefing also states
intel: standing references such as where architecture decisions or other
documentation live, and any references specific to that piece of work.

## Ubiquitous language

Every domain concept has one name, recorded in a ubiquitous language file.
A new term goes into the file when it appears. A name that does not match
the file gets corrected to the term the file defines.

## Current status

tsk currently runs as files and scripts in a repository, using a Git-based
state branch. A planned rewrite in Rust turns this into a deterministic
program, exposed through a CLI and an MCP server, so that a caller creates
a mission or a task through `tsk` without depending on how the state is
stored.
