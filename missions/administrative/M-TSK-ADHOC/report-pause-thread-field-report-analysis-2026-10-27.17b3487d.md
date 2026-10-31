# Report: analysis of the pause-thread field report (M-TSK-ADHOC T-39)

Date: 2026-10-27. Robot: Plain Statement Of Fact.

Input: `missions/administrative/M-TSK-ADHOC/intel-pause-thread-field-report-2026-10-27.md`, a run of
`/tsk:pause-thread` in java-dojo on plugin 0.3.18 that took 44 seconds.

Evidence classes: **commit** (a commit in `main`), **file** (a file read in the plugin cache of this
session) and **report** (a statement in the field report, not checked here).

## Summary

- Findings 1 and 3 describe plugin 0.3.18. Plugin 0.3.21 (commit c332ae6, 2026-10-24) removed their
  cause. Plugin 0.3.35 is the current release. No change to the skill is needed for them.
- Finding 2 is a fault outside the tsk plugin. No hook of the tsk or swe plugin blocks the word
  "curl". The blocker is unidentified.
- The outcome of the run was correct. The skill did not call a dirty worktree safe.
- Three tasks follow: identify the blocker, take the handover text from outside the command line,
  and measure a pause on the current plugin.

## Finding 1: step 4 repeats git commands

**Status: fixed in plugin 0.3.21.**

- **Commit.** At plugin 0.3.18 (commit e6fb179) the skill had five steps. Step 1 ran
  `tsk thread binding --no-fetch`. Step 4 ran `git status --porcelain`, `git branch -r --contains HEAD`,
  `tsk ledger status --local` and `tsk ledger push`. These match commands 1 and 4 of the report.
- **Commit.** Commit c332ae6 made `tsk thread pause` print three verification lines (`code`,
  `code pushed`, `ledger`) and added `--bound`. The skill has four steps. Step 3 reads the three
  lines and runs no git command.
- **File.** The skill of plugin 0.3.35 asks for one command and no git command on the passing path.
  It runs `tsk ledger push` only when the ledger line reports unpushed changes, and git commands only
  for a dirty or unpushed worktree.

Result: finding 1 is correct for 0.3.18 and closed for 0.3.21 and later.

## Finding 2: the word "curl" blocks the command

**Status: open. The cause is not in the tsk plugin.**

- **Report.** The first call to `tsk thread pause` had the word "curl" in the argument that holds the
  handover text. It was blocked with "fetching and executing remote scripts is blocked". A call with the
  text reworded exited 0.
- **File.** The tsk plugin has no `PreToolUse` hook. Its hooks are `SessionStart`, `Stop` and
  `SessionEnd`.
- **File.** The `PreToolUse` hooks of swe 0.23.1 (`bash-check.sh`, `artifact-check.sh`,
  `mcp-send-check.sh`) contain no match for "curl", "wget" or "remote script". A search of the plugin
  cache for the message text finds nothing.
- **Inference.** The block comes from a place this session cannot read: the user settings or the
  project settings of java-dojo on the Mac, a hook of another plugin there, or the permission
  classifier of Claude Code. A classifier decision appears in the session record as
  `reason_type: classifier`. A rule of that kind matches text in the command line and cannot tell
  a word in an argument from a command.

Consequence for tsk: the handover text is free prose, so it can hold any word a rule matches ("curl",
"wget", "bash", "eval"). A pause fails whenever the text names a download tool, which is likely in a
repo that sets up tools. A retry with reworded text costs one tool call, and the agent has to guess
which word to change.

Options:

1. **Find the blocker.** Then change the rule so that it does not match an argument of `tsk`.
2. **Accept the handover text from stdin or a file.** `tsk thread pause` has no such input today
   (`tsk thread pause --help`: the text is the argument `WHATS_NEXT`). A command line with no free
   prose passes any such rule. The cost is a second way to pass the text, and a write of a file or a
   here-document, which a rule can match too.
3. **Tell the agent in the skill to avoid tool names in the text.** This guards one word and leaves
   the cause.

Decision for Jim: option 1 first, since it needs a look at the settings in java-dojo. Option 2 is
worth building only if the rule stays in place.

## Finding 3: 44 seconds, four tool calls and one blocked attempt

**Status: the cause in tsk is fixed. The time is not re-measured.**

The four calls were command 1 (binding), the blocked attempt, the retry, and command 4 (git and
ledger checks). On plugin 0.3.35 the same pause is one command (`tsk thread pause --bound`) and, when
a line fails, one more for each fix. Commit 989952e (0.22.0) cut the command itself to under 1 second,
so the 44 seconds are the agent turns and the blocked attempt, not `tsk`.

The report does not say how the 44 seconds split between model turns and tool time. No run of the
current skill is timed. A run on 0.3.35 in java-dojo would show the remaining cost.

## Outcome: a dirty worktree

The report ends with 13 uncommitted files and a pause "not confirmed safe for /clear". This is the
correct result under the skill: `code: dirty (13 files)` is a line that differs, so the skill must
not say it is safe. The handover entry and the ledger were pushed, and the uncommitted work stays in
the worktree.

Two points for later, with no task yet:

- The line gives a count and not the names. An agent that must decide what to commit runs
  `git status` anyway. Printing the changed paths when the count is below a limit would remove that call.
- The skill says to commit what belongs pushed. It does not say what to do with files that the
  session did not write. The agent should list them and ask, and not commit them.

## Tasks

- **Identify the blocker of "curl"** in java-dojo (Jim, on the Mac): read the user and project
  settings, the plugin hooks and the permission rules, and name the rule.
- **Take the handover text from stdin or a file** in `tsk thread pause`, if the blocker stays.
- **Measure a pause on plugin 0.3.35** in java-dojo, and record the tool calls and the seconds.

## What the work showed

- A field report names a plugin version. Compare it with the current skill before a change, since
  two of the three findings had been fixed 3 days earlier.
- A rule that matches words in a command line blocks prose arguments. A tool that takes free text
  as an argument is exposed to every rule of this kind.
