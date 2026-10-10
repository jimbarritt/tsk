# Rex, by Superlogical

Status: public beta from 2026-10-05, macOS application only, by invitation from the
mailing list. Server version 0.1.0 appears in the docs. Free: "Rex is and will always
be free." No account, self-hostable, and "We do not see or share your data." The
commercial plan is not published. Read 2026-10-09.

Raised by Jim, 2026-10-09, from a post by Steve Yegge on 2026-10-08: "Rex replaces
three layers of ancient terminal emulation with a lightning-fast, modern terminal
emulator that can replace ssh/mosh, ghostty, and tmux". Yegge runs "Emacs with 20+
agents" in it, and says it "solves a problem I didn't realize even had a solution:
agents going through five to six layers of terminals and shells."

Sources: the [Superlogical](https://www.superlogical.com/) home page, the update
[Public testing is beginning](https://www.superlogical.com/updates/public-testing-beginning),
Mitchell Hashimoto's post [Superlogical](https://mitchellh.com/writing/superlogical)
of 2026-07-29, and the Rex documentation pages under `superlogical.com/rex/docs/`:
Installation, The CLI, Lua Scripts, Lua Events, Server API, Shell Scripting, Events,
and the Program Status Protocol. Every page was fetched as raw HTML and read as text.
Quoted phrases were matched against that text. Claims are Superlogical's own and were
not run. The application was not installed: testing is by invitation.

## Who

Superlogical was announced on 2026-07-29 by Mitchell Hashimoto, creator of Ghostty and
co-founder of HashiCorp, with Jack Pearkes, Alasdair Monk and Hector Simpson, all
formerly of HashiCorp. Funded by Notable Capital, Amplify Partners and angels including
Patrick Collison, Tobias Lütke, Guillermo Rauch and Armon Dadgar. Rex builds on
libghostty, the MIT-licensed terminal library from the Ghostty non-profit.

## What it is

Rex is a terminal multiplexer that is also the terminal: "a drop-in replacement for
whichever terminal you use today". A server keeps sessions alive. Clients attach to
them: today the macOS application, which "self-hosts its own Rex server and it is able
to connect to other Rex servers". Servers run on Linux and Windows, and clients for
iOS, Linux and Windows are in progress. Sessions can be reached through the web and
shared live with other people.

The company's statement is broader than a multiplexer. "We believe the missing layer
is a durable session around the work itself: one that can span applications and
environments, provide relevant context by default, expose structured data and actions,
preserve history, and be driven by software while remaining visible and controllable
by people." The plan has three parts: build the multiplexer, make everything in it
composable, make it safe and operable in production.

## Built with

- **The server's language is not published.** No page read names it.
- **The client runs libghostty**, per a secondary write-up. Each client parses the
  server's raw terminal bytes itself. libghostty is the terminal library from Ghostty:
  "a cross-platform, zero-dependency C and Zig library", MIT licence. Its core is Zig,
  about 350,000 lines in 599 files in the Ghostty repository read on 2026-10-10. Only
  `libghostty-vt`, which parses escape sequences and keeps terminal state, is released.
  The Ghostty README calls its API signatures "still in flux".
- **Scripting is Lua 5.1**, "with the full standard library, including io, os, and
  require". The docs do not say whether it is stock Lua or LuaJIT. A script runs where the
  `rex` CLI runs and calls the server's API, so it is a client of the server, not code
  inside it.
- **Ghostty** began as Hashimoto's personal project. In 2025 he donated it to a non-profit,
  and about a dozen core maintainers now work on it.

## On one machine

The macOS client "is fully self-contained with a local server", and "self-hosts its own
Rex server". Every Rex terminal gets `REX_SERVER`, `REX_SESSION` and `REX_BLOCK` in its
environment, so the `rex` CLI knows its place. The docs' listing of connected clients
shows the app as principal `uid 501`, "over unix". The CLI is one more client:
`rex -S <host> ls` points the same command at another server. tmux is also client and
server on one machine. The difference is what a client is. A tmux client draws tmux's
text inside a terminal, and a Rex client is the terminal.

## Openness

Superlogical states: "Rex is and will always be free", no account, self-hostable, "We do
not see or share your data", and "the Rex clients and servers themselves are not going to
be directly monetized". The commercial plan is "not ready to share". The mailing list
promises "any OSS releases along the way". No source for the server or the client is
published. The OSC 7501 protocol is public and carries "no Superlogical-specific
functionality or language".

## Model

| Concept | Meaning |
|---|---|
| Session | "a group of related work, such as one project. The server keeps a session running whether or not anything is looking at it." |
| Window | A tab within a session. It holds a layout of blocks |
| Block | "one application in a window. Today that means a terminal." |
| Client | A program connected to the server, such as the app |

## Automation

The `rex` command is on `PATH` in every Rex terminal, and already knows which session
and block it runs in.

| Surface | Mechanism |
|---|---|
| Server API | Self-documenting. `rex api list` prints every method, `rex api describe <method>` prints its JSON Schema for input and output with examples, and `rex api call` calls it. "The CLI and Lua APIs all derive dynamically from the server API" |
| Lua scripts | `rex do <script.lua>` runs a script with the full Lua standard library. It can create sessions, windows and splits, start a command in a block, read a terminal's screen, and return JSON |
| Waiting | `rex.wait(event, filter, timeout)` pauses until an event arrives, for example `terminal.child_exited` for one block |
| Event handlers | `rex.on(event, fn)` keeps a script running and calls it per event, for one session or `--all` |
| Events on the CLI | `rex events --json` prints one JSON object per line |
| Events | Session events (`block_created`, `client_connected`, `session_view_changed`), terminal events (`child_exited`, `pwd_changed`, `bell`, `process_changed`, `desktop_notification`, `progress_report`, `program_status_changed`), and server events (`session_created`, `tailscale_status_changed`) |

The docs' example runs a test command in a split, waits for it to exit, and zooms the
split if it fails.

## Program Status Protocol (OSC 7501)

A terminal escape sequence "that lets a program tell the terminal what it is doing:
idle, working, waiting on the user, finished, or failed, and why." Superlogical
publishes it as "a generic terminal protocol" with "no Superlogical-specific
functionality or language".

| Key | Values |
|---|---|
| `state` | `idle`, `working`, `done`, `blocked`, `error`, or `clear`. Required |
| `kind` | With `blocked`: `permission`, `question` or `auth` |
| `progress` | 0 to 100, with `working` or `blocked` |
| `app` | A stable name such as `cargo`, `terraform` or `claude-code` |
| `id` | A path such as `build/test`, so one program can report several records, parent and child |
| `title`, `msg` | Base64 UTF-8 text, one line |

A `working` or `blocked` record is dropped when the process exits or a new shell prompt
begins. `done` and `error` survive both, "so a record is left behind for the user to
find". Rex shows an indicator in the session picker for sessions with `done` or
`blocked` records, a spinner or symbol in unfocused tab headers, and emits
`terminal.program_status_changed` to Lua. Planned: push notifications to the mobile
applications and "a unified priority inbox view".

The motivation names the alternative: tools that show many programs "fall back to
heuristics such as parsing window titles or matching screen contents, or to
program-specific plugins". That is how Herdr reads agent state.

## Compared with tmux and Herdr

| Aspect | tmux | Herdr | Rex |
|---|---|---|---|
| Where it runs | Inside a terminal emulator | Inside a terminal emulator | It is the terminal emulator, on libghostty |
| Persistence | A server holds sessions across detach | The same, plus layout restore and agent resume after a restart | A server holds sessions; close the app and reconnect from another device |
| Agent state | None | Read from the screen by a detection manifest per agent, or reported by an integration | Reported by the program through OSC 7501. A program that does not report has no state |
| Blocked state | None | Only when the screen matches a known prompt | `blocked` with `kind=permission`, `question` or `auth`, from the program |
| Scripting | `tmux` commands and format strings | A CLI and socket API that print JSON | A self-documenting JSON Schema API, Lua scripts with `rex.wait` and `rex.on`, and an events stream |
| Remote | ssh to the host, then tmux | Saved SSH machines in one window | Connect to other Rex servers from the app. Tailscale status is a server event |
| Sharing | Attach the same session | Direct attach, with `--takeover` for input | "sharing a live session with other people is built in from the start" |
| Licence | ISC, open source | Apache 2.0, open source | Free and self-hostable. No source published on the pages read |
| Platforms | Unix | macOS, Linux, Windows | macOS client today. Servers on Linux and Windows. iOS, Linux and Windows clients in progress |

Rex and Herdr take opposite positions on how a terminal learns an agent's state. Herdr
reads the screen and keeps a manifest per agent, updated from herdr.dev. Rex publishes
a protocol and waits for programs to adopt it. Until Claude Code and the others emit
OSC 7501, Rex shows them as plain terminals.

## Comparison with the Mission Control task

| Mission Control objective | Rex |
|---|---|
| A list of Claude sessions, the control list | The session picker, with an indicator for `done` and `blocked` |
| One status indicator for "needs attention" | `blocked` and `done` from OSC 7501, and `terminal.bell`, `terminal.desktop_notification` events |
| Hooks write status to a state file per session | A Lua script on `terminal.program_status_changed` writes wherever it likes. Claude Code would need to emit OSC 7501, or a hook would emit it on Claude Code's behalf |
| Token total per session | Not in the protocol. `progress` is 0 to 100 |
| Sessions survive a detach; phase two resumes after a restart | Sessions survive detach. Restart behaviour is not documented on the pages read |
| Local only | Local, and other Rex servers |

## Comparison with tsk

| Aspect | Rex | tsk |
|---|---|---|
| Unit | A session: "a group of related work, such as one project", held by the server | A Mission, delegated with a briefing |
| State recorded | Sessions, windows, blocks and program status records, on one server | Missions, threads and continuation entries, in a ledger |
| "A durable session around the work" | The terminal session, with history, structured data and actions. Stated as the company's aim | Thread continuation: an append-only entry any later actor reads, on any machine |
| Agent state | A program reports `idle`, `working`, `blocked`, `done` or `error` | A mission report: done, failed or blocked. A `blocked` record with `kind=question` is close to an actor's escalation |
| Harness | Any program in a terminal. tsk works in Rex under ADR 0013 with no change | One adapter per harness |
| Model of the work | None. A session has a label | Four dimensions |

Two items intersect tsk:

- **OSC 7501 as an output of `tsk`.** A tsk actor that reports `working`, `blocked` with
  `kind=question`, or `done` through the protocol appears in Rex's session picker and
  in any other terminal that adopts it, with no Rex-specific code. Mission Control could
  read the same sequence instead of its own state file.
- **"Expose structured data and actions, preserve history."** Superlogical's second
  and third plan parts aim at the layer tsk's ledger sits in, from the terminal
  upwards. What they hold is not published.

## Not established

- The server's implementation language.
- Whether the macOS app's local server keeps running after the app quits. The
  announcement says a session can survive closing the application.

- Whether Rex's own source will be published. The mailing list promises "any OSS
  releases along the way", and Hashimoto's post commits to upstreaming shared terminal
  work to libghostty.
- The commercial plan. The clients and servers "are not going to be directly
  monetized".
- Restart behaviour: whether a server restart restores sessions or resumes programs.
- Architecture claims in secondary coverage, such as raw PTY streaming to clients and
  independent scrolling per client, which the pages read do not state.
- What "composable" and "operable in production" mean in the plan's second and third
  parts.
