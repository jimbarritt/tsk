# Robot commits: author, signature and Verified badge

These steps set up a repo so that a commit made by a robot session names the robot as its
author and shows a Verified badge on GitHub. This is mission M-TSK-ATTRIB.

Steps 1 to 5 are done once, by a person. Step 6 is what each robot session does. Steps 7
and 8 check the result.

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
3. Under Network secrets, add a secret named `TSK_ROBOT_SIGNING_KEY`. If the environment
   offers no such section, add an environment variable of the same name.
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

Every process in a session can read the variable. Revoke the key on GitHub (Settings, SSH
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
| `user.signingkey` | A file that tsk writes from `TSK_ROBOT_SIGNING_KEY`, with mode 0600, outside the repo. |
| `commit.gpgsign` | `true` |

The repo's local config takes precedence over the cloud environment's global config, which
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
  the repo. This repo does that.
- Put the keys in the user config, `~/.config/tsk/config.toml`, to choose for one person on
  one machine.
- Precedence: the variable `TSK_ATTRIBUTION_EMAIL`, then the repo config, then the user
  config.
- `tsk config attribution` prints the values and where each came from.
  `tsk config attribution robot --email <address>` writes them to `.tsk/config.toml`.
- The default identity leaves the `Co-Authored-By: Claude` and `Claude-Session` trailers
  in place. The cloud session adds them (tasks T-09 and T-10).

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
