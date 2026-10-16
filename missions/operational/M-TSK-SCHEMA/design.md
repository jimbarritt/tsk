# Design: tsk schema

Mission: M-TSK-SCHEMA. Status: draft, worked out with Jim in questions and answers.

## Problem

An agent writes a mission briefing as Markdown and stores it with `tsk ledger doc put`.
tsk stores the text and does not parse it. So tsk cannot answer "which tasks are open",
cannot render a delegation prompt for one task, and cannot check a status value. Two
agents that change one briefing change the same cell, and Dolt reports a conflict even
when they changed different tasks.

## Core principle: data and documents are separate

Every piece of the ledger is one of two kinds, and the kind decides where it is stored
and how it is written.

| Kind | What it is | Where it is stored | How it is written |
|---|---|---|---|
| Data | Anything with a state, an identity or a link: a mission, a task, a decision, an open question, a thread, a binding | Typed rows in the Dolt database on `tsk/ledger` | `tsk` commands, one per record kind. An agent never writes a row any other way |
| Documents | Free text for a reader: intelligence, research, reports, a design such as this one | Markdown files on the branch `tsk/docs` | `tsk ledger doc put`, which writes the file and commits it |

The test for a new piece: if tsk needs to list it, count it, check it, or change its
state, it is data. If only a reader needs it, it is a document. A record may link to a
document by path. A document never holds state that tsk reads.

The reasons:

- tsk can answer questions about data: which tasks are open, which missions are
  blocked, which questions have no answer. It cannot answer them from Markdown.
- Two agents that change different rows do not conflict. Two that change one Markdown
  file do.
- A document is readable by any tool: GitHub, an editor, marq. It needs no tsk to open.
- Markdown for humans is a projection of the data, produced later, never the source.

Stated by Jim, 2026-10-15.

## Decided

- Missions, tasks and decisions are structured records in the Dolt database, written
  only through `tsk` commands. Agreed by Jim, 2026-10-15.
- Intelligence, research, reports and other free text stay documents, written with
  `tsk ledger doc put`. Agreed by Jim, 2026-10-15.
- The index is generated from the mission records, not written by hand. Agreed by Jim,
  2026-10-15.
- Projections to Markdown for humans come later. Jim, 2026-10-13.
- Record fields: as in the table below, with each objective outcome as its own row and
  blocked-by as a list of links that tsk checks. Agreed by Jim, 2026-10-15.
- Documents are plain Markdown files on a second branch, `tsk/docs`, referenced from
  the ledger by path. The rule: structured data goes in Dolt as typed rows; unstructured
  text goes on `tsk/docs` as files. Dolt owns the whole tree of `tsk/ledger`, so a file
  there is dropped at the next `dolt push`. Jim, 2026-10-15; the branch name chosen by
  Jim.
- `tsk ledger push` pushes both branches in one command, documents first, so a record
  never names a document the remote lacks. `tsk ledger fetch` refreshes both.
  Proposed 2026-10-15.
- Status values: `todo`, `in-progress` and `done`, for missions and for tasks alike,
  in a typed column that rejects any other value. Blocked is not stored: tsk derives it
  when a blocked-by link names a mission or task that is not done. An administrative
  mission stays `in-progress`. No `deferred`: a mission set aside is `todo`. Agreed by
  Jim, 2026-10-15.
- The other briefing sections: decisions and open questions are records, because each
  has a state. Purpose, decision authority, constraints and out of scope are text fields
  on the mission, set by a command. Intelligence and reports are documents on
  `tsk/docs`, linked from the mission. Agreed by Jim, 2026-10-15.
- A mission links to a document by a link record, `(mission, path, role)`, with role
  one of `intelligence`, `report` or `design`. One document can serve several missions,
  and tsk checks that each linked path exists on `tsk/docs`. The directory
  `missions/<category>/M-<ID>/` stays the convention for where a mission's documents are
  written. Agreed by Jim, 2026-10-15.
- Commands: one noun per record kind (`mission`, `task`, `decision`, `question`), with
  the verbs `create` or `add`, `set`, `block` and `unblock`, `link`, `answer`, and
  `list`. Every write changes the local database only; `tsk ledger push` publishes, so
  one push carries a batch. Reads print records, one per line, with the fields asked
  for. No `show` command and no Markdown output: rendering is decided with the
  projections, later. Agreed by Jim, 2026-10-15.
- Migration of the existing briefings: an agent enters each open mission through the
  new commands, record by record, and moves its free text to `tsk/docs`. A done
  mission is intelligence: its briefing and reports become documents on `tsk/docs`,
  linked from a bare mission record (ID, name, status `done`). No parser. Agreed by
  Jim, 2026-10-15.
- There is no current mission. A mission is tied to a thread: a session's mission is
  the one its thread is bound to, and the ledger holds no mark for what to work next.
  Jim, 2026-10-15.
- The index is a query, not a document. `tsk mission list` prints every mission with
  its ID, name, status, derived blocked state and parent. `index.md` goes. The standing
  notes it held (no plan file, administrative missions) become a document on `tsk/docs`
  named in the session start context. Proposed 2026-10-15.
- A thread links to a mission by ID, and a pause entry links to a mission and a task
  by ID. tsk checks that each exists, and refuses an ID it cannot find. The briefing
  path a thread holds today maps to the mission ID in the migration.
  `tsk thread start M-X` takes the ID alone. Agreed by Jim, 2026-10-15.
- The session start context prints the bound thread's mission and its open tasks from
  the records, in place of "read index.md". An unbound session gets `tsk mission list`
  printed instead. Proposed 2026-10-15.
- A custom ref is the default for a repo's ledger and documents. Branches under
  `refs/heads/*` are an override, configured for a repo that is used from Claude Code
  cloud sessions, the only place the proxy blocks a custom ref. Decided by Jim,
  2026-10-15. This supersedes the same day's earlier decision to keep the ledger on
  branches, made because the "Compare & pull request" banner is too noisy for a real
  product with real users.
- Where the ledger setting lives (answer to question 9). A session finds a repo's
  ledger settings in this order:
  1. The nexus, which is part of the environment. The repo's nexus entry holds the
     setting, including whether the repo uses a custom ref or the branch override.
  2. A local repo config, only when the nexus has no entry for the repo.
  3. Neither exists: the session asks the person and discusses the setup. The choices
     are to write an override in this repo, or to register the repo with the nexus.
     When no nexus exists, the session offers to set one up, or defaults to this repo.

  The aim is non-invasive use: a repo registered in a nexus holds nothing of tsk's.
  The local repo config is the uncommon path. It serves a standalone repo with no
  nexus entry that is used from cloud sessions. Decided by Jim, 2026-10-16.

## Custom refs by default, branches for cloud sessions

The branches show on GitHub with a "Compare & pull request" banner that every push
renews. GitHub has no setting to hide it for a branch. A ref outside `refs/heads/*`
would hide the data, and two routes to one were tested from a Claude cloud session on
2026-10-15:

| Route | Result |
|---|---|
| `git push origin HEAD:refs/tsk/<name>` | Cut off by the proxy; the ref does not exist afterwards (as ADR 0008 found) |
| `POST /repos/{owner}/{repo}/git/refs` through `gh` | Refused: "Write access to this GitHub API path is not permitted through this proxy" |
| A remote `dolt sql-server` or DoltHub | Blocked: raw TCP and gRPC are not supported through the proxy |

So a Claude Code cloud session can use only branches. Every other client can use a
custom ref, which GitHub does not list as a branch and does not offer a pull request
for. The override is therefore a property of the repo, set where a cloud session is
wanted.

Options not taken: a nexus repo per user by default (Jim: no), and a GitHub Actions
workflow in each repo that moves the branch to a custom ref after each push. A feature
request to Anthropic for a configurable ref namespace is for Jim to file on
`anthropics/claude-code`.


## Records

Fields agreed by Jim on 2026-10-15. Objective outcomes are rows of their own. Blocked-by
is a list of links, and tsk checks that each named ID exists.

| Record | Fields |
|---|---|
| Mission | ID, name, category (operational or administrative), status, blocked by, objective, purpose |
| Task | ID, mission, description, status, blocked by |
| Decision | text, who, when, mission, task (optional) |

## Commands

```
tsk mission create <id> <name> [--category administrative]
tsk mission set <id> <field> <value>       # status, purpose, constraints, out-of-scope, decision-authority
tsk mission block <id> <by-id>             # and unblock
tsk mission objective add <id> <outcome>   # and done, remove
tsk mission link <id> <path> --role <intelligence|report|design>
tsk mission list

tsk task add <mission> <description>       # mints T-nn, prints it
tsk task set <mission> <task> status <value>
tsk task block <mission> <task> <by>       # and unblock
tsk task list <mission>

tsk decision add <mission> "<text>" --by <who> [--task T-nn]
tsk decision list <mission>

tsk question add <mission> "<text>"        # mints Q-nn
tsk question answer <mission> <q> "<text>" --by <who>
tsk question list [<mission>]              # open questions, across missions by default
```

Each `list` prints one record per line. The exact columns are settled when the commands
are built.

## Open questions

Each question is answered in its own entry below, with the date.

1. Answered 2026-10-15, see Decisions: fields.
2. Answered 2026-10-15, see Decisions: status values.
3. Answered 2026-10-15, see Decisions: the other briefing sections.
4. Answered 2026-10-15, see Decisions: a link record.
5. Answered 2026-10-15, see Decisions: commands.
6. Answered 2026-10-15, see Decisions: migration.
7. Answered 2026-10-15, see Decisions: the index is `tsk mission list`, there is no
   current mission, and the session start context prints the records.
8. Answered 2026-10-15, see Decisions: threads and pause entries link by ID, checked.
9. Answered 2026-10-16, see Decisions: where the ledger setting lives.

## Answers

Answers are recorded under Decisions, with the date.
