# 18. A local Dolt server per ledger store, behind a switch

Date: 2026-10-21

## Status

Accepted. Decided by Jim on 2026-10-21 after the spike in
[2026-10-21-local-dolt-server-spike.md](../arch/2026-10-21-local-dolt-server-spike.md).
Built in 0.23.0 with the switch off by default. The measurements are in
[2026-10-21-local-dolt-server-results.md](../arch/2026-10-21-local-dolt-server-results.md).
ADR 0014 (Dolt is the store) and ADR 0017 (the ledger is a file remote carried by plain
git) stand. This ADR changes how `tsk` sends SQL to the store on one machine, and nothing
about what travels between clones.

## Context

Every SQL batch of a `tsk` command starts a `dolt sql` process. Starting `dolt` costs 60 to
100 ms, and nearly all of that is the start: a read takes 98 ms through `dolt sql` and 0.6
ms on a socket. A batched command starts 1 to 3 processes, a write with a commit starts 2,
and the conformance suite starts about 6,000. A thread pause on the live origin takes 1.7
to 1.9 s, of which about 0.3 s is `dolt` starts. The network remains the larger part of a
publish, and this change does not touch it.

A Dolt server on the same machine answers in under a millisecond, and Dolt can run fetch,
push and merge as SQL procedures. Jim ruled out a remote server and accepted a long-lived
local process.

## Decision

1. **A switch, off by default.** `TSK_DOLT_SERVER=on|off`, or `[ledger] server = "on"` in
   the user config (`${XDG_CONFIG_HOME:-$HOME/.config}/tsk/config.toml`). The environment
   wins. A value that is neither `on` nor `off` counts as unset. With the switch off,
   `tsk` starts exactly the `dolt` processes that 0.22.0 starts. `tsk ledger server status`
   prints the switch and where it came from.
2. **One `dolt sql-server` for each ledger store, started by the first command that needs
   it.** A command takes a file lock (`dolt-server.lock` in the state directory of the
   clone), starts a supervisor, and waits until `select 1` answers. Two commands that start
   at once start one server. The supervisor is `tsk ledger server run`, detached with
   `setsid`. It starts `dolt sql-server` from a generated configuration file, writes the
   pid file (`dolt-server.pid`, with the pids, the socket, the port and the state), and
   keeps the server's log in `dolt-server.log`.
3. **A Unix socket for `tsk`, and a loopback port that Dolt requires.** `tsk` connects to
   `<store>/.tsk-dolt.sock`. When that path is longer than 100 bytes (a Unix socket path
   cannot be longer), the socket is `/tmp/tsk-dolt-<uid>-<hash>.sock`. Dolt does not start
   without a TCP port (it refuses port 0 and -1, and accepts 1024 to 65535), so the
   configuration binds `127.0.0.1` on a free port that the supervisor picks, and picks
   again when the bind fails. Dolt's default `root` user has no password, so any local user
   who can reach the loopback port can open the store. The socket inherits the permissions
   of the store directory. A shared machine is not the target of `tsk`, and the port exists
   only while the server runs.
4. **An own MySQL client, with no new crate.** `cli/src/ledger/store/mysql.rs` is about 470
   lines: the handshake of the empty `root` password, `COM_QUERY`, text result sets, and
   multi statements. It turns each value into the JSON that `dolt sql -r json` prints
   (numbers for integers and floats, strings for decimals and everything else, no key for
   NULL), so the code that reads rows does not know which path made them. The `mysql` crate
   was tried first. With default features off it does not build (`flate2` needs a backend),
   and with its defaults the lock file gains 119 crates, among them the ICU tables of
   `url` and `regex`, for a client that needs one Unix socket and no TLS. The cost of the
   own client is code to keep; its tests run against the real server.
5. **What goes through the server.** Every SQL batch: `rows`, `rows_each`,
   `rows_each_after` and the write transactions of `exec`, on one connection that the
   process keeps. The Dolt commit is `call dolt_commit` inside the batch, as before. The
   `dolt` commands that have a procedure: `fetch` (`dolt_fetch`), `push` (`dolt_push`),
   `reset --hard`, `clean`, `add -A`, `commit -m`, `merge`, `merge --no-commit` and `merge
   --abort`. A merge runs with `dolt_allow_commit_conflicts` and
   `dolt_force_transaction_commit` set for its statement, so a conflict stays in the working
   set for `merge_remote` to read from `dolt_conflicts` and abort, as `dolt merge` leaves
   it. The error text of `dolt_push` is the text of `dolt push`, including the words that
   the retry loop looks for. The commit identity is set on each connection from the same
   places `dolt` reads it: `DOLT_AUTHOR_NAME` and its siblings, then the global Dolt
   configuration.
6. **What stays on the command line.** `dolt init`, `clone`, `remote add` and `remove`,
   `config` and `version`. All of them run in a store with no server, or are rare. `dolt
   gc` is not run by `tsk`. A store whose `origin` is Dolt's git blobstore (transport
   `git-remote`, before `tsk ledger migrate-transport`) stays on the command line for
   everything: its fetch and push start git, and they must start it with the environment of
   the command that asked, not of the command that started the server.
7. **Replacement of a dead, stale or hung server.** A command first connects and runs
   `select 1`, with a 5 s limit (`TSK_DOLT_SERVER_PROBE_SECS`). If that works, nothing
   else happens, and that is the whole cost of a live server. If the socket file exists
   and the connection is refused, the server is stale (a crash, `kill -9`, a container that
   stopped): under the lock, `tsk` removes the socket, the pid file and Dolt's own record,
   and starts a new server. If the connection or `select 1` times out, the server is hung
   (`SIGSTOP`, or a stuck process): under the lock, `tsk` kills the supervisor and the
   server with `SIGKILL`, checking first that the pids are those of this store's
   processes by their command lines, and starts a new one. A statement has 120 s
   (`TSK_DOLT_SERVER_QUERY_SECS`) and a fetch or push 600 s
   (`TSK_DOLT_SERVER_NETWORK_SECS`). A read that loses its connection runs again on a new
   one. A write that loses its connection fails with an error, because it may have
   applied. A pid file whose store was deleted and made again is not used: the
   device and inode of the `.dolt` directory are in the file, and a store with other
   numbers is a different store. If no server can be started for
   any reason (`dolt` is missing, something else holds the store), the process falls back
   to the command line for the rest of its run, with no message.
8. **Hibernation.** A Claude Code cloud session that hibernates loses every process and
   keeps the files. The `SessionStart` hook runs again on resume. With the switch on,
   `tsk thread session-start` checks the server before it does anything else: a live
   server costs one connection and one `select 1`; a dead or stale one is replaced in the
   background, so the refresh that follows, and the first command after the hook, find it
   running or starting. A hook never fails because of the server.
9. **Idle stop, in `tsk`.** Dolt has no option to stop an idle server. The supervisor
   reads the modification time of a file that every `tsk` connection touches
   (`dolt-server.seen`). After 120 s with no touch (`TSK_DOLT_SERVER_IDLE_SECS`) it reads
   the server's process list, and stops the server with `SIGTERM` when no connection but
   its own is open. It also
   stops the server when the store directory is gone, so a deleted store does not keep a
   process or its files alive, and when asked to stop. Each server is about 90 MB resident
   when idle, so the idle time is the cost to size.
10. **The `dolt` command line beside the server: read only, on purpose.** The supervisor
    deletes `.dolt/sql-server.info` once the server answers. A `dolt` command that finds
    that file hands its work to the server through the loopback port, and the answer to
    `dolt sql -r json` then has every number as a string (`"c":"1"`, where the command
    prints `"c":1` when it opens the store itself). tsk 0.22.0 reads counts and sequence
    numbers with `as_i64`, so against such a server it reads task numbers as 0, says
    `Origin  not published yet` for a ledger that is published, and writes duplicate
    rows. That was measured with a 0.22.0 binary before this decision. Without the file, `dolt`
    opens the store read only, because the server holds it for writing: reads return typed
    JSON as ever, and a write fails with `cannot update manifest: database is read only`.
    A `tsk` older than 0.23.0 therefore reads correctly and fails loudly on a write while
    a server runs, and works again when the server stops (at most the idle time later, or
    at once with `tsk ledger server stop`). A 0.23.0 `tsk` with the switch off does the
    stopping itself: a `dolt` write that is refused as read only stops the server of that
    store and runs again.
11. **Commands.** `tsk ledger server status|start|stop`. `status` prints the state,
    the pids, the socket, the port, the idle time, the log and the switch. `start` also
    works with the switch off, for inspection. `run` is the supervisor and is hidden.

## Consequences

- With the switch on, a command makes no `dolt` process for its SQL: `mission list` takes
  15 ms where it took 99 ms, and `task add` 23 ms where it took 225 ms. The first command
  after a stop adds the start of the server, about 110 ms (125 ms for a `mission list`). A
  command that crosses the network gains the `dolt` starts and no more: a thread pause
  takes 0.50 s where it took 0.82 s through a 330 ms shim. The conformance suite takes 91
  s where it took 271 s, and the workspace suite 275 s where it took 509 s. The numbers
  are in the results document.
- Each server holds about 90 MB. The suite starts one for each ledger store, so the test
  fixtures set a 3 s idle time and stop their servers when they are dropped.
- The git steps are unchanged: a publish still costs its two `git push` calls and the
  `git fetch` for the network.
- The server code is for Unix only (Linux and macOS, the targets of the release builds):
  `setsid`, `flock`, `kill` and `send` come from `libc`, which the crate already used, and
  the client uses `std::os::unix`.
- A downgrade needs `tsk ledger server stop` first, or a wait of the idle time. The
  release notes say so.
- The default can change to on in a later release, when the measurements across sessions
  support it. The flip is a one-line change of `resolve_switch`.

## Alternatives not taken

- **The `mysql` crate.** See decision 4.
- **A server without a loopback port.** Dolt does not offer one.
- **Leaving Dolt's `sql-server.info` in place.** It keeps `dolt status` and `dolt sql`
  working for a person, at the cost of the typed JSON that every older `tsk` relies on.
  A person who needs the command line to write stops the server first.
- **A stop timer inside Dolt.** None exists.
- **One server for all clones.** A server serves a directory of databases, and a clone's
  store is reached from the clone. One server for each store keeps a deleted store from
  being served by another clone's process, and keeps the lock and the pid files beside the
  store they describe.
