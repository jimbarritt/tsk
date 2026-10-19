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
any format into the tsk ledger as records. tsk does not parse other plan formats. You
read the plan, and you write it into the ledger through `tsk` commands.

## Rules

- Write the ledger only through `tsk` commands. The ledger is a Dolt database. Do not
  open, edit or create files in the ledger store (`tsk ledger path`), and do not run
  `dolt` or `git` against it.
- Do not read or clone the tsk source code to learn the format. This guide holds what
  you need. `tsk <command> --help` shows the exact syntax of any command.
- Leave the original plan files where they are. Ask the person before you delete or move
  them.
- Show the person your mapping (step 3) before you write anything.

## What the ledger holds

The ledger holds two kinds of thing.

- **Records** are the source of truth for the state of the work: missions, objectives,
  constraints, what is out of scope, tasks, decisions, questions, blocks, and the links
  from a mission to documents. They are typed rows in the database. Write them only
  through the `tsk mission`, `tsk task`, `tsk decision` and `tsk question` commands.
- **Documents** are free text: intelligence, research, reports and designs. They are
  Markdown files on the branch `tsk/docs`. Write them with `tsk ledger doc put`, and link
  each one to a mission with `tsk mission link`.

The principle: state lives in records, and prose lives in documents. Do not write a fact
in both places. A document that repeats a mission's purpose, tasks, decisions or
questions drifts away from the records, and the records win. Do not write a briefing
document, an index, or a plan table. The records replace them.

Threads, continuation entries and session bindings are also in the ledger. `tsk thread`
commands write them. An import does not create threads.

## Mapping a plan to records

| In the plan | In the ledger |
|---|---|
| A goal or project | A mission: `tsk mission create`, then `set` for its purpose and decision authority. A mission ID is `M-` and a short upper-case name, for example `M-SETUP`, or `M-SETUP-02` for a sub-mission (`--parent`). |
| A step under it | A task: `tsk task add`, then `task set` for its status (`todo`, `in-progress` or `done`). tsk numbers tasks `T-01`, `T-02` and so on within the mission. |
| A step or goal that waits for another | `tsk task block` for tasks of one mission, `tsk mission block` for missions. |
| A decision already made | `tsk decision add`, with `--by` for who decided, `--on` for the date if it is not today, `--task` if it concerns one task, and `--adr` if an ADR in the repo records it. |
| An open question | `tsk question add`. When it is answered, `tsk question answer`. |
| What the goal achieves | Objectives: `tsk mission objective add`. Tick one with `objective done`. |
| Limits on how the work is done | Constraints: `tsk mission constraint add`, one line each. |
| What the work does not cover | Out of scope: `tsk mission out-of-scope add`, one line each. |
| Notes, research, reports and reference material | Documents on `tsk/docs`, under `missions/<category>/<ID>/`, written with `tsk ledger doc put` and linked with `tsk mission link <id> <path> --role intelligence`, `report` or `design`. |
| An idea not yet planned | It has no mission yet. Put the idea in a document and link it to an administrative idea-capture mission (create `M-IDEAS` with `--category administrative` once), or create a mission named for the idea if it is clear enough to name. |
| A one-off task that belongs to no goal | A task on an administrative mission, for example `M-ADMIN`. An administrative mission has no end, and its status stays `in-progress`. |

A mission is done when you set its status to `done` with `tsk mission set <id> status
done`. tsk does not check its objectives or its tasks. Set the status yourself, and only
when the person agrees the mission is done.

A mission that is already done in the plan needs only its record, a one-line purpose and
its status. Link its final report as a document if one exists.

## Reading a mission back

`tsk mission list` lists every mission, one per line: id, status, blocked-by, parent,
name, in aligned columns. A script adds `--porcelain` for tab-separated columns.
`tsk mission list <id> --json` prints one mission in full: purpose, decision authority,
constraints, out of scope, objectives, blocks, linked documents, tasks, decisions, and
questions, open and answered. `tsk task list <id>`, `tsk decision list
<id>` and `tsk question list` print the parts.

There is no command that prints a mission as Markdown yet. Projections, which render the
records as Markdown for people, are planned. Until they exist, read the JSON and write
the summary or table you need yourself, for the person you are working with. Do not store
that summary in the ledger.

## Writing many records

Each write command changes the local ledger store only. Nothing reaches the remote until
`tsk ledger push`. You can write a whole import as a list of commands, in a shell script
or one after another, and check the result before you push.

- `tsk ledger status` shows what is not pushed, a line per changed table or document.
- `tsk ledger discard` lists what it would drop. `tsk ledger discard --yes` drops every
  local change, in the records and the documents, and resets both to origin. Use it to
  undo a mistake before the push. After the push, correct a record with `set`, `edit` or
  `unlink`. Never run `dolt reset`, `dolt commit` or any other `dolt` or `git` command on
  the ledger store.
- `tsk ledger push "<message>"` publishes the records and the documents together.

Quote every text argument. A value of `-` for `mission set` is read from stdin, which
suits a purpose of several lines.

## Commands

```bash
tsk mission create <id> "<name>" [--category administrative] [--parent <id>]
tsk mission set <id> <name|status|purpose|decision-authority> <value|->   # - reads stdin
tsk mission block <id> <blocking-id>
tsk mission unblock <id> <blocking-id>
tsk mission objective add <id> "<outcome>"
tsk mission objective done <id> <n>
tsk mission constraint add <id> "<text>"
tsk mission out-of-scope add <id> "<text>"
tsk mission link <id> <path> --role <intelligence|report|design>
tsk mission list [<id>] [--json] [--porcelain]
tsk task add <mission> "<description>"          # prints T-nn
tsk task set <mission> <task> <status|description> <value>
tsk task block <mission> <task> <blocking-task>
tsk task list <mission> [--porcelain]
tsk decision add <mission> "<text>" --by <who> [--task <task>] [--on <YYYY-MM-DD>] [--adr <path>]
tsk decision list [<mission>] [--adr <path>] [--porcelain]
tsk question add <mission> "<text>"             # prints Q-nn
tsk question answer <mission> <question> "<text>" --by <who>
tsk question list [<mission>] [--porcelain]
tsk ledger doc put <path> [<file>]              # body from the file, or from stdin
tsk ledger doc list [<prefix>]
tsk ledger doc get <path>
tsk ledger status
tsk ledger discard [--yes]
tsk ledger push "<message>"
tsk ledger migrate-docs
```

`tsk mission unlink`, `objective undone|remove`, `constraint remove`, `out-of-scope
remove`, `task unblock`, `question edit` and `ledger doc rm` correct a mistake. Write a
document from stdin with a here-document:

```bash
tsk ledger doc put missions/operational/M-SETUP/research.md <<'EOF'
# Research: machine setup
...
EOF
```

## Layout 1 ledgers

A ledger at layout 1 still keeps its documents in a Dolt table. Every `tsk` command
except `tsk ledger fetch` stops and names `tsk ledger migrate-docs`. Run that command
once to move the documents to `tsk/docs`. It rewrites shared data, so tell the person
first. The first record write on a newer tsk upgrades the ledger to the current layout,
and a tsk binary older than that layout then stops with `unsupported ledger version`.
`tsk self-update` brings the binary up to date.

## Steps

1. Find the existing plan. Look for plan, roadmap, todo or task files in the repo and in
   any directory the person names, and ask the person where else their plan is kept.
2. Read it. Note each goal or project, each step under it, what is done, what is in
   progress, what is blocked, each decision, each open question, and each idea not yet
   planned.
3. Map it with the table above, and show the mapping to the person before you write:
   the missions with their IDs and names, the task count of each, and the documents with
   their paths. Wait for the person to agree.
4. Run `tsk ledger fetch`. If it says the ledger needs `tsk ledger migrate-docs`, follow
   the section on layout 1 ledgers.
5. Write the records, mission by mission: create it, set its purpose, add objectives,
   constraints and out of scope, add tasks and set their status, add blocks, decisions
   and questions. Write each document with `tsk ledger doc put` and link it.
6. Check the local result with `tsk ledger status` and `tsk mission list`. Fix a mistake
   with the correcting commands, or run `tsk ledger discard --yes` to start again.
7. Run `tsk ledger push "Import the existing plan"`.
8. Check the pushed result with `tsk mission list`, and read one mission in full with
   `tsk mission list <id> --json`. Compare it with the plan.
9. Tell the person what you imported and where the original files are. Ask what to do
   with the original files, and whether to start a thread on a mission with
   `/tsk:start-thread`.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn the_import_guide_names_the_write_commands_and_the_rules() {
        assert!(IMPORT.contains("tsk ledger doc put"));
        assert!(IMPORT.contains("tsk ledger push"));
        assert!(IMPORT.contains("Do not read or clone the tsk source code"));
        assert!(IMPORT.contains("Show the person your mapping"));
        assert!(IMPORT.contains("Leave the original plan files where they are"));
        assert!(IMPORT.ends_with('\n'));
    }

    #[test]
    fn the_import_guide_says_records_are_the_truth_and_how_to_read_a_mission() {
        assert!(IMPORT.contains("source of truth"));
        assert!(IMPORT.contains("tsk mission list <id> --json"));
        assert!(IMPORT.contains("Projections"));
        let flat = IMPORT.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(flat.contains("write the summary or table you need yourself"));
    }

    #[test]
    fn the_import_guide_states_when_a_mission_is_done_and_how_to_undo_a_mistake() {
        let flat = IMPORT.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(flat.contains("tsk mission set <id> status done"));
        assert!(flat.contains("tsk does not check its objectives"));
        assert!(IMPORT.contains("tsk ledger status"));
        assert!(IMPORT.contains("tsk ledger discard --yes"));
        assert!(IMPORT.contains("Never run `dolt reset`"));
        assert!(IMPORT.contains("tsk ledger migrate-docs"));
        assert!(IMPORT.contains("--role intelligence"));
        assert!(IMPORT.contains("administrative"));
    }

    #[test]
    fn the_import_guide_no_longer_describes_briefing_documents() {
        assert!(!IMPORT.contains("index.md"));
        assert!(!IMPORT.contains("Open questions"));
        assert!(!IMPORT.contains("Plan table\n"));
        assert!(!IMPORT.contains("## A mission briefing"));
    }

    /// The `bash` block under "## Commands", one command per line.
    fn command_lines() -> Vec<&'static str> {
        let after = IMPORT.split("## Commands").nth(1).unwrap();
        let block = after.split("```bash\n").nth(1).unwrap();
        block.split("```").next().unwrap().lines().collect()
    }

    /// Fills placeholders and drops optional parts, so a line parses as a real command.
    fn concrete(line: &str) -> Vec<String> {
        let code = line.split("  #").next().unwrap();
        let mut text = String::new();
        let mut depth = 0;
        for c in code.chars() {
            match c {
                '[' => depth += 1,
                ']' => depth -= 1,
                '"' | '<' | '>' => {}
                c if depth == 0 => text.push(c),
                _ => {}
            }
        }
        text.split_whitespace()
            .map(|word| {
                if word == "n" {
                    "1".to_string()
                } else {
                    word.to_string()
                }
            })
            .collect()
    }

    #[test]
    fn every_command_in_the_import_guide_parses() {
        let lines = command_lines();
        assert!(lines.len() > 20);
        for line in lines {
            let words = concrete(line);
            assert_eq!(words[0], "tsk", "{}", line);
            if let Err(e) = crate::Cli::try_parse_from(&words) {
                panic!(
                    "the guide shows a command that does not parse: {}\n{}",
                    line, e
                );
            }
        }
    }

    #[test]
    fn the_empty_ledger_note_names_the_guide_command() {
        assert!(EMPTY_LEDGER_NOTE.contains("tsk guide import"));
    }
}
