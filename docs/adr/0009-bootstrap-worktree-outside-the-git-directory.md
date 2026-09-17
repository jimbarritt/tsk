# 9. The bootstrap worktree is checked out outside the git directory

Date: 2026-09-17

## Status

Accepted. Refines ADR 0008, which stands: the data still lives on the branch
`refs/heads/tsk/bootstrap`. Only the checkout location changes.

## Context

ADR 0008 kept the bootstrap worktree at
`$(git rev-parse --git-common-dir)/tsk/bootstrap-ref-wt`, a path inside the
repository's own `.git/` directory. The reasoning was that anything under `.git/`
is per-clone and untracked by construction, with no `.gitignore` entry to
maintain and no way for it to leak into `main`.

That reasoning holds, and the mechanism underneath it was never faulty: pushes
from that worktree landed on `refs/heads/tsk/bootstrap` correctly throughout. The
location failed for a different reason, observed during M-BOOT-02-01 on
2026-09-17.

1. **It reads as tampering with git internals.** A path containing `/.git/`
   carries a strong convention: that directory is git's private state, not a
   place to edit files. Jim twice stopped work in one session believing an agent
   was editing a ref, when it was editing an ordinary working-tree file in a
   linked worktree. Both times the mechanism was correct and the reading was
   reasonable. A path that repeatedly needs defending is a poor path.

2. **Tooling makes the same judgement.** Claude Code's auto-mode permission
   classifier escalated `Edit` calls against that path to a human prompt, while
   `git commit` and `git push` from the same session ran unprompted. Every
   mission-plan update therefore required a human approval, which defeats
   unattended operation. This is inference from observed behaviour rather than
   from documentation: the diagnostics log records no per-call permission
   verdicts. The correlation was consistent, and the path is the obvious
   discriminator.

3. **Local disk pollution was the original concern.** Keeping the checkout out
   of `.git/` must not mean scattering directories through the repository or the
   home directory.

## Decision

Check the worktree out at:

```
${XDG_STATE_HOME:-$HOME/.local/state}/tsk/repos/<clone-id>/bootstrap
```

- **Still a linked worktree**, not a separate clone. The metadata stays in the
  repository's `.git/worktrees/`, which is git's own business and is never
  edited by hand. A separate clone was considered and rejected: it would carry a
  second object store for no gain.
- **XDG state home**, not cache home. The worktree is regenerable from `origin`
  only while everything in it is pushed; an in-progress mission edit sitting in
  it is not regenerable. `~/.cache` carries a contract that its contents may be
  deleted at any time, which that work would not survive.
- **Per clone, not per machine.** A linked worktree belongs to one specific
  clone. A fixed path shared by two clones of tsk on one machine would leave the
  second clone resetting a worktree registered to the first.
- **`<clone-id>` is minted once and stored in `.git/tsk-clone-id`.** A hash of
  the repository path would change whenever the clone directory is renamed,
  orphaning the old worktree. A stored marker survives a rename or a
  `git worktree move`. The file is written by script and never edited by an
  agent, so it does not reintroduce the problem this ADR solves.
- **Nexus checkouts become a sibling of `repos/`**, not a child of it: a nexus
  indexes a territory rather than belonging to one repository, so it keys
  differently.

`ops/local/fetch-bootstrap-ref.sh` migrates an existing clone off the old
location on its next run, and refuses if that worktree holds uncommitted work
rather than destroying it.

## Consequences

- The worktree no longer appears anywhere inside the repository. `git worktree
  list` still shows it, which is the correct place for it to be visible.
- Two paths now need a machine-local home rather than one, since the nexus will
  join it. That is the point of choosing a conventional root.
- A second defect was found and fixed while making this change:
  `fetch-bootstrap-ref.sh` ran `git reset --hard` unconditionally, so calling it
  to resolve the worktree path silently discarded uncommitted work. Path
  resolution is now a separate, pure script,
  `ops/local/bootstrap-wt-path.sh`, and the fetch script refuses to reset over a
  dirty worktree. This defect predates this ADR and was latent in the original
  design.
- Anything holding the old path is stale. `$TSK_BOOTSTRAP_WT` is re-exported by
  the `SessionStart` hook each session, so it corrects itself.
- The deeper question of storing mission metadata separately from the main
  repository is unaffected and remains future work.

## References

Decided with Jim during M-BOOT-02-01, 2026-09-17. Refines
[ADR 0008](0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md).
