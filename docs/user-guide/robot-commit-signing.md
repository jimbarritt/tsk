# Robot commits: author, signature and Verified badge

These steps set up a repo so that a commit made by a robot session names the robot as its
author and shows a Verified badge on GitHub. This is mission M-TSK-ATTRIB.

Steps 1 to 5 are done once, by a person. Step 6 is what each robot session does. Steps 7
and 8 check the result.

## It is optional

The feature is off unless a repo or a user turns it on. With nothing set, tsk writes no git
config, and commits keep the author that the environment gives them.

To turn it on for yourself on one machine, in every repo, write the user config
(`~/.config/tsk/config.toml`). The command needs no repo:

```bash
tsk config attribution robot --email robots@jimbarritt.com --user
```

To turn it on for every session of one cloud environment, pass the robot email to
`ops/cloud/setup-environment.sh` in the environment's setup script, and keep the robot key
in the secret `TSK_ROBOT_SIGNING_KEY`:

```bash
curl -sSfL https://raw.githubusercontent.com/jimbarritt/tsk/main/ops/cloud/setup-environment.sh | bash -s -- --use-robot-attribution robots@jimbarritt.com
```

With the option, the script writes the user config with `--user`. It adds no Claude Code
permission rules. It also writes `~/.claude/AGENTS.md`, imported from `~/.claude/CLAUDE.md`:
the platform's Stop hook names an unpushed robot commit as Unverified and prints commands
that reset the author to Claude, and the file tells a session to ignore them and push the
commit in the same turn. An environment without the option stays off, and a clone
elsewhere stays off. This repo's cloud environment does this. The setup script runs when a
container is built, so a change takes effect in the next new container.
A session with no permission rules confirmed robot signing works unaided (T-15, 2026-11-08).

The variables `TSK_ATTRIBUTION_IDENTITY` and `TSK_ATTRIBUTION_EMAIL` outrank both config
files, for a case that needs an override. A session that was running when they were set
picks them up at its next start, or at once with `tsk actor git-identity`.

To turn it on for everyone who clones a repo, run the command without `--user` at the
top of the repo and commit `.tsk/config.toml`. A repo config outranks the user config, so
nobody can turn it off for themselves in that repo. `tsk config attribution` prints what is
in force and where each value came from.

A robot commit is signed and shows Verified only after steps 1 to 5 are done, and a session
reads the key from the environment variable `TSK_ROBOT_SIGNING_KEY`. Without the key, `tsk`
prints a warning and commits keep the default author. It needs `tsk` 0.37.0 or later.

## What a robot commit looks like

| Field | Value |
|---|---|
| Author name and committer name | The registered actor name, for example `Credit Where It's Due`. |
| Author email and committer email | The `email` setting, the same for every robot in the repo. This repo sets `robots@jimbarritt.com`. |
| Signature | An SSH signature made with one shared robot signing key. |
| Badge | Verified, because the committer email and the signing key belong to the same GitHub account. |

All robots share one email, one GitHub account and one key. The name in the commit tells
you which robot made it. GitHub shows the shared account's avatar and username on every
robot commit, and the robot name in the commit details.

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
account that holds the signing key.

## 3. Generate the signing key

Run this on your own machine, not in a session.

```bash
ssh-keygen -t ed25519 -C "robots@jimbarritt.com" -N "" -f ~/tsk-robot-signing-key
```

The command writes two files:

- `~/tsk-robot-signing-key`, the private key.
- `~/tsk-robot-signing-key.pub`, the public key.

The passphrase is empty because a session cannot type one. Use this key for nothing else.

## 4. Register the public key on GitHub as a signing key

1. Sign in as the robot account.
2. Open Settings, then SSH and GPG keys, then New SSH key.
3. Set Key type to **Signing Key**. Do not choose Authentication Key.
4. Paste the output of `cat ~/tsk-robot-signing-key.pub` into the Key field.
5. Give it a title, for example `tsk robots`.
6. Select Add SSH key.

A signing key cannot push to a repository or read one. It only lets GitHub check
signatures.

## 5. Store the private key in the cloud environment

1. Open claude.ai/code.
2. Open the cloud environment menu in the session's title bar, then Edit.
3. Add an **environment variable** named `TSK_ROBOT_SIGNING_KEY`. Do not use Network secrets
   (API credentials): the network proxy adds a network secret to requests for one host, and
   a session cannot read it as a variable.
4. Print the private key with `cat ~/tsk-robot-signing-key`.
5. Copy the whole output, including the `-----BEGIN OPENSSH PRIVATE KEY-----` and
   `-----END OPENSSH PRIVATE KEY-----` lines, into the value.
6. Save the environment.
7. Delete the key files from your machine:

   ```bash
   rm ~/tsk-robot-signing-key ~/tsk-robot-signing-key.pub
   ```

Do not paste the key into a chat. A session started after you save the environment reads
the variable. A session that was already running does not.

The environment stores variable values in plain text, and every person who can edit the
environment can read them. Every process in a session can read the variable. Revoke the key on GitHub (Settings, SSH
and GPG keys, Delete) if you suspect a leak. A revoked key stops new commits from showing
Verified. I did not check what GitHub shows for commits signed before the revocation.

## 6. What a robot session does

The feature is optional. It runs only when `[attribution] identity = "robot"` is set (see
"Choose the setting" below). With `identity = "default"`, or with no setting, tsk writes no
git config and the cloud environment's identity stays.

When a robot registers (`/tsk:construct-robot` or `tsk actor register`) and the identity is
`robot`, tsk writes these keys to the repo's local git config. `tsk thread session-start`
writes them again when a cloud session starts, because a new container has no local git
config. `tsk actor git-identity` writes them on request.

| Key | Value |
|---|---|
| `user.name` | The registered actor name. |
| `user.email` | The `email` setting. This repo sets `robots@jimbarritt.com`. |
| `gpg.format` | `ssh` |
| `gpg.ssh.program` | `ssh-keygen`. The cloud image sets a platform signer in the global config, which ignores `user.signingkey`. |
| `user.signingkey` | A file that tsk writes from `TSK_ROBOT_SIGNING_KEY`, with mode 0600, outside the repo. |
| `gpg.ssh.allowedSignersFile` | A file that tsk writes with the robot email and the public key of `TSK_ROBOT_SIGNING_KEY`, outside the repo. `git log --format=%G?` prints `G` for a robot commit when it exists. |
| `commit.gpgsign` | `true` |

tsk writes these settings to the worktree's own config file, `config.worktree`, and sets `extensions.worktreeConfig` in the repo config. Each worktree of one clone keeps its own robot name, email and key. The worktree config takes precedence over the cloud environment's global config, which
names Claude and Anthropic's key. tsk never prints the key. The command exits 1 and names
the problem when no actor is registered or `TSK_ROBOT_SIGNING_KEY` is unset. It accepts the
key with real line breaks, or with `\n` in place of them. `tsk actor register` and the session
start do not fail on this: they print a warning, and commits keep the default author.
A human actor gets no change.

## Choose the setting

```toml
[attribution]
identity = "robot"
email = "robots@jimbarritt.com"
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
- The default identity leaves the `Co-Authored-By: Claude` and `Claude-Session` trailers
  in place.
- `tsk thread session-start`, which the `SessionStart` hook runs, does the set-up before
  the first turn of a session (T-15). With `identity = "robot"` it writes the key file, the
  allowed signers file, the signing settings and the `commit-msg` hook, and prints
  `Attribution: robot` in the session context. No tool call of the session reads
  `TSK_ROBOT_SIGNING_KEY`. `tsk actor register` then writes `user.name` only.
- The `commit-msg` hook runs `tsk actor commit-msg`. It removes the `Claude-Session` and
  `Co-Authored-By: Claude` trailers and writes `Agent-Session: <session URN>` (ADR 0029).
  It exits 0 when `tsk` is missing or older than 0.48.0, so a commit never fails because
  of it. tsk does not replace a `commit-msg` hook that it did not write.
- With `identity = "default"`, or with the key removed from the settings, the next session
  start removes what an earlier robot session wrote: the signing settings, `user.name`,
  the hook and the key files.

## 7. Check a robot commit locally

In a robot session, after the command in step 6 has run:

```bash
git config user.name
git config user.email
git commit --allow-empty -m "Check robot identity"
git log -1 --format='%an <%ae>%n%G?'
```

The first two commands print the robot name and `robots@jimbarritt.com`. The last prints
the author, then `G` when the signature is good, or `N` when the commit is unsigned.
Remove the check commit before you push it, with `git reset --hard HEAD~1`.

## 8. Check the badge on GitHub

1. Make a robot commit on `main` and push it.
2. Open the commit on GitHub.
3. Check that the author line shows the robot account and the Verified badge.
4. Select the badge. It names the signing key and the verified email.

If the badge reads Unverified, check these in order:

1. The signing key is registered as a **Signing Key** on the same account that holds
   `robots@jimbarritt.com`.
2. The email is Verified in Settings, then Emails.
3. `git log -1 --format=%G?` prints `G` in the session.
4. The private key in `TSK_ROBOT_SIGNING_KEY` matches the public key on GitHub. Compare
   `ssh-keygen -y -f <private key file>` with the public key.

## Rotate the key

1. Repeat steps 3 and 4 with a new key.
2. Replace the value of `TSK_ROBOT_SIGNING_KEY` (step 5).
3. Delete the old key from the robot account.

New commits use the new key. I did not check what GitHub shows for commits signed with the
old key after you delete it.

## Install ssh-keygen in the cloud environment

Git calls `ssh-keygen` to sign a commit. The cloud image does not include it. Add this line
to the setup script of the environment that holds `TSK_ROBOT_SIGNING_KEY`:

```bash
command -v ssh-keygen >/dev/null || { apt-get update -qq && apt-get install -y openssh-client; }
```

A fresh container has no package lists, so `apt-get install` alone fails with `has no installation candidate`. The `apt-get update` fixes that. A check in a new session confirms the signature with `git log -1 --format='%an <%ae>%n%G?'`.

To see the trailers as well, run `git log -1 --format='%an <%ae> %G?%n%(trailers:only,unfold)'`. A robot commit carries the `Agent-Session` trailer.

The platform Stop hook names an unpushed robot commit as Unverified. `~/.claude/AGENTS.md` tells a session to ignore its advice and push.
