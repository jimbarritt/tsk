---
name: init-ledger
description: Create the tsk ledger of a repo that has none, in the attached nexus or in the repo, after asking me where it goes. Invoke as /tsk:init-ledger, or when tsk ledger status prints `Next    ... tsk ledger init`.
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
   - `Next` names `add_repo`: attach the nexus as it says, clone it, run
     `tsk ledger status` again, and go back to the start of this step.
   - `Next` is `run tsk ledger fetch`: a ledger exists. Run it, say so, and stop.
   - `Next` names `check that git on this machine can fetch`: report the `Nexus` or
     `Origin` line and stop.
   - `Next` names `tsk ledger init`: go on.

2. **Ask where the ledger goes.** With a `Nexus` line that names a URL, use the
   AskUserQuestion tool:

   - "In the nexus" (recommended): the ledger is private to the nexus repo.
   - "In this repo": the ledger is on this repo's `origin`, public when the repo is public.

   With `Nexus   none attached`, the ledger goes in this repo, unless I give a nexus URL.
   With a URL, run `tsk nexus add <url>` and go back to step 1.

3. **Choose the mission prefix.** A nexus needs a prefix for each repo: 3 or 4 capital
   letters, for example from the repo name. Propose one, and let me change it.

4. **Run the command.**

   ```bash
   tsk ledger init --nexus --prefix <PREFIX>
   tsk ledger init --repo --prefix <PREFIX>
   ```

   Add `--territory <id>` when the command says the nexus has more than one territory. A
   prefix that another repo uses stops the command: choose another and run it again.

   The command prints the path of the ledger store on stdout. Exit 1 prints the reason
   on stderr: report it.

5. **Report.** Tell me where the ledger is and its prefix, and run `tsk ledger status`
   to show it. Then go on with the work I asked for, or ask which mission to create.
