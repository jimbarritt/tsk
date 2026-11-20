# Robot commits: author, signature and Verified badge

These steps set up a repo so that a commit made by a robot session names the robot as its
author and shows a Verified badge on GitHub.

Steps 1 to 5 are done once, by a person. Each robot session does step 6. Steps 7
and 8 check the result.

## Robot attribution is optional

The feature is off unless a repo or a user turns it on. With nothing set, tsk writes no git
config, and commits keep the author that the environment gives them.

To turn it on for yourself on one machine, in every repo, write the user config
(`~/.config/tsk/config.toml`). The command needs no repo:

```bash
tsk config attribution robot --email 352782120+jmdb-robots@users.noreply.github.com --user
```

To turn it on for every session of one cloud environment, pass the robot email to
`ops/cloud/setup-environment.sh` in the environment's setup script, and keep the robot key
in the secret `TSK_ROBOT_SIGNING_KEY`:

```bash
curl -sSfL https://raw.githubusercontent.com/jimbarritt/tsk/main/ops/cloud/setup-environment.sh | bash -s -- --use-robot-attribution 352782120+jmdb-robots@users.noreply.github.com
```

With the option, the script writes the user config with `--user`. It adds no Claude Code
permission rules. It also writes `~/.claude/AGENTS.md`, imported from `~/.claude/CLAUDE.md`:
the platform's Stop hook names an unpushed robot commit as Unverified and prints commands
that reset the author to Claude, and the file directs a session to ignore them and push the
commit in the same turn. An environment without the option stays off, and a clone
elsewhere stays off. This repo's cloud environment does this. The setup script runs when a
container is built, so a change takes effect in the next new container.
Robot signing works in a session with no permission rules.

The variables `TSK_ATTRIBUTION_IDENTITY` and `TSK_ATTRIBUTION_EMAIL` outrank both config
files, for a case that needs an override. A session that started before they were set
picks them up at its next start, or at once with `tsk actor git-identity`.

To turn it on for everyone who clones a repo, run the command without `--user` at the
top of the repo and commit `.tsk/config.toml`. A repo config outranks the user config, so
nobody can turn it off for themselves in that repo. `tsk config attribution` prints what is
in force and where each value came from.

A robot commit is signed and shows Verified only after steps 1 to 5 are done, and a session
reads the signing key from the environment variable `TSK_ROBOT_SIGNING_KEY`. Without the signing key, `tsk`
prints a warning and commits keep the default author. It needs `tsk` 0.37.0 or later.

## What a robot commit looks like

| Field | Value |
|---|---|
| Author name and committer name | The registered actor name, for example `Credit Where It's Due`. |
| Author email and committer email | The `email` setting, the same for every robot in the repo. This repo sets `352782120+jmdb-robots@users.noreply.github.com`. |
| Signature | An SSH signature made with one shared robot signing key. |
| Badge | Verified, because the committer email and the signing key belong to the same GitHub account. |

All robots share one email, one GitHub account and one key. The name in the commit shows
which robot made it. GitHub shows the shared account's avatar and username on every
robot commit, and the robot name in the commit details.

The robot account must have "Keep my email addresses private" turned on in Settings, then
Emails, so that GitHub accepts the noreply address. The number before the `+` in the address
is the account's user ID.

## What you need

- Access to the `jimbarritt.com` mailbox for `robots@jimbarritt.com`. GitHub sends a
  message to it in step 2.
- Permission to edit the Claude cloud environment (step 5).
- A machine with `ssh-keygen` (step 3).

## 1. Create the shared GitHub robot account

1. Sign out of GitHub, or open a private browser window.
2. Create a new account at github.com/signup. Use `robots@jimbarritt.com` as its email.
3. Choose a username that shows the account belongs to the robots.
4. Follow the verification link GitHub sends to `robots@jimbarritt.com`.

If GitHub does not send a message, check the mailbox for `robots@jimbarritt.com` and its
spam folder.

## 2. Verify the email on the account

Skip this step if step 1 used `robots@jimbarritt.com` as the sign-up email and you followed
the link.

1. Sign in as the robot account.
2. Open Settings, then Emails.
3. Add `robots@jimbarritt.com`.
4. Open the message GitHub sends to that address, and follow its link.
5. Check that Emails lists the address as Verified.

GitHub marks a commit Verified only when its committer email is a verified email of the
account that has the signing key.

## 3. Generate the signing key

Run this on your own machine, not in a session.

```bash
ssh-keygen -t ed25519 -C "robots@jimbarritt.com" -N "" -f ~/tsk-robot-signing-key
```

The command writes these files:

- `~/tsk-robot-signing-key`, the private key.
- `~/tsk-robot-signing-key.pub`, the public key.

The passphrase is empty because a session cannot type one. Use this key for nothing else.

## 4. Register the public key on GitHub as a signing key

1. Sign in as the robot account.
2. Open Settings, then SSH and GPG keys, then New SSH key.
3. Set Key type to **Signing Key**. Do not choose Authentication Key.
4. Paste the output of `cat ~/tsk-robot-signing-key.pub` into the field labelled Key.
5. Give it a title, for example `tsk robots`.
6. Select Add SSH key.

A signing key cannot push to a repository or read one. It only lets GitHub check
signatures.

## 5. Store the private key in the cloud environment

1. Open claude.ai/code.
2. Open the cloud environment menu in the session's title bar, then Edit.
3. Add an **environment variable** named `TSK_ROBOT_SIGNING_KEY`. Do not use Network secrets
   (API credentials): the network proxy adds a network secret to requests for one host, and
   a session cannot read it through a variable.
4. Print the private key with `cat ~/tsk-robot-signing-key`.
5. Copy the whole output, including the `-----BEGIN OPENSSH PRIVATE KEY-----` and
   `-----END OPENSSH PRIVATE KEY-----` lines, into the value.
6. Save the environment.
7. Delete both signing key files from your machine:

   ```bash
   rm ~/tsk-robot-signing-key ~/tsk-robot-signing-key.pub
   ```

Do not paste the private key into a chat. A session started after you save the environment reads
the variable. A session that was already running does not.

The environment stores variable values in plain text, and every person who can edit the
environment can read them. Every process in a session can read the variable. Revoke the signing key on GitHub (Settings, SSH
and GPG keys, Delete) if you suspect a leak. A revoked key stops new commits from showing
Verified. I did not check what GitHub shows for commits signed before the revocation.

## 6. What a robot session does

Robot attribution runs only when `[attribution] identity = "robot"` is set (see
"Choose the setting" below). With `identity = "default"`, or with no setting, tsk exports no
identity and the cloud environment's identity stays.

When the identity is `robot`, `tsk thread session-start` writes these settings as `export`
lines to the file named by `CLAUDE_ENV_FILE`. The lines set `GIT_AUTHOR_*` and
`GIT_COMMITTER_*`, and `GIT_CONFIG_COUNT` with a `KEY_n` and `VALUE_n` pair for each signing
setting. The command replaces its lines on every source. A robot that registers
(`/tsk:construct-robot` or `tsk actor register`) after the session start gets the identity
at the next source. `tsk actor git-identity` prints the lines and writes nothing. tsk
writes no robot setting to a git config file.

| Key | Value |
|---|---|
| `user.name` | The registered actor name. |
| `user.email` | The `email` setting. This repo sets `352782120+jmdb-robots@users.noreply.github.com`. |
| `gpg.format` | `ssh` |
| `gpg.ssh.program` | `ssh-keygen`. The cloud image sets a platform signer in the global config, which ignores `user.signingkey`. |
| `user.signingkey` | A file that tsk writes from `TSK_ROBOT_SIGNING_KEY`, with mode 0600, outside the repo. |
| `gpg.ssh.allowedSignersFile` | A file that tsk writes with the robot email and the public key of `TSK_ROBOT_SIGNING_KEY`, outside the repo. `git log --format=%G?` prints `G` for a robot commit when it exists. |
| `commit.gpgsign` | `true` |

The exported variables take precedence over the cloud environment's global config, which
names Claude and Anthropic's key. A git command a person runs in their own terminal does not
see them. tsk stores the robot name for ledger commits in the file `tsk-robot-name`
in the worktree git directory. Session start removes a robot set-up that an older tsk wrote
to a worktree config. tsk never prints the private key. The command exits 1 and names
the problem when no actor is registered or `TSK_ROBOT_SIGNING_KEY` is unset. It accepts the
key with real line breaks, or with `\n` in place of them. `tsk actor register` and the session
start do not fail on this: they print a warning, and commits keep the default author.
A human actor gets no change.

## Choose the setting

```toml
[attribution]
identity = "robot"
email = "352782120+jmdb-robots@users.noreply.github.com"
```

- `identity` is `default` or `robot`. Unset means `default`.
- `email` has no built-in value. With `identity = "robot"` and no email, the command exits
  1 and names the missing setting.
- Put the keys in the repo config, `.tsk/config.toml`, to choose for everyone who clones
  the repo.
- Put the keys in the user config, `~/.config/tsk/config.toml`, to choose for one person on
  one machine.
- Pass `--use-robot-attribution <email>` to `ops/cloud/setup-environment.sh` to choose for
  the sessions of a cloud environment. It writes the user config. This repo's cloud
  environment does that.
- Precedence: the variables `TSK_ATTRIBUTION_IDENTITY` and `TSK_ATTRIBUTION_EMAIL`, then
  the repo config, then the user config.
- `tsk config attribution` prints the values and where each came from.
  `tsk config attribution robot --email <address>` writes them to `.tsk/config.toml`, and
  with `--user` to the user config.
- The default identity keeps the `Co-Authored-By: Claude` and `Claude-Session` trailers
  in place.
- `tsk thread session-start`, which the `SessionStart` hook runs, does the set-up before
  the first turn of a session. With `identity = "robot"` it writes the signing key file, the
  allowed signers file, the signing settings and the `commit-msg` hook, and prints
  `Attribution: robot` in the session context. No tool call of the session reads
  `TSK_ROBOT_SIGNING_KEY`. `tsk actor register` then writes `user.name` only.
- The `commit-msg` hook runs `tsk actor commit-msg`. It removes the `Claude-Session` and
  `Co-Authored-By: Claude` trailers and writes `Agent-Session: <session URN>` (ADR 0029).
  It exits 0 when `tsk` is not installed or older than 0.48.0, so a commit never fails because
  of it. tsk does not replace a `commit-msg` hook that it did not write.
- With `identity = "default"`, or with the attribution keys removed from the settings, the next session
  start removes what an earlier robot session wrote: the signing settings, `user.name`,
  the hook and the signing key files.

## Add a co-author trailer

The `co_author` setting adds a `Co-authored-by` trailer to a robot commit in a supervised
session. A supervised session has a human supporting actor, added with
`tsk actor join <uid>`. An autonomous session gets no such trailer. GitHub lists the
person as a co-author of the commit.

```toml
[attribution]
co_author = "Jane Doe <1+jane@users.noreply.github.com>"
```

- The value has the form `Name <email>`. tsk rejects an empty value and a value with no
  `<` and `>`. There is no built-in value.
- Precedence is the same as for `email`: the variable `TSK_ATTRIBUTION_CO_AUTHOR`, then
  the repo config, then the user config.
- `tsk config attribution --co-author "Jane Doe <1+jane@users.noreply.github.com>"`
  writes it to `.tsk/config.toml`, and with `--user` to the user config.
  `tsk config attribution` prints it and its source.
- Pass `--co-author "<Name <email>>"` to `ops/cloud/setup-environment.sh`, with
  `--use-robot-attribution`, to set it for a cloud environment.
- The `commit-msg` hook finds the session from `TSK_SESSION` and queries the ledger for whether a
  human joined it. `tsk actor join` runs after the session start, so the ledger is the
  source. Joining a robot actor does not make a session supervised.
- With `identity = "robot"`, the setting present and a human joined the session, the
  `commit-msg` hook writes `Co-authored-by: Jane Doe <1+jane@users.noreply.github.com>` after the
  `Agent-Session` trailer. It writes the line once and keeps the other trailers. Without
  the setting, or in an autonomous session, the hook adds no such line.

## Ledger commits

With `identity = "robot"`, every commit that `tsk` makes in the ledger has the robot as author and committer (0.56.0). This covers the git commits on the ledger data ref and the documents ref. It also covers the Dolt commits in the ledger store.

- A git commit on the ledger refs has the robot name, the robot email and the SSH signature. GitHub shows it as Verified.
- A Dolt commit has the robot name and email. Dolt has no SSH signing.
- `tsk` reads the identity from the git config of the code worktree, where the session start and `tsk actor register` wrote it. It sets the same identity through `GIT_AUTHOR_*`, `GIT_COMMITTER_*` and `DOLT_AUTHOR_*`, `DOLT_COMMITTER_*`, so a value that the platform sets does not replace it.
- Until the session has a registered robot actor, a ledger commit stops with an error that names `tsk actor register <name> --kind robot`. No commit uses a fallback name. The records stay in the local ledger store and the next commit publishes them.
- With `identity = "default"`, ledger commits are as before.
- The ledger history from before 0.56.0 keeps its author, `tsk <tsk@localhost>` and `Claude`.

## 7. Check a robot commit locally

In a robot session, after the command in step 6 has run:

```bash
git config user.name
git config user.email
git commit --allow-empty -m "Check robot identity"
git log -1 --format='%an <%ae>%n%G?'
```

The first two commands print the robot name and `352782120+jmdb-robots@users.noreply.github.com`. The last prints
the author, then `G` when the signature is good, or `N` when the commit is unsigned.
Remove the check commit before you push it, with `git reset --hard HEAD~1`.

## 8. Check the badge on GitHub

1. Make a robot commit on `main` and push it.
2. Open the commit on GitHub.
3. Check that the author line shows the robot account and the Verified badge.
4. Select the badge. It names the signing key and the verified email.

If the badge reads Unverified, check these in order:

1. The signing key is registered as a **Signing Key** on the same account that has
   `352782120+jmdb-robots@users.noreply.github.com`.
2. The email is Verified in Settings, then Emails.
3. `git log -1 --format=%G?` prints `G` in the session.
4. The private key in `TSK_ROBOT_SIGNING_KEY` matches the public key on GitHub. Compare
   `ssh-keygen -y -f <private key file>` with the public key.

## Sign through an SSH agent

A terminal session on a laptop can sign through an SSH agent, such as the 1Password SSH
agent. No private key is on disk or in an environment variable.

Set these in `[attribution]` to turn it on:

| Setting | Value | Environment variable |
|---|---|---|
| `signing_key` | `agent:<public key>`, for example `agent:ssh-ed25519 AAAA...` | `TSK_ATTRIBUTION_SIGNING_KEY` |
| `signing_program` | The value of `gpg.ssh.program`. Optional, `ssh-keygen` when unset. | `TSK_ATTRIBUTION_SIGNING_PROGRAM` |

Each setting is read from the environment variable, then `.tsk/config.toml`, then the user
config. Write them with these commands, and add `--user` to write the user config:

```bash
tsk config attribution --signing-key "agent:ssh-ed25519 AAAA..."
tsk config attribution --signing-program <path of the signing program>
```

tsk rejects a `signing_key` value that does not start with `agent:` and contain `ssh-`, a key
type and a base64 key. On macOS with 1Password, set `signing_program` to the path of the
`op-ssh-sign` helper that 1Password installs.

When `identity` is `robot` and `signing_key` is set, `tsk thread session-start` does not
read `TSK_ROBOT_SIGNING_KEY` and writes no key file. It writes `gpg.format = ssh`,
`gpg.ssh.program`, `user.signingkey = key::<public key>`, `user.email` and
`commit.gpgsign` as exported variables. When both `signing_key` and `TSK_ROBOT_SIGNING_KEY`
exist, `signing_key` wins. The next session start after a change of mode removes the signing
key file of the other mode.

Git then runs `ssh-keygen -Y sign`, or the program set, with the public key from
`user.signingkey`. The agent signs with the matching private key.

- The 1Password SSH agent offers only keys from the vaults that its `agent.toml` lists. The
  default is the personal vault. A key in another vault must be added to `agent.toml`.
- 1Password requires approval when a key is used. An unattended session blocks until a
  person approves.
- Upload the public key to the robot's GitHub account as a Signing key (step 4).

### The agent socket

A process connects to the SSH agent that its `SSH_AUTH_SOCK` names. The shell of a terminal can name
the launchd agent, which contains no 1Password keys, while 1Password has a separate socket. The
setting `ssh_auth_sock` in `[attribution]` names the socket that signing uses.

```bash
tsk config attribution --user --ssh-auth-sock "<path of the agent socket>"
```

The setting is read from `.tsk/config.toml`, then the user config. On macOS the default is
`$HOME/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock` when that file exists.
Otherwise it is unset and tsk uses the `SSH_AUTH_SOCK` of the process. The git commands that
tsk runs for ledger commits and `tsk signing check` use the setting. A commit that you make
with git yourself uses the `SSH_AUTH_SOCK` of your shell.

### Check that the signing key is available

```bash
tsk signing check
```

The command runs `ssh-add -L` with `SSH_AUTH_SOCK` set to the configured socket. It prints
whether the agent has the public key named by `signing_key`, or what to do: unlock 1Password,
approve the prompt, or turn on the SSH agent. It exits 0 when the agent has that public key and 1
otherwise. `tsk ledger status` shows one `Signing` line with the same answer when
`signing_key` is set. The line times out after 1 second and never fails the status.

### Retry of a failed signature

An agent that is locked or has an approval pending can make git fail with `failed to fill
whole buffer` or `agent returned an error`. A signed ledger commit, rebase or merge that fails with one of
these messages is tried again, up to twice, after a delay of 500 ms. After the third
failure tsk prints an error that names `tsk signing check` and reports that a 1Password
approval may be pending. The environment variable `TSK_SIGNING_RETRY_DELAY_MS` sets the
delay in milliseconds.

## Rotate the signing key

1. Repeat steps 3 and 4 with a new key.
2. Replace the value of `TSK_ROBOT_SIGNING_KEY` (step 5).
3. Delete the old key from the robot account.

New commits use the new key. I did not check what GitHub shows for commits signed with the
old key after you delete it.

## Replace the emails in the ledger history

A ledger commit made before robot attribution has the email of the person. GitHub rejects
a push that publishes a private email address (GH007). Two commands find and replace those
emails in the ledger history. They cover the git history of the data ref and the documents
ref, not the Dolt commits inside the data.

1. `tsk ledger emails` lists each email with the number of commits it authored and committed.
2. `tsk ledger rewrite-emails --to <noreply address>` prints the plan and exits 1. Add `--from
   <email>` to replace named addresses only, and repeat it for several.
3. `tsk ledger rewrite-emails --to <noreply address> --yes` writes the rewritten history to
   local refs of the ledger clone. It signs each rewritten commit with the robot key when the
   identity is `robot`, so run `tsk signing check` first. It pushes nothing. It prints the
   old and new tip of each ref. It prints the clones on this machine that contain the old
   commits. It prints the exact `git push`.
4. `tsk ledger push-rewritten --yes` force-pushes both refs in one atomic push, each with a
   lease on the old tip. When origin is ahead of the old tip, the push fails and changes nothing.

Update tsk in every clone before step 4. An agent stops after step 3 until the person approves
step 4. The old commits stay readable by their ids on a server such as GitHub until it prunes
them.

## Install ssh-keygen in the cloud environment

Git calls `ssh-keygen` to sign a commit. The cloud image does not include it. Add this line
to the setup script of the environment that has `TSK_ROBOT_SIGNING_KEY`:

```bash
command -v ssh-keygen >/dev/null || { apt-get update -qq && apt-get install -y openssh-client; }
```

A fresh container has no package lists, so `apt-get install` alone fails with `has no installation candidate`, and `apt-get update` first removes the failure. A check in a new session confirms the signature with `git log -1 --format='%an <%ae>%n%G?'`.

To see the trailers as well, run `git log -1 --format='%an <%ae> %G?%n%(trailers:only,unfold)'`. A robot commit has the `Agent-Session` trailer.

The platform Stop hook names an unpushed robot commit as Unverified. `~/.claude/AGENTS.md` tells a session to ignore its advice and push.
