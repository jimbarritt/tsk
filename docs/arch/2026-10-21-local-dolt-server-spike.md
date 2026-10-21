# Spike: a local Dolt server per ledger store

Date: 2026-10-21. Jim ruled out a remote Dolt server. A server on the same machine is open.
This spike measures one. Dolt 2.5.0, a copy of the live ledger store (5.7 MB, 21 missions),
the remote repointed at a scratch copy of the file remote, a Unix socket, the machine at
load average 0.4. A Python client (pymysql) stands in for a Rust client. Single runs or
medians of 15 to 50.

## Results

| Step | `dolt` launch per call (today) | Local server |
|---|---|---|
| Start | | 110 ms to the first connection, 90 MB resident when idle |
| Read (`missions`) | 98 ms | 0.6 ms median, 1.1 ms p95 |
| Larger join | | 3.3 ms |
| Insert plus Dolt commit | two launches, about 220 ms | 8.6 ms, in one transaction 8.5 ms |
| Fetch from the file remote, nothing new | 172 ms | 2.1 ms (`dolt_fetch`) |
| Push to the file remote, new commits | | 28.6 ms (`dolt_push`) |
| Push, nothing new | | 2.5 ms |
| Pull, nothing new | | 4.5 ms (`dolt_pull`) |
| 4 connections, 10 writes and commits each | | 40 writes in 0.94 s, 0 errors, 40 rows landed |

## Behaviour

- The `dolt` command line still works on the store while the server runs (`dolt sql`,
  `dolt log`).
- After `kill -9` of the server, the command line works and `dolt status` is clean. No stale
  lock stopped it.
- Fetch, pull and push run as SQL calls, so the file remote design (ADR 0017) stays.

## What it does not change

- The git steps stay: `git fetch` and `git push` of the file remote directory, which cross
  the network. A thread pause on the live origin takes 1.7 to 1.9 s. A server removes
  the `dolt` launches from that, about 0.3 s. It does not remove the push.
- The test suite starts about 6,000 `dolt` processes. A server per test costs 110 ms to
  start and removes most of the launches. The saving is an estimate until a Rust client
  exists.

## Cost of building it

- A Rust MySQL client in `cli` (a new dependency).
- Start on first use, stop when idle, one server per store, socket file in the store
  directory, one lock so two `tsk` commands do not start two servers.
- Tests that start servers need a teardown.
- Fallback to the command line path, kept and tested.
