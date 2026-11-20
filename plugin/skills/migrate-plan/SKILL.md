---
name: migrate-plan
description: "Import an existing plan into the tsk ledger as missions, tasks, decisions and questions, in any format. Invoke as /tsk:migrate-plan [<path>], or when asked to migrate, import or convert plan, roadmap or todo files into tsk."
---

Backs the `/tsk:migrate-plan` command. The skill works with any plan format. You find the planning
material, read it, propose missions and tasks, show the proposal, and write records with `tsk`
commands only after the person confirms.

`tsk guide import` contains the mapping from plan content to records and the command syntax. Run
it first and follow it. This skill adds the search, the proposal and the checks.

## Rules

- Write the ledger only through `tsk` commands. Do not open, edit or create files in the ledger
  store, and do not run `dolt` or `git` against it.
- Do not read or clone the tsk source code to learn a format. Run `tsk guide import` and
  `tsk <command> --help`.
- Write nothing before the person confirms the proposal. Reading is always allowed.
- Leave the original files where they are. Ask before deleting or relocating a file.
- Do not copy a record into a document. Records store state, and documents store free text.
- Do not assume the shape of the plan. A plan can be a table, a checklist, a prose outline, an
  issue list, a notes file or a mix.

## Steps

1. **Check the ledger.** Run `tsk ledger status`. If the repo has no ledger, stop and run
   `/tsk:init-ledger` first. Run `tsk mission list` to see which missions exist, so the import
   does not duplicate one.

2. **Find the planning material.** Start with the path in the argument, if any. Otherwise look,
   and report where you looked and what you found.
   - In the repo: files and folders named like `plan`, `planning`, `roadmap`, `todo`, `tasks`,
     `backlog`, `notes`, `docs/planning`, `doc/planning`, `.planning`, and the top-level
     Markdown files.
   - In the home directory of the person: `~/.planning/<project>/`, `~/projects/<project>/`, and
     the folder of notes the person names. Look in named folders only. Never run `find` from
     `~` or `/`.
   - In the agent's own files: `CLAUDE.md`, `AGENTS.md`, and the memory folder, for decisions and
     constraints that belong in records.
   - Ask the person where else the plan is kept, and whether any of what you found is not part of
     the plan.

3. **Read it.** Read every file you propose to import, in full. Note each goal or project, each
   step under it, what is done, in progress or blocked, each decision with who made it and when,
   each open question, and each idea not yet planned. A folder of dated files is a history: the
   latest file is the current state, and the older ones are sources of decisions.

4. **Propose.** Map the material with the table in `tsk guide import`, and show the person:
   - the missions, each with a proposed ID, name and one-line purpose;
   - the task count of each mission and its status, and the tasks that are blocked;
   - the decisions and open questions, with a count for each mission;
   - the documents to write, each with its path and linked mission;
   - the material you propose to skip, and why.
   Keep the proposal short enough to read without scrolling. Where the material is large, show
   the missions first and the detail of one mission at a time. Ask about an unclear item one
   question at a time. Do not guess an ID, a status or an owner that the plan does not state.

5. **Wait for confirmation.** The person can change the proposal. Show the changed parts again.
   Write nothing until the person agrees.

6. **Write.** Follow steps 4 to 6 of `tsk guide import`: `tsk ledger fetch`, then the records
   mission by mission, then the documents. Quote every text argument. Set a mission to `done`
   only when the person agrees it is done.

7. **Check.** Run `tsk ledger status` and `tsk mission list`, then `tsk mission list <id> --json`
   for one mission, and compare it with the plan. Fix a mistake with the correcting commands, or
   run `tsk ledger discard --yes` to start again. Tell the person before a discard.

8. **Publish.** Run `tsk ledger push "Import the existing plan"`. Read any line such as
   `T-03 is now T-05 in M-X` and use the new numbers from then on.

9. **Hand back.** Tell the person what you imported and where the original files are. Ask what
   to do with the original files, and whether to start a thread on a mission with
   `/tsk:start-thread`.

## Learned from the field test

On a machine with no guidance, an agent migrated plan files without help. The person judged the result
good and asked for guidance that does not depend on the format of one person's plan files.

- The plan files can be outside the repo. Ask where the plan files are kept, and look in the places in
  step 2.
- Show the mapping before writing, so the person can correct it.
- Settings can be unset at ledger init, such as the push workflow of the repo (`main` or pull
  requests). Run `tsk ledger status` before the import, and ask the person about any setting it
  reports as not set.
- A ledger in a nexus can have a territory with no display name. Ask the person for one. Do not
  use `tsk nexus register-repo --territory-name` for this: it names a territory the command
  creates.
- Custom refs (`refs/tsk/...`) are the default outside a Claude Code cloud session and need no
  change.
- Planning material in a person's own format contains decisions and open questions in prose. Look
  for them in each file, not only in the task lists.
