# A new Claude cloud environment with tsk installed

These steps create a Claude Code cloud environment on claude.ai/code in which every new
session finds the `tsk` binary on `PATH`, the `tsk` plugin loaded, and the ledger
fetched.

An environment is created by a person, through the web or the Desktop app. No session or
API call creates one. See
[session-creation-and-environments.md](../kb/session-creation-and-environments.md#constraints).

## What a session needs

| Item | Provided by |
|---|---|
| The repo's code | The cloud session clones it from GitHub. |
| `tsk` and `dolt` on `PATH` | The environment's setup script (step 3). |
| Access to the GitHub release downloads | The environment's network access setting (step 2). |
| The `tsk` plugin | The repo's `SessionStart` hook (step 4). |
| `$TSK_LEDGER_WT` and the ledger worktree | `tsk thread session-start`, run by the same hook. |

A cloud session does not load plugins installed on your machine, or plugins that the
repo's `.claude/settings.json` enables. The `SessionStart` hook installs the plugin from
the command line, which works in a cloud session. See
[claude-code-plugin-packaging.md](../kb/claude-code-plugin-packaging.md#where-plugins-load).

## 1. Create the environment

1. Open claude.ai/code and start a new session. Do not type a prompt yet.
2. Select the small cloud icon next to the prompt box. It opens environment selection.
3. Create a new environment from there. No existing environment is needed first.
4. Name it, for example `tsk`.

Once created, the environment is selectable in every later session.

## 2. Set network access

The environment's network access setting has four levels:

| Level | Meaning |
|---|---|
| None | Blocks internet access for maximum security. |
| Trusted (Recommended) | Downloads packages from verified sources. |
| Full | Unrestricted internet access for maximum flexibility. |
| Custom | A list of allowed domains. |

Select Trusted.

The script and the session hooks use these hosts:

- `github.com`, for the release downloads, the plugin marketplace and the ledger.
- `release-assets.githubusercontent.com` and `objects.githubusercontent.com`, which
  serve the release downloads after a redirect from `github.com`.

If a host is blocked, use Custom and add it, or use Full.

## 3. Add the setup script

Paste this into the environment's setup script field:

```bash
#!/bin/bash
set -euo pipefail

case "$(uname -m)" in
  x86_64) TSK_TARGET=x86_64-unknown-linux-gnu; DOLT_ARCH=amd64 ;;
  aarch64) TSK_TARGET=aarch64-unknown-linux-gnu; DOLT_ARCH=arm64 ;;
  *) echo "unsupported machine: $(uname -m)" >&2; exit 1 ;;
esac

# tsk: the prebuilt binary from the latest GitHub release.
latest="$(curl -sSfLI -o /dev/null -w '%{url_effective}' https://github.com/jimbarritt/tsk/releases/latest)"
TSK_VERSION="${latest##*/v}"
curl -sSfL "https://github.com/jimbarritt/tsk/releases/download/v$TSK_VERSION/tsk-$TSK_VERSION-$TSK_TARGET.tar.gz" |
  tar -xz --no-same-owner -C /usr/local/bin tsk

# dolt: the ledger store.
DOLT_VERSION=1.88.2
tmp="$(mktemp -d)"
curl -sSfL "https://github.com/dolthub/dolt/releases/download/v$DOLT_VERSION/dolt-linux-$DOLT_ARCH.tar.gz" |
  tar -xz --no-same-owner -C "$tmp"
cp "$tmp/dolt-linux-$DOLT_ARCH/bin/dolt" /usr/local/bin/dolt
dolt config --global --get user.name >/dev/null 2>&1 || dolt config --global --add user.name tsk
dolt config --global --get user.email >/dev/null 2>&1 || dolt config --global --add user.email tsk@localhost

mkdir -p /usr/local/share/tsk-setup
date -u +%Y-%m-%dT%H:%M:%SZ > /usr/local/share/tsk-setup/last-run
tsk --version
dolt version | head -n 1
```

The script reads the latest release from the redirect of
`github.com/jimbarritt/tsk/releases/latest`, downloads the prebuilt `tsk` for the
machine, and installs it to `/usr/local/bin`, which is on `PATH`. It installs `dolt`
1.88.2 beside it and sets a Dolt identity when none is set. It needs no Rust toolchain.
Measured in a cloud session on 2026-10-13: about 6 seconds.

The plugin's own `SessionStart` hook also installs `dolt`, but a plugin installed during
startup has no hooks in that session (step 4), so the setup script installs it.

Setup scripts and sessions both run as root, observed on 2026-10-08, so the binaries
are readable in the session.

The `last-run` lines write the run time to `/usr/local/share/tsk-setup/last-run`. They
support the cache test below and can be removed afterwards.

To pin a version, set `TSK_VERSION=<version>` in place of the two `latest` lines.

## 4. Add the session start hook to the repo

### Why a cloud session needs a hook in the repo

Each cloud session runs in a new container. Claude Code installs plugins under
`~/.claude/` in the container, so a plugin installed in one session is gone in the next.
A cloud session also ignores the plugins installed on your own machine, and the plugins
that `enabledPlugins` in the repo's `.claude/settings.json` turns on (see
[claude-code-plugin-packaging.md](../kb/claude-code-plugin-packaging.md#where-plugins-load)).

The repo's own files are the one thing every cloud session has. So the repo holds a
`SessionStart` hook in `.claude/settings.json`, and Claude Code runs it at the start of
every session:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "tsk thread session-start --install-plugin claude-cli"
          }
        ]
      }
    ]
  }
}
```

At each session start the hook:

1. Installs the `tsk` plugin, only in a cloud session (`CLAUDE_CODE_REMOTE` is `true`).
   On your own machine the plugin stays installed, so the hook skips this step.
2. Fetches the ledger, exports `$TSK_LEDGER_WT`, and prints the thread binding context.
   This step runs in every session, local and cloud.

`tsk thread session-start` without `--install-plugin` never installs the plugin. A
failed install adds a warning to the session context and does not stop step 2.

### Add the hook once per repo

The hook is committed to the repo, so it is added once per repo, not once per session or
per environment. Use either way.

**From a session in the repo.** In the first session, the repo has no hook yet, so the
session start ran nothing. Run:

```bash
tsk install-plugin claude-cli
tsk thread session-start </dev/null
git add .claude/settings.json && git commit -m "Add the tsk session start hook" && git push
```

1. `tsk install-plugin claude-cli` installs the plugin and writes the hook into
   `.claude/settings.json`. It keeps any other settings in the file, and a second run
   changes nothing.
2. `tsk thread session-start </dev/null` gives this session the ledger context, which a
   hook would have given at its start.
3. The commit puts the hook in the repo for every later session.

The plugin's slash commands, such as `/tsk:start-thread`, can stay absent until the
next session, because Claude Code reads plugin skills when its process starts.

**Before the first session.** Add `.claude/settings.json` with the content above to the
repo, for example in the GitHub web editor. The first cloud session then runs the hook,
and needs no command.

### Every later session

Nothing. The hook installs the plugin in a cloud session and fetches the ledger, in any
environment that has `tsk` and `dolt` on `PATH` (step 3).

The binary claims each session start event by session ID and source. When the plugin's
own `SessionStart` hook also runs, the second run prints nothing, so the context appears
once.

### The tsk repo

The tsk repo has its own hook, `ops/local/claude-session-start.sh`, which also builds
`tsk` from `cli/` when the installed binary is not at the workspace version. It needs
nothing from this step.

A repo that keeps its ledger in a nexus needs the nexus attached in the environment.
Set the nexus URL in `~/.config/tsk/config.toml` from the setup script or the hook, with
`tsk nexus add <nexus url>`, or `tsk config attach-nexus <nexus url>` with a release that
has no `nexus` command. `tsk nexus list` shows the attached nexus and the repos it lists. See
[new-nexus-on-a-clean-machine.md](new-nexus-on-a-clean-machine.md).

## 5. Check the environment

Start a session in the new environment, selecting your repo. In the session, run:

```bash
tsk --version
dolt version
echo "$TSK_LEDGER_WT"
claude plugin list
```

Expected:

- `tsk --version` prints the installed version, and `dolt version` prints 1.88.2.
- `$TSK_LEDGER_WT` names a directory that exists.
- `claude plugin list` shows `tsk@jimbarritt-claude-plugins` enabled.

The session's first context message names the ledger worktree path and the thread
binding, if one exists.

## Test whether the setup result is cached

`/clear` does not test this. It starts a new conversation in the same Claude Code
process and the same container, so the setup script does not run again either way.
Compare two separate sessions:

1. Save the environment with the stamp lines from step 3.
2. Start session A in the environment. Run `cat /usr/local/share/tsk-setup/last-run`,
   `date -u` and `whoami`. Note the output and how long the session took to become
   usable.
3. End session A. Start session B a few minutes later in the same environment and run
   the same commands.
4. The same `last-run` time in A and B means the setup result was reused. A later time
   in B means the script ran again.
5. Repeat after an hour or more, and after a day, to find whether the result expires.
6. Edit the script, for example by adding a comment line, and start session C. A
   `last-run` time later than B's confirms that an edit invalidates the result.

The stamp shows reuse only when the cache keeps the container's filesystem.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `tsk: command not found` or `dolt is not installed` | The setup script failed. | Run the setup script's commands in the session and read the error. |
| A download in the setup script fails | A GitHub download host is blocked by the network access level. | Add the hosts in step 2 under Custom, or use Full. |
| The `SessionStart` context reports `WARNING: adding the ... marketplace failed` | `github.com` is blocked, or the marketplace repo is not readable. | Allow `github.com`. Confirm `jimbarritt/claude-plugins` is readable from the session. |
| A session starts with no tsk context, and `/tsk:` commands are missing | The repo has no `SessionStart` hook yet. | Add it once, see [step 4](#add-the-hook-once-per-repo). |
| `$TSK_LEDGER_WT` is empty | The hook did not run, or `tsk thread session-start` failed. | Run `WT="${TSK_LEDGER_WT:-$(tsk ledger fetch)}"` and read any error. |
| `tsk ledger push` is rejected | The cloud sandbox proxy refuses writes outside `refs/heads/*`. | The ledger is the branch `tsk/ledger`, not a custom ref. See [ADR 0008](../adr/0008-bootstrap-data-on-a-detached-branch-not-a-custom-ref.md). |
| `git merge-base` reports unrelated branches | The clone is shallow. | `git fetch --unshallow origin`. The hook does this at session start. |
| A new `tsk` release is not picked up | The setup script result is cached for the environment. | Edit the script, for example by pinning `TSK_VERSION`, so the environment rebuilds. |

## Observed

On 2026-10-08, with the earlier script that built `tsk` with `cargo install`:

- The setup script and the session both run as root, with `HOME=/root`.
- A new session reuses the setup result. A session whose container booted at
  05:53:48Z read a `last-run` stamp of 05:48:41Z, written before the boot. The script did
  not run at session start.
- Two sessions read stamps two seconds apart, 05:48:39Z and 05:48:41Z. The cause is not
  established.
- `/clear` keeps the same container and does not test the cache.

## Not verified

- Whether the Trusted level allows the GitHub release download hosts. The run on
  2026-10-13 was in a session with a wider network policy.
- How long the setup result is kept, and whether editing the script invalidates it.
