# Mission: tsk schema

| Field | Value |
|---|---|
| ID | M-TSK-SCHEMA |
| Territory | agentic research |
| Assignee | unassigned |
| Blocked by | none |

Designed. Named by Jim on 2026-10-15. The design is in
`missions/operational/M-TSK-SCHEMA/design.md`, worked out in eight questions and answers
with Jim the same day. The plan below is drawn from it and awaits Jim's approval.

## Idea, as captured

Raised by Jim, 2026-10-13, when the import guide (M-ENV T-10) was proposed in terms of
document paths:

> Wait. No we should be using dolt now. I thought we agreed everything should be stored
> in dolt.
>
> The whole point is that the agents should not edit the markdowns themselves they
> should always go through tsk. Then we do the projections later

And, on the same day:

> Doc put will still be used for eg storing intel files.

Named by Jim, 2026-10-15: "M-TSK-SCHEMA".

## Intelligence

- `missions/operational/M-TSK-SCHEMA/design.md`: the design document for this mission.
  The design is worked out there.
- M-DOLT's briefing, `missions/operational/M-DOLT-ledger-store.md`: every ledger record
  moved to Dolt as it was. A mission briefing is a row in the `documents` table, with its
  Markdown body in one text column. An agent writes that Markdown and stores it with
  `tsk ledger doc put`, and tsk does not parse it. The briefing lists "Changing what the
  ledger records" as out of scope, and "Projections are decided later" as a decision.
- `future-missions-tbd.md`, "Dolt as the ledger's store, and markdown as a projection for
  humans" (Jim, 2026-10-09): the earlier form of the idea.
- `future-missions-tbd.md`, the subagent delegation pattern: the candidate features,
  including `tsk mission brief <mission> <task>`, which renders a delegation prompt from
  the ledger, decision authority per run, file ownership as data, and structured
  questions from a subagent.
- `docs/adr/0007-event-log-as-source-of-truth.md` in the tsk repo: Markdown as a
  read-only projection of tsk's own store.
- `docs/adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md` in the tsk repo:
  Dolt as the store, and the projection deferred.
- `docs/domain/mission-model.md` and `docs/domain/mission-briefing-template.md` in the
  tsk repo: the mission model and the briefing sections today.

## Decisions

- **Records and documents.** Missions, tasks and decisions become structured records,
  written through `tsk` commands. Intelligence, research, reports and other free text
  stay documents, written with `tsk ledger doc put`. The index is generated from the
  mission records. Agreed by Jim, 2026-10-15.

## Objective

- Missions, tasks, decisions and open questions are typed rows in the Dolt database,
  written and read only through `tsk` commands. Markdown briefings no longer exist for
  open missions.
- Documents are plain Markdown files on the branch `tsk/docs`, written with
  `tsk ledger doc put` and linked from mission records by path and role.
  `tsk ledger push` pushes both branches in one command, documents first.
- A thread links to a mission by ID, and a pause entry to a mission and a task by ID.
  tsk refuses an ID it cannot find.
- `tsk mission list` is the index. The session start context prints the bound thread's
  mission and its open tasks, or the mission list when unbound.
- Every open mission in this repo's ledger is migrated to records through the commands,
  and every done mission's briefing is a document linked from a bare mission record.
- The import guide, the plugin skills, `CLAUDE.md` and the user guide describe the
  commands, and no agent-facing text names a briefing file.
- A repo's ledger and documents use a custom ref by default. Branches under
  `refs/heads/*` are an override, set for a repo that is used from Claude Code cloud
  sessions.

## Decision authority

Jim decided everything in the design doc's Decisions section. The actor decides table
definitions, column types, output columns of `list`, and test layout, and records each
choice in the design doc.

## Constraints

- Every write goes through the `LedgerStore` trait, and the conformance suite runs
  against both backends. The `git` backend may hold records as JSONL files, so the file
  ledger keeps working for a repo that selects it.
- No Markdown rendering of records in this mission. Projections are a later mission.
- The existing `documents` table and `tsk ledger doc` commands keep working until the
  migration moves every document to `tsk/docs`, then the table goes.
- One release per task where a task changes the binary, so each step can be tried in a
  live session before the next.

## Out of scope

- Projections to Markdown for humans.
- Rendering a delegation prompt from the records (`tsk mission brief`).

## Plan

Drafted 2026-10-15 by thread 25366ae1 from the design doc. Jim told the actor to run with T-01 on 2026-10-15 and to report when it is released.

| ID | Task | Status |
|---|---|---|
| T-01 | Documents on `tsk/docs`: a docs worktree beside the Dolt database, `tsk ledger doc` commands read and write it, `tsk ledger fetch` and `push` handle both branches, documents first. Migrate every document out of the `documents` table, then drop it. Conformance tests for both backends. Release | DONE 2026-10-16. Released as 0.8.0 (`562b604`) on 2026-10-15. The ledger was migrated with `tsk ledger migrate-docs`, run by another session before this one asked Jim's go-ahead: the Dolt history shows the commit "Move the documents to the tsk/docs branch", then a pause for thread uu44oy4p, the M-ADMIN-03 daily check. Verified by the actor: all 53 old rows are byte-identical to the files at the docs branch's migration commit, and the manifest is version 2. `tsk self-update` upgraded a released 0.7.0 binary to 0.8.0 against the live releases |
| T-02 | Mission records: the `missions`, `objective_outcomes`, `mission_blocks` and `mission_documents` tables, `tsk mission create`, `set`, `block`, `unblock`, `objective`, `link` and `list`, with derived blocked state. Release | DONE 2026-10-16. Released as 0.9.0 (`588375f`, includes `2d6fd93`). The ledger manifest is version 3; a binary older than 0.9.0 stops with the unsupported version error. Tables: `missions` (id, name, category, status, parent, purpose, decision_authority, constraints_text, out_of_scope, next_objective), `objective_outcomes`, `mission_blocks`, `mission_documents` (role intelligence, report or design). Commands: `create`, `set`, `block`, `unblock`, `objective`, `link`, `unlink`, `list`. `list` prints one line per mission: id, status, blocked-by, parent, name. There is no `show`, by decision. Additions beyond the plan: a `--parent` option on `create`, a `next_objective` column, an error when `block` repeats a pair, `link` of an existing path changes its role, and a derived `blocked` boolean in JSON. The files backend stores `records/missions/<ID>.json`, and `records/` is a reserved top-level name. Verified: full suite green, and the v2 to v3 upgrade ran on a scratch copy of this repo's real ledger with documents unchanged |
| T-03 | Task, decision and question records: their tables and commands, with `T-nn` and `Q-nn` minting per mission. Release | DONE 2026-10-16. Released as 0.10.0 (`369bfe6`). Manifest version 4. New columns `missions.next_task` and `next_question`; tables `tasks`, `task_blocks`, `decisions`, `questions`. Commands: `tsk task add|set|block|unblock|list`, `tsk decision add|list`, `tsk question add|answer|list`. Lists are tab separated. `task list` prints id, status, open blockers, description. `decision list` prints date, who, task, text. `question list` prints mission, id, asked date, text, open questions only. Records also appear in `mission list --json`. Behaviour to know: any mission write on a version 3 ledger upgrades it to 4; `question answer` on an answered question is refused; a task or decision added to one mission by two clones at once conflicts on the primary key at push. Verified: full suite green; upgrade run on scratch copies of the real ledger from version 2 and from version 3 |
| T-04 | Threads and pause entries link by ID: `tsk thread start <mission-id>`, `pause` takes a task ID and checks it, the session start context prints the mission and open tasks or the mission list. The thread tables gain the links. From issue #4: the session start context names `tsk ledger migrate-docs` when the ledger needs it; the Stop hook check exits 0 when the ledger holds no missions; `tsk thread skip` records that this session works without a thread, and the Stop hook check exits 0 for that session. Release | DONE 2026-10-16. Released as 0.11.0 (`ea341c8`, `7d32cc1`). Manifest version 5: `threads.mission_id` added, `briefing_path` nullable; a legacy thread's mission ID is derived from its briefing file name at read time. `start` accepts a mission with a record or with a briefing document; the old two-argument `start` and four-argument `pause` forms still work, with a note on stderr. Skip state is a local file in the git directory (`tsk-thread-skip`, or `tsk-thread-skip-<session-id>` in a cloud session), never pushed; `start` and `resume` clear it. A defect found in verification and fixed: thread commands refused after an unpushed record write; they now keep local changes and push them with their own change, and session start notes unpushed local changes. The plugin requires tsk 0.11.0 (plugin 0.3.7). Verified: full suite green; checks on scratch copies of the real ledger |
| T-05 | Migrate this repo's ledger: each open mission entered through the commands, each done mission's briefing moved to `tsk/docs` and linked from a bare record, `index.md` and `future-missions-tbd.md` retired, the standing notes moved to a document | TODO |
| T-06 | Guides and docs: `tsk guide import` rewritten for the commands, the plugin skills, `CLAUDE.md`, `ledger-layout.md`, `session-continuation-design.md` and the user guide updated. From issue #4: the guide states that the records are the source of truth for missions, tasks, decisions and questions, and that a mission is done when its status is set to `done`, with no check of its objectives; the session start context warns when the loaded plugin is older than the plugin version the binary was built with, and names the update command. Release | TODO |
| T-07 | Custom ref by default: a repo's ledger and documents use a custom ref outside `refs/heads/*`, and branches under `refs/heads/*` are an override for a repo used from Claude Code cloud sessions. Where the setting lives is open question 9 in the design doc. This repo keeps the override. Conformance tests for both modes. Release | TODO |
| T-08 | Respond to the feedback in GitHub issue #4 (feedback on 0.10.0 from the dotfiles repo): decide which of its confusions, gaps and suggestions to act on, fold the ones that belong to T-04 to T-06 into those tasks, build the rest as separate changes, and reply on the issue with what was decided | DONE 2026-10-16. Decided by the actor, on Jim's instruction to decide. Folded into T-04: the migrate-docs prompt, the Stop hook with an empty ledger, the session opt-out, `thread start` by mission ID. Folded into T-06: the guide for the record model, the rule for done, the plugin version warning. New task T-09: `tsk ledger status`, `tsk ledger discard`, `decision add --on <date>`, a plain error outside a git repository. Not taken now: `rm` commands, because `discard` covers an unpushed mistake and `set` corrects a pushed one, and deletion needs reference checks for little gain; a batch import, because T-05 measures the import time on this repo first. `migrate-docs` is offered, not run, because it rewrites shared data. Reply posted on issue #4 |
| T-09 | Ledger status and discard, from issue #4: `tsk ledger status` prints the local changes not yet pushed and whether origin is ahead or behind, for the data and the documents; `tsk ledger discard` drops every local change not yet pushed, in the data and the documents; `tsk decision add --on <YYYY-MM-DD>`; run outside a git repository, every command stops with one plain line that says to run tsk inside the repository. Release | DONE 2026-10-16. Released as 0.12.0 (`f26f79d`, `89356a7`). `tsk ledger status` prints `data` and `docs` lines (clean, or local changes with one line per changed table or file, a table with a schema-only change marked `schema changed`) and two `remote` lines (`up to date`, `ahead N`, `behind N`, `diverged ahead N behind N`, `unpublished` or `unknown`); it works offline. `tsk ledger discard` lists what it would drop and exits 1 without `--yes`, and with `--yes` resets the data and the documents to origin and prints `discarded`; thread state is left alone. `tsk decision add --on <YYYY-MM-DD>` checks the date is real. Outside a git repository, ledger, mission, task, decision, question and thread commands print one line, `error: not inside a git repository. Run tsk from inside the repository whose ledger you want.`; the two hook commands still exit 0 with JSON. The hints that told a person to run `dolt reset --hard` now name `tsk ledger status` and `tsk ledger discard`. Verified: full suite green; the commands run on a scratch copy of the real ledger |
| T-10 | A thread without a mission, as decided 2026-10-16 in the design doc: `tsk thread start` with no argument mints a thread with no mission; `pause` on it takes an empty task ID and refuses any other; session start prints the latest what's next and says the thread has no mission; `tsk thread set mission <id>` attaches or changes the mission, and each pause entry keeps the mission current when it was written. No schema change: `threads.mission_id` is already nullable. Release | TODO |
| T-11 | Every binding in the ledger: one `bindings` table keyed on the actor, the cloud session ID or the clone ID from `.git/tsk-clone-id` plus the worktree name for a `git worktree` checkout, replaces the `cloud_session_bindings` table and the `tsk-thread-id` file. A local bind, detach or switch writes the ledger and waits for the next push. `tsk thread list` shows the actors bound to each thread. The Stop hook check reads the local store without fetching. Existing file bindings migrate on first use. Release | TODO |
| T-12 | Decisions link to ADRs, optionally: `tsk decision add ... --adr <path>` stores the path of an ADR in the repo, for example `docs/adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md`, checked to exist in the repo when the command runs. `decision list` prints the path in a column. `tsk decision list --adr <path>` finds the decisions that name an ADR. The `decisions` table gains a nullable `adr_path` column. Release | TODO |

Order: T-01, then T-02 to T-04 in order, then T-09, then T-10 to T-12, then T-05, then T-06. T-05 is the
test of T-01 to T-04 and T-09 to T-12 on real data, so T-06 waits for what it finds. T-07 follows
T-01, because it covers the documents ref as well as the Dolt ref.
