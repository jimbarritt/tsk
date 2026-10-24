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

Record: M-PERF T-18.

### 2026-10-24, entry 10: timings after the ledger was published

Jim ran three timings in `java-dojo`. By now the ledger was on `origin`: the Stop hook had published it (`Background publish: ok at 2026-10-24T09:28:40Z`), and `git ls-remote origin` lists `refs/tsk/ledger` and `refs/tsk/docs`, so this repo uses custom refs, as a laptop repo does by default. Output, exactly:

```
$ time git ls-remote origin
05a8f80ca1c952e75d9c5a57e9c6fe0424c1ce5c        HEAD
05a8f80ca1c952e75d9c5a57e9c6fe0424c1ce5c        refs/heads/main
ea471e4821a17294338285510cf2fec75573c937        refs/tsk/docs
fb26c93661e72923e6ee5fa689dc63d9570e8dc4        refs/tsk/ledger
git ls-remote origin  0.03s user 0.04s system 4% cpu 1.603 total
$ time tsk ledger status --local
Local   nothing to push
Origin  not checked (--local)
Server  not running (switch off)
Background publish: ok at 2026-10-24T09:28:40Z
tsk ledger status --local  0.32s user 0.40s system 37% cpu 1.938 total
$ time tsk ledger status
Local   nothing to push
Origin  up to date
Server  not running (switch off)
Background publish: ok at 2026-10-24T09:28:40Z
tsk ledger status  0.36s user 0.35s system 26% cpu 2.652 total
```

Reading:

- One round trip to GitHub costs 1.6 s on this laptop (`git ls-remote`, 0.07 s of CPU).
- `tsk ledger status` against a published ledger takes 2.65 s, against 6.8 s when the ledger was unpublished (entry 9). So the unpublished state costs about four extra seconds. This supports the suspected cause in M-PERF T-18: nothing is cached for a ledger that origin does not have yet.
- `tsk ledger status --local` makes no network call and still takes 1.94 s, with 0.72 s of CPU. About 1.2 s is spent waiting on something local. With the Dolt server off, each command starts `dolt` processes, and a start can be slow on macOS (for example when the system scans each new process). This is a guess. The Dolt server (M-PERF T-19) avoids those starts. Next measurement: enable the server and time `--local` twice.
- The Stop hook publish worked on the first real use on a laptop.

Records: M-PERF T-18 (unpublished ledger cost) and T-19 (server on by default).

### 2026-10-24, entry 11: timings with the Dolt server on

Jim set `TSK_DOLT_SERVER=on` and ran the same commands. Output, exactly:

```
$ time tsk ledger status --local    # starts the server
Local   nothing to push
Origin  not checked (--local)
Server  running (pid 35472, idle stop 1800 s)
Background publish: ok at 2026-10-24T09:31:49Z
tsk ledger status --local  0.23s user 0.25s system 27% cpu 1.737 total
$ time tsk ledger status --local    # server already running
tsk ledger status --local  0.22s user 0.16s system 42% cpu 0.894 total
$ time tsk ledger status
Origin  up to date
tsk ledger status  0.29s user 0.25s system 22% cpu 2.369 total
```

Comparison with entry 10 (server off):

| Command | Server off | Server on |
|---|---|---|
| `ledger status --local`, first run | 1.94 s | 1.74 s (starts the server) |
| `ledger status --local`, later run | 1.94 s | 0.89 s |
| `ledger status` (one `ls-remote`) | 2.65 s | 2.37 s |

Reading: the server saves about 1 s on a local command and about 0.3 s on a command that asks `origin`. It does not remove the local wait: 0.89 s elapsed against 0.38 s of CPU leaves about 0.5 s of waiting with no network call. On the cloud container the same command takes 0.1 s, so the extra time is specific to this laptop. Next measurement: `time tsk --version`, which starts the binary and touches no ledger. If it takes 0.3 s or more, the cost is process start on this machine and not the ledger code.

The server supports M-PERF T-19 (on by default). The remaining local wait goes on M-PERF T-18.

### 2026-10-24, entry 12: starting the binary is fast, a ledger command is not

Jim ran `time tsk --version`. Output, exactly:

```
$ time tsk --version
tsk 0.26.0
tsk --version  0.00s user 0.00s system 51% cpu 0.008 total
```

Starting `tsk` takes 8 ms, so the 0.5 s wait in `tsk ledger status --local` (entry 11) is inside the ledger command.

Trace in the cloud container (not Jim's laptop): `strace -f -e trace=execve` shows `tsk ledger status --local` starting `git` six times, with no network call. Here each start is cheap and the command takes 25 ms.

Hypothesis: on macOS `/usr/bin/git` is a shim that looks up the real `git` through `xcode-select` on every call, so six starts cost about 0.5 s. Test for Jim: `which -a git` and `time git rev-parse --git-common-dir`, and the same with `/opt/homebrew/bin/git` if it exists. Result still to come.

Record: M-PERF T-20.

### 2026-10-24, entry 13: a git start takes 40 to 60 ms on this laptop

Jim ran the `git` timings inside `java-dojo`. Output, exactly:

```
$ which -a git
/usr/bin/git
$ time git rev-parse --git-common-dir
.git
git rev-parse --git-common-dir  0.01s user 0.01s system 38% cpu 0.063 total
$ time git rev-parse --git-common-dir
.git
git rev-parse --git-common-dir  0.01s user 0.01s system 51% cpu 0.040 total
$ ls /opt/homebrew/bin/git
"/opt/homebrew/bin/git": No such file or directory (os error 2)
```

Reading: the only `git` is Apple's `/usr/bin/git`, and a start takes 40 to 63 ms for a command that does almost no work (0.02 s of CPU). The container takes a few milliseconds. Six `git` starts in `tsk ledger status --local` (entry 12) then cost about 0.25 to 0.4 s, which explains most but not all of the 0.5 s wait. The rest may be more `git` starts on a custom-refs repo than the six counted in the container, or the server socket.

Not tested: whether Homebrew `git` starts faster than the Apple shim.

Record: M-PERF T-20 (updated with the measurement).

### 2026-10-24, entry 14: the agent set up its own mission in java-dojo

Jim asked an agent in `java-dojo` to store the Revolut interview brief (distilled in this session from Jim's email) as an intel document on a mission. Jim reported that the agent did the whole job: mission, document, link and push. Jim then ran `tsk mission list`. Output, exactly:

```
$ time tsk mission list
M-DOJO-ENV  todo  -  -  Bootstrap a Java practice environment
tsk mission list  0.21s user 0.14s system 36% cpu 0.946 total
```

Readings:

- The ledger has one mission, `M-DOJO-ENV` "Bootstrap a Java practice environment", with status `todo`. No separate interview mission is listed, so the brief is probably linked to `M-DOJO-ENV`. Not yet checked.
- The status is `todo` while work on the mission is under way.
- `tsk mission list` takes 0.95 s with 0.35 s of CPU on a one-mission ledger, so about 0.6 s is waiting. This matches the git start cost in M-PERF T-20 (entry 13).

Open: which mission holds the brief, and whether the agent used `tsk` for all checks this time.

### 2026-10-24, entry 15: installing the swe plugin

Jim asked for the command to install the swe plugin, then whether installing inside a Claude session defaults to project scope.

Facts checked in this session: `claude plugin install --help` prints `-s, --scope <scope>  Installation scope: user, project, or local (default: "user")`. So `claude plugin install` on its own installs for every repo on the machine. `tsk install-plugin` passes `--scope project` explicitly. `tsk install-plugin claude-cli --plugin tsk --plugin swe` installs both at project scope. Whether the `/plugin` menu inside a session asks for a scope was not tested.

Jim's request: put this in the user guide that `tsk --help` becomes, as an optional instruction.

Record: M-ADMIN-02 T-09 (reworded to include the optional swe step and the scope default).

### 2026-10-24, entry 16: plugin list on the laptop

Jim ran `claude plugin list` in `java-dojo` and saw `swe` twice and `tsk` twice, mostly "disabled". `claude plugin list --json` explained it. Each entry has a `projectPath`, and the list covers every project, not only the current one. Relevant entries, from Jim's output:

| Plugin | Version | Scope | projectPath | Enabled here |
|---|---|---|---|---|
| swe | 0.12.1 | local | `/Users/jmdb/Code/github/jimbarritt/marq` | no |
| swe | 0.16.4 | project | `/Users/jmdb/Code/github/jimbarritt/tsk` | no |
| tsk | 0.1.4 | project | `/Users/jmdb/Code/github/jimbarritt/tsk` | yes (`projectEnabled` true) |
| tsk | 0.3.16 | project | `/Users/jmdb/Code/github/jimbarritt/java-dojo` | yes |

Readings:

- `java-dojo` has `tsk` 0.3.16 and no `swe` at all. The two `swe` entries belong to the `marq` and `tsk` clones, and they are old (the current `swe` is 0.21.0).
- The `tsk` clone on the laptop still has `tsk` plugin 0.1.4, installed on 2026-10-06. That plugin has not been updated for 18 days. Expected: 0.3.16.
- Three LSP plugins at user scope (kotlin, rust-analyzer, swift) are disabled and unrelated.

Fix for `java-dojo`: `tsk install-plugin claude-cli --plugin tsk --plugin swe`, then restart `claude`.

Finding for `tsk doctor` (M-ADMIN-02 T-12): the plugin check lists the entry for this repo's path and flags entries older than the binary expects, with the path of each.

### 2026-10-24, entry 17: a thread pause took 24 seconds

Jim's words: "It took 24s to pause the thread note that it will be useful to comapre to the pasue plan."

The pause plan is `docs/arch/2026-10-20-session-start-and-pause.md` and ADR 0019 (amended for the synchronous pause). Its budget for `thread pause` is 2 s. The measured pause in the cloud container was 1.42 s (0.88 s after the 0.22.0 work), with two to three git network round trips. On Jim's laptop one round trip to GitHub costs 1.6 s (entry 10), so a pause with a push is expected to take 3 to 6 s there. 24 s is about five times that.

Not known yet: whether the 24 s is `tsk thread pause` alone or the `/tsk:pause-thread` skill, which includes the agent's own time to write the entry; whether the Dolt server was on; which repo and ledger state (published ledger, custom refs). Needed from Jim: `time tsk thread pause <thread> <T-nn> "<text>"` run directly, with the server on and then off.

Record: M-PERF T-21.

### 2026-10-24, entry 18: session start in java-dojo took 23 seconds

Jim started `claude` in `java-dojo` and typed "hi". What the terminal showed, as pasted (some lines were cut or overlapped in the paste):

```
Ran 1 shell command
Skill(tsk:resume-thread)
Successfully loaded skill
Ran 2 shell commands
Hi. This session is now bound to thread 5a33b6e5.
thread id: 5a33b6e5
we are working on mission Bootstrap a Java practice environment (M-DOJO-ENV)
this is where we are at: The mission has one task, T-01, which bootstraps the real interview Java project ... Main.java has uncommitted ch...
Do you want to continue with this thread?
SWE: Verified
Sautéed for 23s
```

Readings:

- The agent ran one shell command, loaded the `tsk:resume-thread` skill, ran two more shell commands, and then wrote the summary. The whole turn took 23 s.
- tsk's own session start takes under 1 s on this machine (the hook ran before the first prompt). So the 23 s is the agent's turns: model time and tool calls.
- The bound thread and the mission came out right, and the swe plugin is now active ("SWE: Verified").
- The same shape is the likely cause of the 24 s pause in entry 17: `/tsk:pause-thread` is a skill, so its time includes the agent writing the entry. To confirm, Jim runs `tsk thread pause` directly.

Records: M-PERF T-21 (hypothesis added) and T-22 (cut the agent's turns at session start and pause).

### 2026-10-24, entry 19: bug, tsk ledger push fails with a git pre-commit hook

Reported by the agent in `java-dojo` and relayed by Jim. Environment: tsk 0.26.0, tsk plugin 0.3.18, swe plugin 0.22.0, `/swe:install-commit-hook` run earlier the same day (it wrote `.git/hooks/pre-commit`, version 2).

What happened: `tsk ledger push "<message>"` stopped before it pushed anything. The swe hook blocked the commit of two staged documents with "commit blocked. 2 staged markdown file(s) have no clean lint record for their staged content", and it listed `revolut-build-it-brief.md` and `revolut-recruiter-call-transcript.md` under `missions/operational/M-DOJO-ENV/`. The background publish (Stop hook) failed the same way at 10:52:41Z and 10:54:04Z. The next-command warning read: `warning: the last background publish failed at 2026-10-24T10:52:41Z: git commit --quiet -m Background publish failed (exit status 1):; run tsk ledger push`. The text after "exit status 1:" is empty, so the reason is lost.

Cause (verified by the agent): the documents worktree (`~/.local/state/tsk/repos/java-dojo-fe0769ec/docs`) is a linked worktree of the code repo. `git rev-parse --git-dir` gives `<repo>/.git/worktrees/docs` and `--git-common-dir` gives `<repo>/.git`, with `core.hooksPath` unset. A linked worktree uses the common directory's hooks, so the code repo's `pre-commit` runs when tsk commits documents on the `tsk/docs` ref. The documents commit is the first step of the push, so documents and Dolt data both stayed unpushed.

State left: documents staged locally (one modified, one added); data has local changes (1 decision, 2 constraints, 1 mission document link, 1 mission row) and the remote is ahead by 2. Nothing is lost.

Fix options from the report: 1) tsk runs its own commits with hooks disabled; 2) a worktree-specific `core.hooksPath` for the documents worktree; 3) the background publish warning includes the commit's stderr. Chosen: 1 and 3, released as 0.26.1 (below).

Interim for Jim: lint the two documents, or list them in `.swe-ignore`, so the hook passes; or switch the hook off for the push.

Record: M-ADMIN-02 T-21.
