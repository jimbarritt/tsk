---
name: self-update
description: Update the tsk binary to the latest release and report the old and new version. Invoke explicitly as /tsk:self-update, or when asked to update tsk.
---

Backs the `/tsk:self-update` command. This skill is the agent-facing wrapper around
`tsk self-update`, which does the deterministic work: it installs the latest release by the
method that installed `tsk` (Homebrew, Cargo or a release binary).

The skill updates the `tsk` binary only. The plugin updates at the next session start.

## Steps

1. **Read the version now.**

   ```bash
   tsk --version
   ```

2. **Check for a newer release.**

   ```bash
   tsk self-update --check
   ```

   - Exit 0 and the output says `tsk` is up to date: tell me the version and stop.
   - Exit 0 and the output names a newer release: go on.
   - Exit 1: report stderr and stop.

3. **Update.**

   ```bash
   tsk self-update
   ```

   The command can take a minute. Do not pipe it through `head` or `tail`: read its exit
   code. Exit 1: report stderr and stop. Do not retry with another install method.

4. **Read the version again.**

   ```bash
   tsk --version
   ```

5. **Tell me** the old version, the new version, and that the plugin and this session's hooks
   change at the next session start: a restart of Claude Code, or a new cloud session.

## Notes

- `tsk doctor` lists every `tsk` on `PATH`. If the version in step 4 is the old one, another
  copy shadows the new one. Run `tsk doctor` and report its output.
- `TSK_AUTO_UPDATE=never` switches off the automatic update that some commands run. It does
  not stop this command.
