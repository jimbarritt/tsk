# Report: M-BOOT-06, Mission Control

| Field | Value |
|---|---|
| Mission | M-BOOT-06 |
| Outcome | In progress |
| Actor | Cloud session, Sonnet 5 |
| Date started | 2026-09-25 |

## Outcome

In progress. This report is written as execution proceeds, per the mission model's rule
that a report takes addenda rather than being written only at the end.

## Decisions made during execution

**T-05 key bindings, approved by Jim 2026-09-25/26.** `j`/`k` move the list selection,
`Enter` switches the right pane to the selected session, `n` starts a new session with a
name prompt defaulting to the current git repo name. Vim style, matching the vim
keybindings already recorded for tsk's own TUI in the ledger's legacy backlog.

**T-08 key bindings, approved by Jim 2026-09-25/26.** `e` opens nvim, zoomed, in the
selected session's worktree. `prefix+v` (tmux prefix, `Ctrl-b` by default) restores the
three-pane layout, hiding nvim while it keeps running.

Reasoning for `prefix+v` over the two alternatives considered:
- `prefix+z` was ruled out before it was proposed: it is tmux's own real pane-zoom
  toggle, and nvim in this design runs in a dedicated window, not a zoomed pane, so
  reusing that key would be misleading as well as functionally wrong.
- `prefix+r` was proposed first and rejected. Stock tmux binds `prefix+r` by default to
  `refresh-client` (force a redraw). Scoping the override to the Mission Control
  session only, so `refresh-client` keeps working in every other tmux session on the
  machine, was offered as a way to keep `r`. Jim chose the second option instead: pick a
  letter tmux does not bind by default, so nothing is ever overridden, anywhere.
- `prefix+v` has no default binding in stock tmux, so it introduces no clash and needs
  no session-scoped key table.

**Opus review before starting.** An Opus 5.5 subagent reviewed the plan before work
began, per the mission's instruction to consult one when deciding how to proceed. Its
recommendations, folded into the plan below: pane IDs (`%N`) rather than indices; nvim as
a dedicated tmux window rather than a popup, so it survives independently of the
Mission Control app; state files under
`${XDG_STATE_HOME:-~/.local/state}/tsk-mission-control/`, keyed by tmux session ID; a
poll-based list redraw (`st_mtime_ns` comparison) rather than an OS-specific
file-watching dependency, per the macOS/Linux-parity constraint; and several caveats on
Claude Code hook events for T-06, recorded there rather than here.

**T-01, done.** `jimbarritt/tsk-mission-control` has a README (tmux 3.4, Python 3.9,
Claude Code CLI 2.1.283 as the versions it needs), a `src/tsk_mission_control` package,
and a `pyproject.toml` console-script entry point. Install step: `pipx install
--editable .`. Proven in this sandbox with `--backend pip`, since the sandbox's `pipx`
defaults to a `uv` backend below the version pipx wants; that flag is a sandbox-only
workaround, not part of the documented install step, and does not apply on macOS.

**T-02, done.** One JSON file per Claude session, keyed by tmux pane ID, under
`${XDG_STATE_HOME:-~/.local/state}/tsk-mission-control/<tmux session id>/`. Fields:
`name`, `pane_id`, `worktree`, `claude_session_id`, `transcript_path`, `status`
(`attention`/`reason`/`since`), `tokens_total`, `schema_version`, `updated_at`.
`transcript_path` is not in the briefing's field list for T-02 but the briefing's own
note under T-06 asks for it to be stored, so the field is defined here rather than
reshaping the file format later. Writes lock a sibling `.lock` file and go through a
temp file plus `os.replace`, so a hook can patch one field without a read-modify-write
race against another hook. Covered by four `unittest` tests, no dependency beyond the
standard library.

**T-03, done.** `tsk-mission-control [name]`, run inside an existing tmux session,
splits the invoking pane into the three-pane layout, then `execvp`s into `claude`, so the
invoking pane becomes the first Claude session's pane directly rather than a wrapper
around it. Order: split off the bottom terminal first, while the pane still spans full
width, then split the remaining top region into list (left) and session (right). List
pane runs a placeholder module until T-04. Verified against a real detached tmux 3.4
session in this sandbox, with `claude` actually starting in the right pane and the state
file written correctly.

**Finding: `split-window -p <N>` fails in this sandbox.** `-p` (percentage) fails with
`size missing`, reproducibly, against a freshly created session with an attached client,
tmux 3.4, this sandbox. `-l <N>%` (the documented, more general size argument, which also
takes a percentage) works and was used instead. Not yet checked against macOS's tmux;
T-09 rechecks it there. If macOS's tmux accepts `-p` fine, the cause is this sandbox
specifically and the code stays as `-l <N>%` regardless, since that argument works on
both.

**T-04, done.** A `curses` app in the list pane, showing every session by name with an
attention marker (`○`/`●`), polling the state directory every 0.5s and redrawing only
when a `(name, mtime_ns, size)` signature over its files changes. `curses.wrapper`
handles terminal setup and teardown, including on an exception, closing the gap tsk's
own TUI hit in the legacy backlog (BUG-1: terminal left broken on a panic). Verified
against a real tmux pane: the list renders, and a state file flip (as T-06's hooks will
do) redraws the marker within one poll interval.

Also fixed, found while building T-04: `list_states`' sort was lexicographic on the
filename, putting `%10` before `%2`. Pane IDs now sort on their numeric value.

**T-05, done.** `j`/`k` move the list cursor, `n` starts a new Claude session (name
prompt defaulting to the git repo name), `Enter` switches the right pane to the
highlighted session. Switching uses `swap-pane` to trade the visible session's pane with
the target's, so the one leaving view keeps running rather than being killed. A session
not currently shown lives in a hidden window, `mc-stash`, created on first use. The
visible session's pane is found by position each time (the pane sharing the list pane's
top row), not tracked as a separate "current" ID, so it stays correct even if a pane is
swapped from outside the app.

`entrypoint.py` lets a newly spawned pane register its own state file after creation,
reading its pane ID from `$TMUX_PANE`. This sidesteps an ordering problem T-02's own
design would otherwise hit here: the state file's key is the pane ID, but the pane ID is
only assigned once tmux creates the pane, so the code that spawns a new session's pane
cannot compute that path before spawning it. The pane's own first action, once it
exists, can.

**Finding, in the same shape twice.** The first name-input implementation derived its
input cap from the space left on the pane's row after the prompt, so a long default name
(or a narrow list pane) silently truncated whatever the user typed. Fixing the cap
uncovered a second, deeper version of the same fault: curses' own `getstr` does not
scroll within a line, so it stops accepting input at the pane's right edge regardless of
any cap passed to it. Both were found by typing a real, long name into a real tmux pane,
not by reading the code. Replaced with a manual, scrolling line reader that keeps the
full typed string regardless of what fits on screen.

**T-06, done.** `--settings <json>`, passed on the `claude` command line at launch
(never a settings file, and never `~/.claude/settings.json`), registers one command for
seven hook events, all pointing at `hooks.py`'s own module invocation. `--settings` is
additive, so nothing here touches the user's global settings, and the hooks fire only
for a `claude` process Mission Control itself launched.

Event names and payload fields were confirmed directly against the installed CLI
(`cli.js`, Claude Code 2.1.283), not from documentation, since no public reference lists
them exhaustively: `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`,
`PostToolUse`, `PostToolUseFailure`, `Notification`, `PermissionRequest`,
`SubagentStart`, `SubagentStop`, `PreCompact` all exist as literal event names in this
build. Every payload carries `hook_event_name`, `session_id`, `transcript_path`, `cwd`.
`Notification` carries `notification_type` (`permission_prompt`, `idle_prompt`, among
others). `SessionEnd` carries `reason`, one of `clear`, `logout`, `exit`, `other`,
`prompt_input_exit`, `error`.

**This settles the briefing's own open question on T-06: which event, if any, fires on
an error.** There is no separate event for a mid-turn API error that leaves the session
running; `PostToolUseFailure` exists but is scoped to one tool call failing, not the
turn or the session, and firing "needs attention" on it would fire on routine,
self-recovered tool errors too. What does fire, and cleanly matches "an error" at the
session level the briefing asks for: `SessionEnd` with `reason="error"`, when the whole
session ends abnormally. The other four `SessionEnd` reasons are clean ends and set
nothing.

Mapping: `Notification` and `PermissionRequest` set attention (a prompt needs a look);
`Stop` sets it (a finished turn); `UserPromptSubmit` clears it (submitting is the
natural acknowledgement). `PostToolUse` also clears it, beyond the briefing's four named
states: nothing fires when a human approves a pending permission, checked directly
against the same schema, so without this the indicator would stay lit through the rest
of the turn even after Jim had already looked and approved. The next tool call
completing is what actually signals that.

Verified with unit tests covering every event and transition; the hook logic itself
needs no live `claude` session. Also verified: `claude --settings '<generated json>'
doctor` loads the settings with no hook-schema warnings.

**Not verified end to end, and not this task's fault.** A live run, as done for T-03 and
T-05, hit an unrelated environment problem: this sandbox's shared auth state now asks
even a bare `claude`, no flags at all, to log in again, which it did not a few commits
earlier in this same session. Checked directly: identical wizard from a plain `claude`
with no `--settings` argument, so the change is in the shared container's auth state,
not in anything built here. T-07 through T-09 may need this resolved, or a workaround
(T-07 can likely test against this very session's own transcript file instead of
spawning a fresh one); T-09 runs on Jim's own macOS machine regardless, where this
sandbox's auth state does not apply.

**T-07, done.** `tokens.py` sums the usage records in a session's transcript file.
Confirmed directly against a real transcript, this session's own, not from
documentation: a streamed response writes several JSONL lines for the same assistant
message, each repeating that message's usage totals verbatim, so a message counts once,
by its id. Total is `input + cache_creation_input + cache_read_input + output` tokens
per unique message; thinking tokens are a breakdown within `output_tokens`, not
additional on top.

**Definition recorded, as the briefing's own note under T-06 asked for regarding
`transcript_path`, and as this task's own token definition needs recording too.** The
displayed number runs into the tens of millions for a long session, because a cache
read is counted again on every turn: that is how the API itself bills it, not a bug in
the sum. Numbers are formatted compactly (`1.5k`, `35.2M`) for this reason.

**Checked and ruled out as a shortcut.** A transcript can carry a `cost-state` entry
with running token totals. Checked directly: it is a one-off snapshot, not kept current
to the end of the file (found at line 84 of 1101 in the transcript checked against, with
332 assistant entries by the end). Summing the usage records directly is the only
correct source.

`TranscriptTokenCounter` reads incrementally from a saved byte offset rather than
re-parsing the whole file every poll tick, and stops at the last complete line, so a
transcript caught mid-write is picked up correctly on the next read rather than having
its finishing bytes skipped over.

**Verified against a live, growing transcript, working around the sandbox's auth
problem.** A real `claude` session still cannot be launched here (T-06's finding).
Pointed a hand-written state file at this session's own transcript instead: the
displayed total tracked the transcript's growth over several seconds, unprompted,
confirming the poll loop and the incremental reader both work against a file actively
being appended to, not only against a static fixture.
