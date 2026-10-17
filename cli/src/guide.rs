use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum GuideCommands {
    #[command(about = "How to bring an existing plan, in any format, into the tsk ledger")]
    Import,
}

pub fn run(action: GuideCommands) -> Result<(), String> {
    match action {
        GuideCommands::Import => print!("{}", IMPORT),
    }
    Ok(())
}

/// The sentence the session start context adds when the ledger holds no missions.
pub const EMPTY_LEDGER_NOTE: &str = "The ledger holds no missions yet. To bring in an existing plan, run `tsk guide import` and follow it.";

pub const IMPORT: &str = r#"# Importing an existing plan into tsk

This guide is for an agent. It explains how tsk stores work, so you can move a plan in
any format into the tsk ledger. tsk does not parse other plan formats. You read the plan
and write it into the ledger through `tsk` commands.

## Records and documents: read this first

Since tsk 0.9.0 the ledger holds two kinds of thing.

- **Records** are the source of truth for the state of the work: missions, tasks,
  decisions, questions, objectives, blocks, and the links from a mission to documents.
  They are typed rows in the database. Write them only through the `tsk mission`,
  `tsk task`, `tsk decision` and `tsk question` commands.
- **Documents** are free text: intelligence, reports and designs. Write them with
  `tsk ledger doc put`, and link them to a mission with `tsk mission link`.

Do not write the same fact in both places. A briefing document that repeats a mission's
purpose, decisions, questions or plan drifts away from the records, and the records win.

To read a mission in full, run `tsk mission list <id> --json`. It prints the whole
record: purpose, constraints, objectives, blocks, linked documents, tasks, decisions and
questions, open and answered. `tsk mission list` with no ID lists every mission, one per
line. There is no command that prints a mission as Markdown yet. Projections, which
render the records as Markdown for people, are planned. Until they exist, read the JSON
and write the summary or table you need yourself, for the person you are working with.

The rest of this guide describes the document form of a plan, which tsk 0.6.0 used. It is
being rewritten for the record commands. Where it conflicts with this section, follow
this section.

## Rules

- Write the ledger only through `tsk` commands. The ledger is a Dolt database. Do not
  open, edit or create files in the ledger store (`tsk ledger path`), and do not run
  `dolt` or `git` against it.
- Do not read or clone the tsk source code to learn the format. This guide holds what
  you need.
- Leave the original plan files where they are. Ask the person before you delete or move
  them.
- Show the person your mapping (step 3) before you write anything.

## What the ledger holds

The ledger holds documents. A document is a Markdown body stored under a path. The path
is a key in the database, not a file.

| Path | Holds |
|---|---|
| `index.md` | The entry point: the current mission, current state and next steps, and a table of every mission. An agent reads it first in every session. |
| `missions/operational/M-<ID>-<slug>.md` | A mission briefing: a mission that builds something. |
| `missions/administrative/M-<ID>-<slug>.md` | A mission that keeps the work in order and has no end point, such as idea capture. |
| `missions/<category>/M-<ID>/...` | A mission's supporting documents: research, intelligence, reports. |
| `future-missions-tbd.md` | Ideas not yet shaped into missions. |

A mission ID is `M-` and a short upper-case name, with a number for a sub-mission, for
example `M-SETUP` or `M-SETUP-02`. The slug is the mission name in lower-case words
joined by `-`.

Threads, continuation entries and session bindings are also in the ledger. `tsk thread`
commands write them. An import does not create threads.

## A mission briefing

Write each briefing in this shape. Leave out a section that has nothing in it.

```markdown
# Mission: <name>

| Field | Value |
|---|---|
| ID | M-<ID> |
| Status | TODO, IN PROGRESS or DONE |
| Blocked by | none, or the IDs of other missions |

## Objective

The state of the world when the mission is done, as a short list of outcomes.

## Purpose

Why the mission exists.

## Decisions

Each decision already made, with who made it and when.

## Open questions

Each question not yet answered.

## Plan

| ID | Task | Status |
|---|---|---|
| T-01 | What the task does | TODO |
```

## Commands

```bash
tsk ledger fetch                      # refresh the ledger store from the remote
tsk ledger doc list [<prefix>]        # list document paths
tsk ledger doc get <path>             # print a document
tsk ledger doc put <path> [<file>]    # write a document from a file, or from stdin
tsk ledger doc rm <path>              # remove a document
tsk ledger push "<message>"           # commit and push every change
```

`doc put` and `doc rm` change the local store only. `tsk ledger push` publishes them.
Write a document from stdin with a here-document:

```bash
tsk ledger doc put missions/operational/M-SETUP-machine-setup.md <<'EOF'
# Mission: Machine setup
...
EOF
```

## Steps

1. Find the existing plan. Look for plan, roadmap, todo or task files in the repo and in
   any directory the person names, and ask the person where else their plan is kept.
2. Read it. Note each goal or project, each step under it, what is done, what is in
   progress, what is blocked, and each idea not yet planned.
3. Map it, and show the mapping to the person before you write:
   - each goal or project becomes a mission briefing;
   - its steps become rows in the briefing's Plan table, with their status;
   - decisions and open questions go to those sections of the briefing;
   - notes, research and reference material become supporting documents under the
     mission's directory, `missions/<category>/M-<ID>/`;
   - ideas not yet planned go to `future-missions-tbd.md`;
   - the overall state goes to `index.md`.
4. Run `tsk ledger fetch`, then write each document with `tsk ledger doc put`.
5. Write `index.md` last: the current mission, current state, next steps, and a table
   of every mission with its ID, name, status and briefing path.
6. Run `tsk ledger push "Import the existing plan"`.
7. Check the result with `tsk ledger doc list` and `tsk ledger doc get index.md`.
8. Tell the person what you imported and where the original files are. Ask whether to
   delete them, and whether to start a thread on a mission with `/tsk:start-thread`.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_import_guide_names_the_write_commands_and_the_rules() {
        assert!(IMPORT.contains("tsk ledger doc put"));
        assert!(IMPORT.contains("tsk ledger push"));
        assert!(IMPORT.contains("Do not read or clone the tsk source code"));
        assert!(IMPORT.ends_with('\n'));
    }

    #[test]
    fn the_import_guide_says_records_are_the_truth_and_how_to_read_a_mission() {
        assert!(IMPORT.contains("source of truth"));
        assert!(IMPORT.contains("tsk mission list <id> --json"));
        assert!(IMPORT.contains("Projections"));
        assert!(IMPORT.contains("write the summary or table you need yourself"));
    }

    #[test]
    fn the_empty_ledger_note_names_the_guide_command() {
        assert!(EMPTY_LEDGER_NOTE.contains("tsk guide import"));
    }
}
