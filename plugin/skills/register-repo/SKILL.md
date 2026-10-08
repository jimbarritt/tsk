---
name: register-repo
description: Add this repo's entry to the attached nexus's nexus.json, then commit and push it, so the repo's ledger can live in the nexus. Invoke explicitly as /tsk:register-repo [<id>] [<territory>], or when a natural-language request asks to register this repo in the nexus.
---

Backs the `/tsk:register-repo` command. This skill is the agent-facing wrapper around
`tsk nexus register-repo`, which does the deterministic work: it reads the repo's origin
URL, fetches the nexus, adds the entry, and commits and pushes `nexus.json`.

Never edit `nexus.json` by hand, and never clone, commit to or push the nexus repo
yourself. The command does all of it.

## Steps

1. **Check the nexus.** Run:

   ```bash
   tsk nexus list
   ```

   - `nexus: none attached`: no nexus is attached on this machine. Stop and ask me for
     the nexus URL. Do not guess one. With the URL, run `tsk nexus add <url>` and run
     `tsk nexus list` again.
   - Exit 1: report the command's stderr and stop.
   - Otherwise, note the territories it lists, and whether this repo already appears.

2. **Choose the options.** Use only values I gave you or values the command derives.
   - `--id <id>`: pass it only when I named an ID. Without it, the command derives the
     ID from the repo name. If I did not name one and you are not sure the derived ID is
     wanted, ask.
   - `--territory <id>`: required when the nexus lists more than one territory. Ask me
     which one unless I named it. Do not pick one yourself. With one territory, omit it.
     With none, ask me for a territory ID and a name, and pass both
     `--territory <id> --territory-name <name>`.
   - `--ledger nexus|repo`: omit it for the default, `nexus`. Pass `--ledger repo` only
     when I asked for the ledger to stay in this repo.
   - `--local`: pass it only when the repo has no origin remote URL, or when I asked for
     the entry to be local to this machine.

3. **Run the command.**

   ```bash
   tsk nexus register-repo [--id <id>] [--territory <id> [--territory-name <name>]] [--ledger nexus|repo] [--local]
   ```

4. **Handle the result.**
   - Exit 0, output `registered in territory <territory>: <entry>`: the entry is pushed.
     Tell me the entry and the territory, then run `tsk ledger fetch` as the output says.
   - Exit 0, output `already registered in territory <territory>: <entry>`: nothing
     changed. Tell me the entry as stored, then run `tsk ledger fetch`.
   - Exit 1, stderr names `--territory`: the nexus has more than one territory, or none.
     Ask me which territory to use, then run again with it. Do not guess.
   - Exit 1, stderr says an id or URL is already used by another entry: report it and
     ask me for an ID. Do not retry with an invented one.
   - Exit 1, stderr names `--local`: the repo has no origin URL that other machines can
     match. Ask me whether to register it for this machine only, then run again with
     `--local`.
   - Exit 1, any other error: report the command's stderr and stop.

If `tsk ledger fetch` already ran in this repo before the entry existed, its ledger
worktree stays in-repo. Tell me, and point me to the section "When `tsk ledger fetch`
ran before the entry existed" in `docs/user-guide/new-nexus-on-a-clean-machine.md` in the
tsk repository. Do not move the worktree yourself.
