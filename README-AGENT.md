# tsk for agents

Read this first if you are an agent in a repo that uses tsk. It covers where the state of
the work is, how to read and change it, and what to do when something is wrong. The
`tsk` plugin skill is the full command reference: load it, or run any command with
`--help`.

## The state of the work is in the ledger

The ledger contains records: missions, their objectives, constraints and out-of-scope items,
tasks, decisions, open questions, and threads with their continuation state. Records are
typed rows in a Dolt database. Write them only through `tsk` commands. Do not look for
a plan file, a briefing or an index document: there is none. Free text (research,
reports, designs) is Markdown documents on a second ref, read and written with
`tsk ledger doc`.

Read the state with these commands:

```bash
tsk mission list                 # every mission: id, status, blocked-by, parent, name
tsk mission list <id> --json     # one mission in full: purpose, objectives, tasks,
                                 # decisions, questions, linked documents
tsk task list <id>               # the tasks of one mission
tsk decision list [<id>]         # decisions of one mission, or of all
tsk question list [<id>]         # open questions of one mission, or of all
tsk ledger doc get <path>        # one document
tsk ledger doc list [<prefix>]   # the documents
```

The `--json` form of `mission list` has a `blocked` flag on the mission and on each task.
The list forms show only blockers that are not done.

## Change the state with commands

```bash
tsk task add <mission> "<text>"                 # prints the new task id
tsk task set <mission> <T-nn> status done        # todo, in-progress or done
tsk decision add --by <who> <mission> "<text>"   # --task <T-nn> ties it to a task
tsk question add <mission> "<text>"
tsk question answer --by <who> <mission> <Q-nn> "<text>"
tsk mission objective done <mission> <n>
tsk ledger doc put <path> [<file>]               # body on stdin or from a file
tsk mission link <mission> <path> --role intelligence|report|design
```

Every write changes the local ledger store only. A write never waits for the network.
Record what you decide and why, as a decision, in one line.

## When records are published

Records are published at four points: `tsk thread start`, `tsk thread pause` and
`tsk thread stop`, the end of the session (a hook), and `tsk ledger push "<message>"`
when you want them out sooner. The `Stop` hook publishes any unpushed work in the
background after a turn. Taking a task is a claim that others must see at once: after you
set a task to `in-progress`, run `tsk ledger push "Claim <mission> <task>"`.

`tsk ledger status` shows what is not yet pushed and whether origin is ahead. A push that
has to merge renumbers your own new rows if their numbers collide with rows already on
origin, and prints each change, for example `T-03 is now T-05 in M-X`. Use the new numbers
from then on.

## Threads: where work stops and starts

A thread is one line of work. Your session binds to a thread, and the thread names a
mission, or none.

- `tsk thread start <mission>` starts a thread and binds this session to it.
- `tsk thread pause <thread> <T-nn> "<where things stand>"` records a continuation entry
  and publishes. Run it before you run out of context and before `/clear`. With `--bound`
  it takes the thread this session is bound to: `tsk thread pause --bound <T-nn> "<...>"`.
  After `paused:<thread>` it prints `code: clean`, `code pushed: yes` and `ledger: pushed`.
  Say the session is safe to clear only when all three print.
- `tsk thread resume <thread>` binds this session to a thread and prints its latest entry, with the mission name and the task description.
- `tsk thread list` shows every thread and who is bound to it.

The session start hook prints the bound thread and its mission, or the mission list when
nothing is bound. The plugin has skills for these: `/tsk:start-thread`,
`/tsk:pause-thread`, `/tsk:resume-thread`.

## Rules

- Never fetch, update or push the ledger by hand. Use `tsk ledger fetch` and
  `tsk ledger push`. Do not run `dolt` or `git` against the ledger store or the ledger
  refs yourself.
- Do not open the files of the ledger store. It is a database. Documents are the one
  thing to read as files, through `tsk ledger doc path <path>`.
- Do not repeat a record in a document. A document is for free text.
- A git command you run by hand against a ledger ref names it in full, for example
  `refs/heads/tsk/ledger` or `refs/tsk/ledger`.

## When something is wrong

| What you see | What it means | What to do |
|---|---|---|
| `unsupported ledger version` | The ledger is newer than this `tsk`. | Nothing: `tsk` updates itself and runs the command again. From a hook, run `tsk self-update`. |
| `origin refused the push to refs/tsk/...` | The repo is used from a cloud session, and a push of a custom ref fails at the cloud proxy. | Run `tsk config ledger-refs branches`, commit `.tsk/config.toml`, push again. |
| A push exits 1 and names tables | Two actors changed the same field of the same row. | Read the conflict, fix the row with a `set` command, push again. The local commit stays. |
| `warning: the last background publish failed` | The `Stop` hook's publish was not pushed to origin. | Run `tsk ledger push "<message>"` and read its error. The warning shows the first 20 lines of the failing command's stderr or the hook's output. |
| `tsk ledger doc put` or `tsk ledger push` exits 1 and prints findings, then `the documents hook ... failed` | The documents hook (`tsk config docs-hook` names it) rejected a document. | Fix the document the findings name and put it again, then push. Do not set `TSK_SKIP_DOC_CHECK=1`. |
| `$TSK_LEDGER_WT` is not set | The session start hook did not run. | Run `tsk ledger fetch`. It prints the ledger store path. |
| `database is read only` | A local Dolt server from a newer `tsk` has the store open. | Run `tsk ledger server stop`, or update `tsk`. A `tsk` older than 0.23.0 meets this on every write once the server is on by default (0.29.0); 0.23.0 is the minimum that stops the server itself. |

## Where to read more

- The plugin skill `tsk` (`plugin/skills/tsk/SKILL.md` in the tsk repo).
- `tsk guide import`, how to bring an existing plan into the ledger as records.
- In the tsk repo: `docs/domain/ledger-layout.md` for every table and command,
  `docs/domain/session-continuation-design.md` for threads and bindings, and the ADRs
  under `docs/adr/` for the reasons behind the design.
