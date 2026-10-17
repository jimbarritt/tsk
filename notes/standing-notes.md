# Standing notes

These notes held the standing part of `index.md`, which is retired. The mission records
replaced the rest of it. Read the missions with `tsk mission list`, and one mission in full
with `tsk mission list <id> --json`.

## There is no plan file

There is no `plan.md`. "Plan" is a domain term for the execution of a single mission. It is
not the name of a file that holds the state of all missions. See
`docs/domain/ubiquitous-language.md` in the tsk repo. To ask where things stand, ask for
mission status at the level of scale you mean.

## There is no current mission

The index named a current mission. That is gone. A mission is tied to a thread: the mission
of a session is the one its thread is bound to, and the ledger holds no mark for what to
work next. `tsk mission list` is the index.

## Administrative missions

Administrative missions keep the work itself in order. They do not build the artefacts.
They are kept in `missions/administrative/`, they do not sequence, and nothing blocks them.
A mission with no end point has no `done`, so it stays `in-progress`.

Examples that motivated the category: a session kept open to capture ideas, a tidy up
thread looking for work that needs a nudge, a reviewer watching missions for drift, and a
mission distilling trends out of mission reports. Three exist as missions: M-ADMIN-01 (idea
capture), M-ADMIN-02 (ad hoc tasks) and M-ADMIN-03 (dependency vulnerability watch).
Rationale: `docs/domain/mission-model.md` in the tsk repo, under mission categories.

## A thread without a mission

Superseded. The index said that a thread with no mission is always an administrative
mission, settled on 2026-09-17. The decision of Jim on 2026-10-16 replaces it: a thread can
exist without a mission. A thread is the unit of continuation, and a mission owns the tasks,
decisions and questions. The four cases are:

- `skip`: no thread, nothing persists.
- A thread without a mission: continuity, and no tasks.
- A thread on an administrative mission: continuity and ad hoc tasks.
- A thread on an operational mission: the full model.

Ad hoc tasks go on an administrative mission such as M-ADMIN-02.

## The essential mission

The essential mission is M-BOOT-07. Its objective and the objective of M-BOOT are the same
state: no bootstrap scaffolding remains. Briefings also mark an essential task. The mission
records do not carry that mark, so read it in the linked briefing.

## Ideas

Ideas that are not shaped into missions are kept in `ideas/future-missions-tbd.md`, linked
to M-ADMIN-01. Nothing there is committed to.

## The briefings

The briefing documents under `missions/` are a snapshot as of the migration of the ledger to
records. The records are the truth from then on. Do not repair a briefing to match a record.

## Domain design reference

Distilled design decisions for the domain model of tsk live in `docs/domain/` in the tsk
repo: `ubiquitous-language.md`, `domain-model-overview.md`, `mission-model.md`,
`bootstrap-rationale.md`, `territory-and-nexus.md`, `persistence-and-sync.md` and
`mission-briefing-template.md`.

## Legacy backlog

The checkpoint of 2026-09-14 and the pre-bootstrap backlog that `index.md` carried are kept
verbatim in `notes/legacy-backlog-2026-09.md`.
