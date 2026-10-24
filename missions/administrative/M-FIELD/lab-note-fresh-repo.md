# Lab note: tsk in a fresh repo

Mission: M-FIELD. Tester: Jim, on a laptop, in a repo that has no ledger yet. The latest release at the start was 0.26.0. The version Jim installed is not yet known: see entry 2.

Each entry has a date, what Jim did, what happened, and what he expected. Jim's wording is kept as he wrote it. Findings that need action are copied to a record, and the entry names it.

## Setup

- Install: `brew install jimbarritt/tap/tsk`, then `tsk install-plugin claude-cli`, then `claude` (README, Install).
- Questions to answer: did the install need anything the README does not say, how long did the first session start take, did the ledger create on the first push.

## What the repo is

Jim is building a training environment to practise writing Java by hand for a coding interview. The repo is that environment. It is a new repo with no ledger.

## Entries

### 2026-10-24, entry 1: tsk --help

Jim ran `brew install jimbarritt/tap/tsk` in a fresh terminal, then `tsk --help`.

Jim's finding: "tsk --help - it should list install and in the usage section there should be a statement about installing it in a repo."

Expected: the help lists the install command and says how to install tsk in a repo.

Record: M-ADMIN-02 T-09.

### 2026-10-24, entry 2: tsk ledger status

Jim ran `tsk ledger status`. Output, exactly:

```
$ tsk ledger status
error: unrecognized subcommand 'status'
```

Expected: the ledger status. The 0.26.0 source has `tsk ledger status`, so the installed binary is probably older than the latest release. Open: the output of `tsk --version` on the laptop, and whether the Homebrew formula follows each release.

Record: M-ADMIN-02 T-10.

### 2026-10-24, entry 3: an old tsk shadowed the Homebrew install

Jim ran `brew install jimbarritt/tap/tsk`. `tsk --version` then printed 0.2.1, and Jim thought the Homebrew formula was out of date. The tap has `Formula/tsk.rb` at 0.26.0, so the formula was current.

Jim ran `which -a tsk`. Output, exactly:

```
/Users/jmdb/.cargo/bin/tsk
/Users/jmdb/.local/bin/tsk
/opt/homebrew/bin/tsk
```

The cargo copy runs first and shadows the Homebrew copy. A `sudo xcodebuild -license` prompt also appeared during the brew steps (the Xcode licence was not accepted), which may have interrupted the install or the update.

Cause: a stale copy from an earlier install. Fix for Jim: remove the cargo and `~/.local/bin` copies, run `hash -r`, check `tsk --version`.

Findings: a new user with an old `tsk` meets error messages from the old version (`unrecognized subcommand 'status'`, `runs without tskd` in the old help) that look like defects in the new one. Entries 1 and 2 are from 0.2.1 until Jim shows otherwise; recheck both on 0.26.0.

Record: M-ADMIN-02 T-11. T-09 and T-10 stay open until rechecked on 0.26.0.

### 2026-10-24, entry 4: 0.26.0 installed, the help is too long

After Jim removed the cargo and `~/.local/bin` copies, `which -a tsk` printed only `/opt/homebrew/bin/tsk` and `tsk --version` printed `tsk 0.26.0`.

Jim's finding on the 0.26.0 help: "ok yes now the help is fully there but its a bit overhwleming. can we have it more in the form of a user guide?"

Expected: a short guide that says what tsk is, how to start, and the few commands a person uses first, with the rest one step further away.

Jim also asked for a check of the environment, so that a shadowed `tsk`, a missing `dolt` or an unaccepted Xcode licence shows up in one place.

Records: M-ADMIN-02 T-09 (reworded to a user-guide help) and T-12 (`tsk doctor`). The shadowing finding in entry 3 is T-11, the first check of the doctor.

### 2026-10-24, entry 5: install-plugin does not say it works on a repo

Jim's finding: "`install-plugin` its not really explaining that it needs to be installed in a repo."

Expected: the help and the output say that the command installs into the repo it runs in, and what it changes there.

Record: M-ADMIN-02 T-13.

### 2026-10-24, entry 6: install-plugin with no target, and outside a repo

Jim ran `tsk install-plugin` with no argument. Output, exactly:

```
$ tsk install-plugin
error: the following required arguments were not provided:
  <TARGET>
```

Jim's finding: "it would be good if this told you what the available options are. Also install-plugin should have the check to see if we are in a git repo or not first. if not it should warn us and ask us if we want to install in global claude config. I'm not even sure if this is a valid configuration?"

Facts checked in this session (0.26.0):

- `--scope user` exists for claude-cli. `tsk install-plugin` skips the SessionStart hook at user scope: "skipped the SessionStart hook: a user-scope install is not about this repo". So the code allows it.
- tsk needs a git repo. In a directory with no repo, `tsk mission list` stops with "not inside a git repository". The plugin's session start hook exits 0 and adds the warning "the ledger could not be fetched automatically at session start" to the context.
- So a user-scope plugin is a valid Claude Code setting, but with it every session outside a tsk repo starts with that warning. Whether user scope should be offered is a decision for Jim.

Records: M-ADMIN-02 T-14 (list the targets in the error) and T-15 (check for a git repo first, and offer user scope or stop).

### 2026-10-24, entry 7: install-plugin on the Java repo

The repo is `java-dojo`, a Java 25 Gradle project made in IntelliJ (see the intel document `intel-setting-up-a-java-project-from-scratch-with-intellij.md`). Jim ran `tsk install-plugin claude-cli` inside it. Output, exactly:

```
$ tsk install-plugin claude-cli
add the jimbarritt/claude-plugins marketplace: claude plugin marketplace add jimbarritt/claude-plugins
install tsk@jimbarritt-claude-plugins at project scope: claude plugin install tsk@jimbarritt-claude-plugins --scope project -y
refresh the jimbarritt-claude-plugins marketplace: claude plugin marketplace update jimbarritt-claude-plugins
update tsk@jimbarritt-claude-plugins at project scope: claude plugin update tsk@jimbarritt-claude-plugins --scope project
wrote the SessionStart hook to .claude/settings.json
commit .claude/settings.json so cloud sessions install the plugin
```

What worked: the command ran to the end with no error.

Findings:

- The output names the four commands it runs. It does not show whether each one worked, or what they printed.
- It does not say what to do next: start `claude`, and that the ledger is created on the first push.
- The last line says to commit `.claude/settings.json` "so cloud sessions install the plugin". Jim works on a laptop. The line does not say that committing is only needed for cloud sessions.
- This session expected `.tsk/config.toml` to be written as well. `git status` afterwards showed one untracked entry, `.claude/`, and no `.tsk/`. So the command writes only under `.claude/`. The expectation was wrong, and the output was right not to mention `.tsk/config.toml`.

Record: M-ADMIN-02 T-17.

### 2026-10-24, entry 8: an agent checked the ledger with ls and head

Jim started a Claude Code session in `java-dojo` and asked whether the ledger was initialised. The agent listed files and ran `git status` on the ledger store, a Dolt directory, to look for the ledger. It also ran `tsk mission list` and `tsk thread list` with every command piped through `head`.

The agent's own account, as Jim pasted it:

"I also inspected the ledger through the filesystem with ls and a git status on a Dolt directory. The git status was meaningless there. I should have used tsk for all of it."

"I piped every command through head, so I never saw an exit code. 'Run without error' was not verified. The empty mission list could also mean the command printed nothing."

Reproduction in a scratch repo on 0.26.0 (this session): after `tsk ledger fetch` in a repo whose `origin` has no ledger yet, `tsk mission list`, `tsk question list` and `tsk decision list` print nothing and exit 0. `tsk ledger status` prints `Local   nothing to push`, `Origin  not published yet (tsk ledger push creates it)` and a `Server` line, and exits 0. So the right answer to "is the ledger initialised" was `tsk ledger status`, and the empty list is ambiguous: it is the same output as a failure that prints nothing.

Why the agent did not use it: an agent in another repo sees the plugin skill and the session start context, not `README-AGENT.md`. Neither tells it how to check the ledger or to avoid `head`.

Records: M-ADMIN-02 T-18 (agent rules in the skill and session start context), T-19 (an empty list says so), T-20 (a Ledger block in `tsk ledger status`).

### 2026-10-24, entry 9: tsk ledger status takes 6.8 seconds

Jim ran `time tsk ledger status` in `java-dojo`, which has no ledger on `origin` yet. Output, exactly:

```
$ time tsk ledger status
Local   nothing to push
Origin  not published yet (tsk ledger push creates it)
Server  not running (switch off)
tsk ledger status  0.49s user 0.55s system 15% cpu 6.775 total
```

Jim's words: "wow this is sloooow".

Reading of the numbers: 6.8 s elapsed against 1.0 s of CPU, so the command waited about 5.7 s, most likely on network round trips to GitHub.

Suspected cause, from the source and not yet measured: when origin has no ledger, nothing is cached (discovery of the ledger refs caches only a found ledger). Each command then repeats the ref discovery `git ls-remote` and the tips `git ls-remote`, and may run a fetch. Each is a full connection to GitHub. A published ledger records the tips and skips most of this (0.22.0), so the cost may be specific to the unpublished state.

To measure on Jim's laptop: `time git ls-remote origin` (one round trip), `time tsk ledger status --local` (no network), and `GIT_TRACE=1 tsk ledger status 2>&1 | grep -c "git"` (how many git calls).

Record: M-PERF T-23.
