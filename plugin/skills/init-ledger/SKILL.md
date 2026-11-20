---
name: init-ledger
description: Create the tsk ledger of a repo that has none. Ask me first whether to create it in the attached nexus or in the repo. Invoke as /tsk:init-ledger, or when tsk ledger status prints `Next    ... tsk ledger init`.
---

Backs the `/tsk:init-ledger` command. This skill is the agent-facing wrapper around
`tsk ledger init`, which does the deterministic work: it adds the repo to the nexus's
`nexus.json` when a nexus is attached, creates the ledger, sets its mission prefix and
pushes it.

Never create a ledger with `tsk ledger fetch`, and never edit `nexus.json` by hand.

## Steps

1. **Check the state.** Run:

   ```bash
   tsk ledger status
   ```

   - `Local` is not `no ledger in this clone`: a ledger exists. Say so and stop.
   - `Local` says `marked as not under tsk`: this clone works without tsk. Say so and
     stop, unless I ask to start using tsk: then run `tsk ledger not-used --forget` and go
     on.
   - `Next` names `add_repo`: attach the nexus by following that line, clone it, run
     `tsk ledger status` again, and go back to the start of this step.
   - `Next` is `run tsk ledger fetch`: a ledger exists. Run it, say so, and stop.
   - `Next` names `check that git on this machine can fetch`: report the `Nexus` or
     `Origin` line and stop.
   - `Next` names `tsk ledger init`: go on.

2. **Ask where to create the ledger.** Read the `Nexus` line of the status.

   With a URL, use the AskUserQuestion tool. When the URL starts with `file://`, the nexus
   is a local nexus: name it with the path, which is the URL without `file://`.

   - `"In the local nexus (<path>)"` (recommended): the ledger is kept in the local nexus.
     For a URL that does not start with `file://`, the option reads
     `"In the nexus (<url>)"` (recommended): the ledger is kept in the nexus.
   - "In this repo": the ledger is on this repo's `origin`, public when the repo is public.

   With `Nexus   none attached`, use the AskUserQuestion tool with three options:

   - "Create a local nexus" (recommended): run `tsk nexus init-local --yes`. It makes a
     bare git repo at `~/.local/share/tsk/nexus.git` and attaches it. The folder holds the
     only copy of every ledger in it, so tell me to back it up. In a cloud session, do not
     offer this option.
   - "Attach a nexus by URL": ask for the clone URL and run `tsk nexus add <url>`.
   - "In this repo": the ledger is on this repo's `origin`, public when the repo is public.

   After a nexus is attached, go back to step 1.

3. **Choose the mission prefix.** A nexus needs a prefix for each repo: 3 or 4 capital
   letters, for example from the repo name. Propose one, and let me change it.

4. **Ask for the push workflow.** Only with a nexus. Use the AskUserQuestion tool:
   "Push to main or open pull requests?", with the options "Push to main" and "Open pull
   requests", and no default. The answer is saved in the `workflow` field of the repo's
   nexus entry. Skip this step when `Nexus` already shows `workflow: main` or
   `workflow: pull-request`.

5. **Name a new territory.** When the nexus has no territory, ask me for a territory id and
   a display name, then pass `--territory <id> --territory-name <name>`. With one
   territory, pass neither. With more than one, ask which and pass `--territory <id>`
   only. A territory that exists keeps its name.

6. **Run the command.**

   ```bash
   tsk ledger init --nexus --prefix <PREFIX> [--workflow main|pr] [--territory <id> [--territory-name <name>]]
   tsk ledger init --repo --prefix <PREFIX>
   ```

   A prefix that another repo uses stops the command: choose another and run it again.
   A repo whose nexus entry has a prefix needs no `--prefix`.

   The command prints the path of the ledger store on stdout. Exit 1 prints the reason
   on stderr: report it. With `[attribution] identity robot`, the command registers the
   robot of this session in the new ledger before its first commit. When the session has no
   robot name, the command stops before it changes anything: register the robot with
   `tsk actor register "<name>" --kind robot` or the `/tsk:construct-robot` skill, and run
   the command again.

7. **Report.** Tell me where the ledger is, its prefix and its workflow, and run
   `tsk ledger status` to show it. Then go on with the work I asked for, or ask which
   mission to create.

## When I choose to work without tsk

Session start offers "Work without tsk" beside initialising a ledger. After I choose it,
ask whether to remember the choice, with the AskUserQuestion tool: "Remember this choice
for this repo?", with the options "Yes" and "No". Run `tsk ledger not-used` only on "Yes".
It records the choice in the `tsk` field of the repo's nexus entry when the attached nexus
lists the repo, and in `.git/tsk-not-used` of the clone otherwise. On "No", record nothing
and go on with the work I asked for.
