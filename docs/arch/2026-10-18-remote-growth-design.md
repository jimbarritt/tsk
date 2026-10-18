# Remote growth and fetch cost: measurements and design, 2026-10-18

Task T-05 of mission M-PERF. Written by Claude (Sonnet 5.5). The decisions at the end
are Jim's. Nothing here is built, and nothing was written to the live ledger.

## Summary

- The git remote is slow for two separate reasons. The review
  (`docs/arch/2026-10-17-architecture-and-performance-review.md`, section Performance)
  named only the second.
  1. **Round trips.** A `tsk ledger fetch` makes 5 network operations to GitHub.
     Each costs 0.2 to 0.4 s through the cloud proxy, and they carry about 10 KB in total.
     This is about half of the live time and does not depend on the file count.
  2. **Table-file reads.** `dolt fetch` re-reads every table file in the remote tree
     through `git cat-file`, on every fetch, whether or not it changed. The launch count
     is 6 per file in the tree and grows with every push. This is about a quarter of the
     live time today, and it is the part that scales.
- `dolt gc` followed by a push to the same branch does not shrink the remote. A push to
  a fresh branch does: 125 files became 3, 750 launches became 18, and the local-origin
  fetch fell from 1.28 s to 0.52 s.
- A one-line transport setting (`protocol.version=0` for the git processes `tsk` starts)
  removes 0.7 s from each live fetch with no storage change.
- Beads uses the same git-remote mechanism and its users reported the same `cat-file`
  growth. Upstream Dolt fixed much of it. On the same data Dolt 2.5.0 launches 456
  `cat-file` processes against 1362 for the pinned 1.88.2 (see What beads does).
- Recommendation: do the round-trip work and a Dolt bump first (T-01 to T-04 plus three small additions
  below), and add a `tsk ledger compact` that rewrites the branch with a lease. Do not
  squash Dolt history, and do not move to a server.

## Problem, measured

The review says a live `tsk ledger fetch` takes 2.3 to 2.5 s against 0.5 s on a local
origin, and puts the difference down to the remote's table-file count. This task split
the time. All numbers below are from one machine under load (see Method and caveats).

One `tsk ledger fetch` with nothing new, medians of 6 interleaved runs, ledger of 125
files (live) or 127 files (scratch):

| Where | Total | Network | `cat-file` loop | Other local |
|---|---|---|---|---|
| Live origin (GitHub through the proxy) | 2.65 s | 1.37 s | 0.70 s | 0.58 s |
| Scratch bare origin, local | 1.28 s | 0 | 0.70 s | 0.58 s |
| Scratch origin with 330 ms added per network operation | 2.63 s | 1.35 s | 0.70 s | 0.58 s |

How the columns were found:

- Total: wall clock of the command (six interleaved runs, in the commands section).
- Network: live total minus local total (1.37 s). A second check from the launch
  timeline gives 1.5 s for 5 operations (Finding 1c).
- `cat-file` loop: the time between the first and last `git cat-file` launch in the
  timeline, 0.70 to 0.74 s on both origins. It is the same on live and local, as it must
  be, because it only reads the cache clone on disk.
- Other local: the remainder. Six `dolt` launches at 55 to 75 ms each are about 0.4 s of
  it. The rest is `tsk` startup, `git status`, `git reset --hard` in the documents
  worktree and `dolt reset --hard origin/main`.

So on the live origin the network is 52 % of a fetch, the `cat-file` loop is 26 % and
everything else is 22 %. The review's attribution of 2.3 s "over the network" to the
file count is wrong in this respect. Network time here does not depend on the file
count. File-count time is the 0.7 s loop (see Finding 2).

### Contradictions with the 2026-10-17 review

| Review, section Performance | Finding here |
|---|---|
| "about 2.3 s over the network and 0.5 s on a local origin", with the cost put down to the remote's file count | The network share (1.4 s) comes from 5 round trips, not from file count. The file-count share is the 0.7 s loop. A local origin with the same data takes 1.3 s on this machine, not 0.5 s. The review's machine may be faster or less loaded. |
| "two processes per file" (423 launches for 107 files) | True for plain table files. A `.darc` archive file costs 10 launches, not 2. The live tree is half archives, so the real rule is `2 × plain + 10 × archive`, which fits both 423 and the 750 seen here. |
| "A local `dolt gc` ... still launched 440 processes, because the cost is the remote's file count" | Confirmed. |
| "Pushing the compacted database did not shrink the remote" | Confirmed on the same branch (127, 129, 131, 132 files after gc, gc `--full` and `push --force`). A push to a fresh branch does shrink it (Finding 3). |
| Proposal 6: session start after proposals 1 to 3 "about 2.6 s live ... under 1 s once 4 lands" | Session start on live already costs 3.5 to 4.2 s, and proposals 1 to 3 do not touch its path (Finding 5). Compaction alone takes a live fetch to about 1.9 s, not under 1 s. |
| Proposal 4, "a `tsk ledger compact` that writes a compacted database to a fresh branch tip" | Works, and it is tested here, including stale clones. It needs a force update of the branch (Option B). |

## Method and caveats

- All experiments ran on scratch copies. The live origin was read only: `git fetch` of
  `refs/heads/tsk/ledger` and `refs/heads/tsk/docs`, `git ls-remote`, and
  `tsk ledger fetch`, `tsk ledger status`, `tsk thread session-start` and
  `tsk thread list` run from a scratch clone whose `origin` URL is the real one and whose
  `XDG_STATE_HOME` is in the scratch directory. These write only to the scratch store.
  No `tsk` write command and no push touched the real environment. The fetch into
  `/home/user/tsk` updated its remote-tracking refs `refs/remotes/origin/tsk/ledger` and
  `.../tsk/docs`.
- The scratch bare origin was seeded with the real refs. At seeding time the ledger had
  125 files, 60 commits and 939 KB. (The live ledger reached 64 commits during the work.)
- The machine had a load average of 2 to 16 because another agent was building. Times
  carry that noise (the loop time and local totals vary by up to 2x between runs). Launch
  counts are exact and do not vary. Where it matters, times are medians of 5 or 6
  interleaved runs.
- The binary is `/home/user/tsk/target/release/tsk`. It reports `0.15.0` (the workspace
  version is 0.15.1). `dolt` is 1.88.2.
- `tc` is not installed. Network delay is emulated by a small C wrapper around `git`
  that sleeps before `fetch`, `push`, `ls-remote`, `clone` and `pull`, and logs every
  launch. A 330 ms sleep reproduced the live total to within 1 % (2.63 s against 2.65 s).
  330 ms is also what a live `git fetch` with nothing new costs (0.33 to 0.39 s).
- Network bytes were counted with `GIT_TRACE_CURL`.

### Setup, repeatable

```bash
S=/tmp/scratch-remote-growth; mkdir -p $S/bin2 && cd $S
# 1. seed a bare origin with the real refs (read of the real origin)
cd /home/user/tsk
git fetch origin refs/heads/tsk/ledger:refs/remotes/origin/tsk/ledger \
                 refs/heads/tsk/docs:refs/remotes/origin/tsk/docs
git init -q --bare $S/origin.git
git push -q $S/origin.git refs/heads/main:refs/heads/main \
  refs/remotes/origin/tsk/ledger:refs/heads/tsk/ledger \
  refs/remotes/origin/tsk/docs:refs/heads/tsk/docs   # a "push negotiation" warning is harmless
git -C $S/origin.git symbolic-ref HEAD refs/heads/main
# 2. isolated environment
cat > $S/env.sh <<EOF
export XDG_STATE_HOME=$S/state XDG_CONFIG_HOME=$S/config CLAUDE_CODE_REMOTE_SESSION_ID=session_fake1
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export PATH=/home/user/tsk/target/release:\$PATH
EOF
. $S/env.sh; mkdir -p $XDG_STATE_HOME $XDG_CONFIG_HOME
git clone -q $S/origin.git $S/c1 && cd $S/c1 && git config user.name t && git config user.email t@t
tsk ledger fetch            # builds the ledger store and the cache clone
# 3. a clone that reads the REAL origin (reads only; never run a write command here)
git clone -q /home/user/tsk $S/live1 && git -C $S/live1 remote set-url origin https://github.com/jimbarritt/tsk
```

The counting and delaying wrapper, `$S/gw.c`, built with `gcc -O2 -o $S/bin2/git $S/gw.c`
and put first on `PATH` (`PATH=$S/bin2:$PATH`):

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <time.h>
int main(int argc,char**argv){
  const char*verbs[]={"fetch","push","ls-remote","clone","cat-file","rev-parse","ls-tree",
    "status","log","reset","remote","pull","commit","add","gc","repack",0};
  const char*verb="";
  for(int i=1;i<argc&&!*verb;i++)for(int j=0;verbs[j];j++)if(!strcmp(argv[i],verbs[j])){verb=verbs[j];break;}
  const char*d=getenv("GITWRAP_DELAY_MS");
  int net=!strcmp(verb,"fetch")||!strcmp(verb,"push")||!strcmp(verb,"ls-remote")||
          !strcmp(verb,"clone")||!strcmp(verb,"pull");
  if(d&&net)usleep(atoi(d)*1000);
  const char*lg=getenv("GITWRAP_LOG");
  if(lg){struct timespec t;clock_gettime(CLOCK_REALTIME,&t);char b[256];
    int n=snprintf(b,sizeof b,"%ld%09ld 0 %s",(long)t.tv_sec,t.tv_nsec,verb);
    for(int i=1;i<argc&&n<230;i++)n+=snprintf(b+n,sizeof b-n," %s",argv[i]); b[n++]='\n';
    int fd=open(lg,O_WRONLY|O_APPEND|O_CREAT,0644); if(fd>=0){write(fd,b,n);close(fd);}}
  argv[0]="git"; execv("/usr/bin/git",argv); return 127;
}
```

A bash wrapper that did the same thing was tried first and distorted the timing
(750 bash launches added about 2 s), so only the C wrapper was used for times.

## Finding 1: network and local split

### 1a. What the live fetch costs

Live `git` operations from a scratch bare repo (`git init --bare live.git`, remote set to
the real URL), milliseconds:

| Operation | Time |
|---|---|
| First `git fetch` of `refs/heads/tsk/ledger` into an empty repo (about 1 MB) | 823 |
| `git fetch` of `tsk/ledger` with nothing new, 3 runs | 391, 367, 350 |
| `git fetch` of `tsk/docs` with nothing new, 3 runs | 331 to 345 |
| `git ls-remote --heads origin` | 326 |
| `curl` of `info/refs` (one HTTP request) | 165 |

A fetch with nothing new makes 2 HTTP requests under protocol version 2 (a GET of
`info/refs`, then a POST for `ls-refs`) and costs 0.33 to 0.39 s. Under protocol version
0 or 1 it makes 1 request (the ref advertisement) and costs 0.17 to 0.21 s.

### 1b. What one `tsk ledger fetch` does on the network

Counted with `strace -f -e trace=execve,connect` and `GIT_TRACE_CURL`:

| Operation | By | Round trip live (ms) |
|---|---|---|
| `git ls-remote --exit-code origin refs/heads/tsk/ledger` | `tsk` | 484 |
| `git ls-remote --heads origin` in the cache clone | `dolt fetch` | 306 |
| `git fetch ... +refs/heads/tsk/ledger:refs/dolt/remotes/origin/...` | `dolt fetch` | 320 |
| The same `git fetch` again, after the `cat-file` loop | `dolt fetch` | 342 |
| `git fetch --quiet origin refs/heads/tsk/docs` | `tsk` | 342 |

That is 5 operations and 1.8 s in a run with extra tracing. There are 10 TCP connections
to the local proxy (`127.0.0.1:36137`, one per HTTP request), about 5 KB sent and 9 KB
received including headers. Bytes are negligible. The cost is latency, multiplied by the
number of operations and, under protocol version 2, by 2 requests each.

`dolt fetch origin` runs the same `git fetch` twice (before and after the file-reading
loop) and ends with `dolt reset --hard origin/main` even when nothing changed.

### 1c. Split of `tsk ledger fetch`, three environments

From the launch timeline of one run (wrapper log, no tracing), milliseconds:

| Phase | Local origin | Live origin |
|---|---|---|
| Start to `ls-tree` (tsk checks, `dolt` start, `ls-remote`, first `git fetch`) | 335 to 353 | 1369 to 1552 |
| First to last `cat-file` (746 launches) | 692 to 734 | 703 to 737 |
| Last `cat-file` to end (second fetch, docs fetch, `dolt reset`, `dolt sql`) | 424 to 474 | 711 to 812 |
| Total | 1476 to 1571 | 2787 to 3078 |

The live run adds about 1.1 s before the loop and 0.3 s after it, and nothing inside the
loop.

## Finding 2: how Dolt's git remote reads the remote

What was observed, with the evidence:

- **Where the cache is.** `<ledger-db>/.dolt/git-remote-cache/<hash>/repo.git`, a bare
  git repository created by `dolt` on first use. `~/.dolt` holds only `config_global.json`
  and `eventsData`. The remote is recorded in `.dolt/repo_state.json` as
  `git+file:///...` or `git+https://...` with `"params": {"git_ref": "refs/heads/tsk/ledger"}`.
  The local noms store mirrors the remote's file list (129 files locally for 127 on the
  remote), so the local `.dolt/noms` directory is not a compact copy until `dolt gc` runs.
- **Each `dolt fetch` fetches the whole branch tip.** It runs `git fetch --no-tags
  --refmap= origin +refs/heads/tsk/ledger:refs/dolt/remotes/origin/heads/tsk/ledger/<uuid>`
  with a new UUID for every fetch. Git sends only objects the cache clone lacks, so the
  bytes are small. The cache clone gains one ref per fetch: 77 after about 40 commands in
  a scratch store, 194 in the real store. With 5000 extra refs a fetch took 75 ms instead
  of 14 ms, so this is housekeeping and not a cost today.
- **It re-reads every table file on every fetch.** After the fetch it runs `git ls-tree
  -r -t <tip>^{tree}`, then `git cat-file -s <sha>` and `git cat-file blob <sha>` for the
  files. A fetch with nothing new on a 125-file tree launched 746 `cat-file` processes.
  Unchanged files are read again.
- **Archive files are read 5 times each.** Counting launches per blob SHA in one run:

  | Kind of file in the tree | Files | Reads per file | Launches |
  |---|---|---|---|
  | `.darc` archive | 62 | 5 `-s` + 5 `blob` = 10 | 620 |
  | Plain table file (321 bytes each) | 61 | 1 `-s` + 1 `blob` = 2 | 122 |
  | `manifest` and one more | 2 | 4 | 4 |

  The likely cause is that each ranged read of an archive (footer, index, metadata)
  becomes a full `cat-file` pair. This matches beads issue #4770 (see What beads does),
  where the same pair per ranged read was found in Dolt's blobstore. Every push adds one archive and one 321-byte plain file.
  `.darc` is Dolt's archive format (`DOLTARC` signature, format version 3, zstd or snappy
  chunk groups with an index), written by `dolt gc` and by the git blobstore on push.
  `dolt admin archive-inspect <file>` prints one.
- **The count scales with total files, not new files.** Test: a writer clone adds 1, 10
  and 50 small commits (one `tsk task add` and one `tsk ledger push` each, to a scratch
  origin copied from the real data). After each stage a second clone that already held
  the previous state fetched, and then a fresh clone fetched. (`$S/scale.sh` in the
  commands section.)

| Pushes added | Files in remote tree | Steady fetch (nothing new): launches | Time (s, loaded) | Launches per file | First fetch after the change, existing clone: launches | Time (s) | First fetch, fresh clone: launches | Time (s) |
|---|---|---|---|---|---|---|---|---|
| 0 | 127 | 762 | 1.61 | 6.0 | | | | |
| 1 | 129 | 774 | 1.49 | 6.0 | 786 | 1.69 | 1030 | 1.83 |
| 10 | 147 | 882 | 1.64 | 6.0 | 990 | 2.12 | 1174 | 2.05 |
| 50 | 227 | 1362 | 1.96 | 6.0 | 1908 | 3.66 | 1814 | 2.38 |

Reading the table:

- The steady launch count is 6.0 per file at every size: `2 × plain + 10 × archive` with
  half the files of each kind. It is linear in the total file count.
- A fetch that brings new files costs extra launches for the new files (546 extra for 80
  new files after the 50 pushes, 6.8 per new file). Then it falls back to the steady
  count.
- Time rises about 4.8 ms per file in this environment (0.47 s for 98 files). At 1000
  files that is about 4 s more than today, on a local origin, before any network time.
  The live ledger gained 18 files over about 20 commits since the review, so the growth
  is between 1 and 2 files per push.
- The git history is not the problem. In a scratch run with 6 pushes the branch went from
  64 commits to 2 at the third push while the file count kept rising (127, 129, 131,
  133, 135, 137). Dolt's git blobstore re-roots the git history at about 64 commits and
  keeps the tree. The first re-root on the live branch is a non-fast-forward update that
  will happen without any change from us (the live branch was at 64 commits at the end of
  this work).
- **Batching writes does not add files.** 20 `tsk task add` writes followed by one
  `tsk ledger push` added 2 files to the remote (139 to 141), the same as one write
  and one push. Growth is per push, not per write. This supports T-08.

## Finding 3: what shrinks or flattens it

All on scratch copies of the real 127-file ledger. "Fresh clone fetch" is the second to
fourth `tsk ledger fetch` in a new clone with a local origin, no added delay.

| Change | Files in remote | Launches per fetch | Fetch time, local origin | Verdict |
|---|---|---|---|---|
| Baseline | 125 to 127 | 750 | 1.28 to 1.43 s | |
| `dolt gc`, then `tsk ledger push` with a new change, same branch | 127, then 129 | no change | no change | Does not shrink. The push is additive. |
| `dolt gc --full`, then push, same branch | 131 | no change | no change | Does not shrink. |
| `dolt gc`, then `dolt push --force origin main`, same branch | 132 | no change | no change | Does not shrink. Force adds a file. |
| `dolt gc`, then `dolt push` to a **fresh** git ref, branch then pointed at it | 3 | 18 | 0.52 s | **Shrinks.** 125 files become 3 (one 600 KB archive, one 321-byte file, the manifest). |
| `dolt gc --full --archive-level=0`, then push to a fresh ref | 3 | 18 | 0.45 to 0.58 s | Same as above. The push converts the table files to an archive anyway, so the archive level has no effect on the remote. |
| Squash Dolt history to 2 commits (`dolt reset --soft <first>`, `dolt commit`), `dolt gc --full`, push to a fresh ref | 3 | not measured | not measured | No gain over `gc` alone: store 603,962 bytes against 602,877 bytes. The history is already deduplicated in one archive. It also breaks every other clone's history, so it is rejected. |
| Batch 20 writes into one push | +2 | grows by about 12 | | Slows the growth, does not reduce the size. |
| `git fetch --depth 1`, partial clone, `fetch.negotiationAlgorithm` | n/a | no change | no change | Not useful. The fetch moves 10 KB. `dolt` builds its own `git fetch` arguments, so there is no setting for depth. `noop` and `skipping` negotiation gave 329 and 340 ms against 320 to 350 ms. |
| `DOLT_DISABLE_EVENT_FLUSH=1` | n/a | no change | 437 to 465 ms against 425 to 467 ms | No effect. (The sandbox gateway answers 403 to the `eventsapi.dolthub.com` telemetry connection on every `dolt` launch. It is not on the critical path.) |
| `protocol.version=0` for all git processes | n/a | no change | live total 2.37 s to 1.76 s (6 interleaved runs) | **Cuts network time by half**, no storage change (see Finding 5). |

The compacted remote also holds after other clones use it. Test, with the compact result
forced onto the branch of a copy of the origin (`git update-ref`):

1. Clone A had the old state and one unpushed local mission. `tsk ledger push` fetched the
   compacted tip, merged, and pushed (1.4 s). The remote went from 3 to 5 files.
2. Clone B had the old state. `tsk ledger fetch` took the compacted tip. B's status read
   "nothing to push" and "up to date". B wrote a mission and pushed (5 to 7 files).
3. A fetched again and saw both missions.

So a stale clone recovers on its next fetch. The cache clone's fetch refspec is forced
(`+refs/heads/...`) and the Dolt commit graph is unchanged, so only the physical table
files differ. Growth then resumes at 2 files per push.

How a compaction would be made, as run here:

```bash
cd $(tsk ledger path)                       # a clone that has fetched and has no local changes
dolt gc                                     # 129 noms files to 4, 1.4 MB to 620 KB
dolt remote add --ref refs/heads/tsk/ledger-compact fresh git+https://<origin-url>
dolt push fresh main                        # writes 3 files to a new branch
git push --force-with-lease=refs/heads/tsk/ledger:<sha-read-at-start> origin \
    refs/heads/tsk/ledger-compact:refs/heads/tsk/ledger
git push origin :refs/heads/tsk/ledger-compact
```

Note `dolt remote add` takes `--ref`, not `--git-ref`. Whether the cloud proxy accepts
the force-with-lease update on the real origin is not tested. `dolt` itself already
pushes to `refs/heads/tsk/ledger` with `git push --force-with-lease=refs/heads/tsk/ledger:<sha>`
on every `tsk ledger push` (seen in the wrapper log), so lease pushes work. A
non-fast-forward update is what Dolt's own re-rooting at 64 commits does, and the live
ledger reaches that point soon. Its first occurrence is a free test.

## Finding 4: server alternative, facts only

Jim ruled a Dolt server out as the shared store on 2026-10-18 (M-PERF decisions). The
facts are recorded for completeness.

- `dolt sql-server` speaks the MySQL wire protocol over TCP (port 3306 by default). With
  `--remotesapi-port` it also serves Dolt's remote API, which `dolt clone`, `fetch` and
  `push` use over gRPC on HTTP/2 plus signed HTTP chunk transfers. These two points are from memory of Dolt's documentation and were not re-verified here.
- DoltHub exposes the remote API on `doltremoteapi.dolthub.com:443` and a REST SQL API
  on `www.dolthub.com`. The REST API is plain HTTPS, with read and (asynchronous) write
  endpoints that need an API token. Not verified here.
- The design document `missions/operational/M-TSK-SCHEMA/design.md`, section Custom
  refs, records that a remote `dolt sql-server` or DoltHub was "Blocked: raw TCP and gRPC
  are not supported through the proxy" (tested 2026-10-15).
- Probe today: `curl https://www.dolthub.com/`, `https://doltremoteapi.dolthub.com/` and
  `https://dolthub.com/` each fail with `CONNECT tunnel failed, response 403`, and the
  proxy status endpoint lists `connect_rejected ... gateway answered 403 to CONNECT
  (policy denial or upstream failure)` for all three. So at present the cause is the
  host allowlist, before any protocol question. That means the REST API route is also
  closed until a host is allowed.
- What it would take: Jim adds the host under Allowed domains in the cloud environment's
  Network access setting (documentation topic `environment.network`). That settles the
  allowlist only. Raw TCP for MySQL, and gRPC over HTTP/2, would still need to pass the
  proxy's CONNECT tunnel, which the 2026-10-15 test says they do not. The routes that
  would work through an HTTPS-only proxy are a relay (a small service on an allowed host
  that accepts HTTPS from the session and talks MySQL or gRPC to a server) or DoltHub's
  REST SQL API with a token. Either replaces the git remote and the offline-first model
  and needs a hosted service and credentials in every session. Nothing was built.

## Finding 5: round trips per command, and the minimum

Counted with the wrapper log and `strace -f -e trace=execve`, from a scratch clone with
a local origin. "Network ops" are `git fetch`, `git ls-remote` and `git push` launches
(including those `dolt` starts in the cache clone). Origin had 127 to 163 files.

| Command | `git fetch` | `ls-remote` | `git push` | Network ops | `cat-file` launches | `dolt` launches |
|---|---|---|---|---|---|---|
| `tsk ledger fetch` | 3 | 2 | 0 | 5 | 774 | 6 |
| `tsk ledger status` | 3 | 1 | 0 | 4 | 786 | 5 |
| `tsk thread session-start` | 3 | 3 | 0 | 6 | 774 | 10 |
| `tsk ledger push` (nothing to push) | 3 to 6 | 1 | 0 | 4 to 7 | 774 to 966 | 6 |
| `tsk ledger push` (one change) | 10 to 13 | 2 | 2 | 14 to 17 | 1584 to 1946 | 9 to 11 |
| `tsk thread pause` | 9 to 17 | 4 | 2 | 15 to 23 | 2394 to 2934 | 20 |
| `tsk thread list` | 3 | 2 | 0 | 5 | 786 | 38 |
| `tsk mission list` (no fetch) | 0 | 0 | 0 | 0 | 0 | 1 |

The scratch origin grew from 127 to 163 files while these were measured, so later runs
sit at the high end of each range. The range in `git fetch` comes from `dolt` repeating
its ledger fetch inside a push. `dolt` launches count successful `execve` calls under
`strace`.

`thread pause` breaks down, in the last clean run (163 files), as: 9 ledger fetches
(5 in earlier runs), 2 documents fetches, 1 code-repository fetch
(`git fetch --prune origin +refs/heads/*`), 3 `ls-remote` in the cache clone, 1 `ls-remote`
by `tsk`, and 2 pushes, each `git push --force-with-lease`. It reads the whole table-file
tree three times (2934 launches, three passes of about 980).

Two things the review did not count:

- `tsk thread session-start` makes a third `ls-remote`: `git ls-remote --tags --refs
  https://github.com/jimbarritt/tsk refs/tags/v*` (`cli/src/self_update.rs:106`). This is
  the version check. It costs one more round trip (0.2 to 0.4 s) on every session start.
- `tsk` runs `git ls-remote --exit-code origin refs/heads/tsk/ledger` before `dolt fetch`
  runs its own `ls-remote`. Both ask the same remote the same question.

### The minimum

| Command | Minimum with the current storage | What it needs |
|---|---|---|
| Read with nothing new (`ledger fetch`, `session-start`, `thread list`) | 1 `git ls-remote origin refs/heads/tsk/ledger refs/heads/tsk/docs`, no `dolt fetch`, no `cat-file` loop | The last remote heads remembered locally, and a skip when both match |
| Read with something new | 1 `ls-remote`, 1 `dolt fetch` pass, 1 documents fetch | As above |
| Write with nothing new on origin | 1 `ls-remote`, then 1 push of each store (2 pushes) | Push without a second fetch |
| Write after origin moved | 1 `ls-remote`, 1 fetch pass, 2 pushes | T-01 |

A skip is safe because equal heads mean no new data. It also removes the 750-launch loop
from every command that finds nothing new, which today is nearly every fetch.

### How far T-01 to T-04 get on their own

Live-equivalent times use the 330 ms emulation (it matched the live total within 1 %) on
a scratch origin with 155 files. Medians of 3 runs, seconds:

| Command | Local origin | With 330 ms per network op | With 200 ms per network op (about protocol v0) | Live, measured (load 6) |
|---|---|---|---|---|
| `tsk ledger fetch` | 1.6 | 2.8 | 2.2 | 2.7 |
| `tsk thread session-start` | 2.7 to 3.9 | 4.2 to 5.7 | 3.3 to 3.7 | 3.5 to 4.4 |
| `tsk thread pause` | 5.6 | 10.1 | 8.2 | not run (a write) |

- **Session start is not helped by T-01 to T-04.** T-01 removes the second fetch in the
  push path, T-02 removes `dolt sql` processes in thread writes, T-03 drops the code
  fetch in `pause`, T-04 adds tests. Session start runs one refresh and no push, so it
  has no duplicate for them to remove. It stays at 3.5 to 4.4 s live: the cost of
  `ledger fetch` (2.7 s) plus the tags `ls-remote` (about 0.35 s) and 4 more `dolt`
  launches (about 0.3 s).
- `thread pause` gains the most. From the launch counts, T-01 removes one of the three
  table-file passes (about 2.8 s live-equivalent, as a `ledger fetch` costs 2.7 to
  2.9 s) and one documents fetch (0.35 s), T-03 removes the code fetch (0.35 s), and
  T-02 removes about 2 s of `dolt sql` launches. That puts `pause` at about 4.5 s
  live-equivalent, from about 10 s. These are estimates from the measured parts and were
  not run.
- None of T-01 to T-04 changes the 5 operations and 750-launch loop of the one remaining
  refresh. That is the part the options below address.

## What beads does

Beads is the Dolt-backed issue tracker (`bd`) that `docs/kb/orchestration-ecosystem/beads-as-backing-store-analysis.md`
already compares with tsk. That document says beads syncs through Dolt remotes and
defaults to `refs/dolt/data`. This section answers the sync-cost questions with sources.
The project's README at https://github.com/steveyegge/beads links its docs under
`gastownhall/beads`, so the sources below use that path. Fetched 2026-10-18 with WebFetch and WebSearch. The docs
site `beads.gascity.com` could not be fetched from this sandbox (`ENOTFOUND`), so claims
that rest only on search-result summaries of that site are marked "not verified".

### How beads syncs

| Claim | Source | Status |
|---|---|---|
| Sync is `bd dolt push` and `bd dolt pull` against Dolt remotes. For a git project the remote is the code repository's own `origin`. Data lives on `refs/dolt/data`, not on a branch. `bd init` records `origin` as `sync.remote`. `bd bootstrap` clones from `refs/dolt/data` | https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md and https://github.com/gastownhall/beads/blob/main/docs/architecture/dolt.md | Verified |
| Other remote kinds are DoltHub, S3, GCS, `git+ssh://` and `file://` | https://github.com/gastownhall/beads/blob/main/docs/architecture/dolt.md | Verified |
| `.beads/issues.jsonl` is an export, not the sync channel | https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md | Verified |
| A Dolt server is used for concurrent writers on one machine, not as the cross-machine channel. In server mode auto-commit defaults to off | https://github.com/gastownhall/beads/blob/main/docs/architecture/dolt.md | Verified |
| There is no automatic push or pull cadence in the documented core. Push is manual and sits in session-close instructions. `bd sync` has been a deprecated no-op since v0.51 | https://github.com/gastownhall/beads/issues/6034 (push is a session-close duty in three governance documents), and a search summary of https://www.mintlify.com/steveyegge/beads/guides/sync-and-backup | Partly verified. The `bd sync` statement is from a search summary only |
| An optional `dolt.auto_push` setting makes every write push, and a community script runs a daemon with `--auto-commit --auto-push --auto-pull --interval 30s` | https://www.mintlify.com/steveyegge/beads/guides/sync-and-backup (search summary) | Not verified. Unofficial or third-party |

So beads uses the same mechanism as tsk, Dolt's git remote, with a different ref.
`refs/dolt/data` is a ref outside `refs/heads/*`, which the cloud proxy rejects (see
`docs/kb/orchestration-ecosystem/beads-as-backing-store-analysis.md`, row "Sync to a git
remote, default ref"). tsk already avoided this by using a branch.

### Slow fetch and a growing remote: what has been reported

All are open issues in https://github.com/gastownhall/beads unless stated.

| Report | What it says | Source |
|---|---|---|
| Quadratic `cat-file` reads (closed 2026-07-17) | Dolt's git blobstore launched a `git cat-file -s` and `git cat-file blob` pair for every ranged chunk read, and each read streamed and discarded the bytes before its offset. About 60 spawns a second; one blob read 33 times in 15 s. A pull hung for 10 minutes or more. Fixed in Dolt v2.2.0 (dolthub/dolt#11247: spool git-backed table files to a temp file and batch `cat-file --batch-check`). After the fix: Windows pull 2.289 s, Linux pull 5.4 to 7 s on a 243 MB store, fresh clone of 71,605 chunks in about 25 s | https://github.com/gastownhall/beads/issues/4770 |
| Per-chunk `cat-file` still spawned at once (open) | On Windows with git+https, one pull or push starts one `git cat-file -s` per remote chunk, about 230 processes, and fails 3 of 3 pushes with `ERROR_NOT_ENOUGH_MEMORY`. Each push adds a chunk, so the cost grows with the remote. Reporter proposes a worker pool of 8 to 16 or one long-lived `cat-file --batch`. bd 1.3.0 with embedded Dolt 2.1.9 | https://github.com/gastownhall/beads/issues/7082 |
| Table files grow without bound (open, "resolved pending bump") | On a git+ssh remote, 2,259 table files, pull time 10+ minutes and rising daily. Dolt's conjoin (threshold 256 files) failed because one push opened about 247 SSH connections against `MaxStartups` 10. Raising `MaxStartups` let conjoin run: 2,264 files fell to 32. Upstream Dolt fixes (dolt#11742, #11745) are reported done, and the reporter could not reproduce on Dolt 2.3.5 | https://github.com/gastownhall/beads/issues/6359 |
| Disproportionate storage (open) | A few-thousand-issue repo: `.beads/` 2.3 GB after compaction, of which `embeddeddolt` 1.7 GB, against a 2.9 MB JSONL export. A full push held bd's exclusive lock for about 50 minutes. Root cause unconfirmed | https://github.com/gastownhall/beads/issues/4625 |
| No retry on transport timeout (open) | A single timeout aborts `bd dolt push` with a partial write. Operators re-run by hand. No retry, timeout or backoff knob | https://github.com/gastownhall/beads/issues/6034 |
| Fixed 30 s pre-push fsck timeout (open) | Every long-lived database eventually crosses it, because each write creates a Dolt commit | https://github.com/gastownhall/beads/issues/4822 and feature request https://github.com/gastownhall/beads/issues/7255 (run the check on a schedule) |

I did not find a reported per-operation sync time for a small ledger like tsk's. The
nearest numbers are the post-fix timings in #4770 (pull 2.3 s to 7 s). Beads documents no
latency benchmark (https://github.com/gastownhall/beads/blob/main/docs/architecture/dolt.md).

### What beads does about it

| Measure | Detail | Source | Status |
|---|---|---|---|
| `dolt gc` and `dolt gc --full` | Documented as the first diagnosis. A default pass collects only the new generation. `--full` collects both. If `--full` frees nothing, a branch, tag or cached remote-tracking ref still references the bytes | https://github.com/gastownhall/beads/blob/main/docs/architecture/dolt.md | Verified |
| `bd flatten` (all history into one commit) and `bd compact` (squash commits older than N days) | Build a temporary branch and hard-reset `main` onto it. Marked irreversible. On Dolt 2.3.x the reset can fail with `context canceled`. A default `gc` after a squash often reclaims little because orphaned commits sit in the old generation | https://github.com/gastownhall/beads/blob/main/docs/architecture/dolt.md | Verified |
| Multi-clone warning for a squash | A live peer undoes the squash on its next sync: it pulls the new baseline, merges it into its old chain and pushes the whole old history back. Stop every syncing machine and back up first | https://beads.gascity.com/recovery/history-squash (via search summary) | Not verified: page not fetchable |
| Backups | `bd backup` makes a full Dolt backup to a path, NAS or DoltHub | https://github.com/gastownhall/beads/blob/main/docs/architecture/dolt.md | Verified |
| Dolt upgrades | Beads relies on newer Dolt for the git-remote fixes: spooling, coalesced fetches, a persistent fetch anchor ref for incremental pulls, and settings that trade disk for faster incremental transfer | https://github.com/dolthub/dolt/releases (2.3.3 PRs #11745, #11742; 2.4.1 PRs #11993, #11991, #11989; 2.5.0 PRs #12061, #12062) | Verified from the release list. I did not read the PR bodies |

I found no case where beads does anything to a git remote's file count other than rely on
Dolt's own conjoin and these history tools. No `bd` command writes a compacted tree to a
fresh ref, as `tsk ledger compact` would.

### Test: does a newer Dolt cut tsk's cost?

The beads reports point at Dolt itself. tsk's hook pins Dolt 1.88.2 (`CLAUDE.md`,
`plugin/hooks/install-dolt.sh`). Dolt 2.5.0 was downloaded from
https://github.com/dolthub/dolt/releases/download/v2.5.0/dolt-linux-amd64.tar.gz into the
scratch directory and put first on `PATH`. Same scratch origin (227 files after the 50
pushes), fresh clone, `tsk ledger fetch` with nothing new:

| Dolt | Launches | Local time | With 330 ms per network op |
|---|---|---|---|
| 1.88.2 | 1362 | 1.72 to 1.95 s | 3.47 to 3.77 s |
| 2.5.0 | 456 | 1.30 to 1.33 s | 2.60 to 2.74 s |

The launch count falls to 2.0 per file (from 6.0), still linear in file count. The time
saving is about 0.5 s locally and 0.9 s with live-like latency at 227 files, and it grows
with the file count. Mixed versions work: a 2.5.0 clone pushed, a 1.88.2 clone fetched,
merged its own write and pushed, and the 2.5.0 clone read both. This confirms that the
`cat-file` pair per ranged read in Finding 2 is the beads #4770 mechanism, partly fixed
upstream. It was run on scratch only.

### What tsk should copy or avoid

- **Copy:** track a recent Dolt. Beads' worst reports were fixed upstream (#4770, #6359).
  Raise tsk's pinned Dolt from 1.88.2 to 2.5.0 or later after a broader test.
- **Copy:** diagnose with `dolt gc --full` before any rewrite, and keep a backup before
  one (`bd backup`).
- **Copy:** `bd`'s open request for a scheduled integrity check and a configurable
  timeout (#7255, #6034). tsk should give its push a retry and a timeout knob from the
  start.
- **Avoid:** `refs/dolt/data` (the cloud proxy rejects it), a daemon that pushes on a
  30 s interval (unofficial, unverified), and `bd flatten` as the standard fix. A
  history squash breaks other clones (shown above), and here it saved nothing over `gc`.
- **Avoid:** a push after every write. Beads' session-close push and `dolt.auto_push`
  being optional match tsk's T-08 rule. Growth is per push.
- **Note:** beads has no remote compaction command and its own users hit unbounded
  table-file growth (#6359). `tsk ledger compact` would be new ground, which is a reason
  to test it live once and to keep the threshold conservative.

## Options

Times are for a live `tsk ledger fetch` with nothing new unless stated. "Measured" means
run here. "Estimate" is arithmetic on measured parts.

| | Option | Cost to build | Benefit | Risk | Needs from Jim |
|---|---|---|---|---|---|
| A1 | T-01 to T-04 as planned | Already planned | `pause` about 10 s to about 4.5 s live-equivalent (estimate). No effect on session start. | Low | Nothing new |
| A2 | Set `protocol.version=0` for every git process `tsk` starts, through `GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_0` and `GIT_CONFIG_VALUE_0` in the child environment | One small change in the code that spawns `git` and `dolt` | Live fetch 2.65 s to 1.94 s (measured). Every network op costs about half. | Low. Protocol 0 advertises every ref, which is cheap here (33 refs in the repo). With thousands of branches the advertisement would grow. Only the processes `tsk` spawns are affected. | Yes or no. |
| A3 | Skip `dolt fetch` and the documents fetch when one `git ls-remote origin refs/heads/tsk/ledger refs/heads/tsk/docs` matches the heads seen at the last fetch. Fold `tsk`'s own `ls-remote` into it | A remembered head per store, and the skip in `Session::refresh` and `docs::fetch` | A read with nothing new falls to about 0.5 to 0.7 s on live (estimate): one `ls-remote` at 0.4 s (0.22 s on protocol 0) plus local work. No `cat-file` loop. Stays flat as the remote grows, for reads that find nothing new. | Medium. A wrong remembered head would hide new data. The head is the full commit SHA, so a match means identical data. Cleared by `discard` and `push`. | Approve the design, and say whether it goes before or after T-01. |
| A4 | Cache the self-update version check (at most once a day) and stop it running at session start | Small | 0.2 to 0.4 s per session start (measured round trip) | Low. A new version is noticed up to a day late. | Yes or no. |
| A5 | Push less often (T-08) | Planned | Remote growth is per push, so 20 writes in one push add 2 files instead of 40 (measured: 2 files per push regardless of writes) | Writes sit locally longer, which T-08 already weighs | Nothing new |
| B | `tsk ledger compact`: `dolt gc`, push to a temporary ref, then `git push --force-with-lease` onto `refs/heads/tsk/ledger`, then delete the temporary ref | A command, a lease-and-retry loop, a threshold check, and a test over stale clones | Remote tree 125 files to 3. Live fetch 2.65 s to about 1.9 s (emulated, measured), and the loop stops costing anything until the count grows again. With A2 about 1.2 s (estimate). Needs repeating: growth is 1 to 2 files per push. | Medium. It rewrites the branch tree. The lease stops it overwriting a push that landed after it read the tip, and the compacting clone must fetch and merge first. The proxy's reaction to a non-fast-forward update is untested. Dolt's own re-rooting at 64 commits will test it on live soon. | Approve rewriting `refs/heads/tsk/ledger`. Set the threshold and who runs it. Allow one supervised live compaction as the test. |
| C | Do nothing about the file count | None | None | A fetch grows by about 4.8 ms per file added in this environment: 1.8 s more at 500 files and 4.2 s more at 1000 files. The live ledger gained 18 files in about a day, so 500 files is about three weeks away. | Yes or no. |
| D | Squash the Dolt history | Medium | None beyond B (603,962 against 602,877 bytes after `gc --full`) | High. Every other clone diverges and must re-clone. | None. Rejected. |
| E | Ask Dolt to read each table file once (for example one `cat-file --batch`), to skip files the local store holds, and to avoid the second `git fetch` | An upstream issue and patch | Removes the loop, whatever the file count. Largest long-term lever. | Outside this repo. Timing is not ours. | File an issue upstream, and say who. |
| F | Server (DoltHub, `dolt sql-server`, relay) | High | Removes the git remote from the hot path | The proxy denies the hosts and, per the 2026-10-15 test, raw TCP and gRPC. Ruled out by Jim on 2026-10-18. | None unless the ruling changes. |
| G | Trim the cache clone's `refs/dolt/remotes/.../<uuid>` refs | Small | None measurable until thousands of refs (14 ms against 75 ms at 5000) | Low | None. Housekeeping only. |
| H | Raise the pinned Dolt from 1.88.2 to 2.5.0 or later (the `SessionStart` installer and the `CLAUDE.md` note) | A version bump, a test of the installer in the cloud sandbox and a conformance run | Launches per fetch 1362 to 456 (measured at 227 files). Live-equivalent fetch 3.5 s to 2.6 s at 227 files. The saving grows with the file count. Beads' fixes (#4770, #6359) come from the same upstream line. | Medium. A new major version of the storage engine. Mixed 1.88.2 and 2.5.0 clones worked in a scratch test. Not tested on the live ledger or on the cloud installer. | Approve the bump, and decide whether the minimum becomes 2.5.0 or stays "or later". |

Projected live `tsk ledger fetch` with nothing new:

| State | Seconds | Basis |
|---|---|---|
| Today | 2.65 | Measured |
| A2 | 1.94 | Measured |
| B | 1.89 | Measured (emulated delay) |
| A2 + B | about 1.2 | Estimate: 2.65 minus 0.74 minus 0.71 |
| A3 | about 0.5 to 0.7 | Estimate, not run |
| A3 + A2 | about 0.4 to 0.5 | Estimate, not run |

A3 makes the file count irrelevant for the common case. B and E matter when something
did change, because a fetch that finds new commits still reads the whole tree (a first
fetch after a change costs about 1.4 times the steady launches).

## Recommendation

Do A2, A4 and the already planned T-01 to T-04 first. They are small, reversible, and
cut a live fetch from 2.65 s to 1.9 s with no change to the stored data. Then build A3,
so a read that finds nothing new takes about half a second and no longer depends on the
file count. Add B (`tsk ledger compact`) behind a threshold of about 60 files, after one
supervised live run, so a fetch that does find changes stays near 1.9 s.

New after the beads research (section What beads does): raise the pinned Dolt (H). On
the same data Dolt 2.5.0 launches a third of the `cat-file` processes and takes 0.9 s less
per live-like fetch at 227 files. Beads' users reported the same defect and it was
fixed upstream. This is cheaper than compaction and is a version bump, so do it with
A2 and A4, ahead of B. Mixed versions across clones worked in the scratch test. B stays,
but its urgency falls, because H slows the growth of the cost per file from 6 to 2
launches. Beads has no remote-compaction command, so B is new ground. Test it live once.

Rank by value: A3 first (largest change for reads), then A2 and A1 (writes), then B (the
only fix for the scaling of a fetch that finds changes). E is worth filing in parallel
because it is the only fix that removes the cause.

This also settles the review's proposal 4. The remote can be compacted, but only by
writing to a fresh ref and moving the branch. A plain push after `gc` cannot do it.

## Action budget, current and projected

Source: `tsk ledger doc get missions/operational/M-PERF/action-budget.md` (read only).
Budgets there are proposals for Jim to change. Every projected figure below is an
estimate built from the measured parts in this document. None was run, except where the
Now column says measured.

Assumptions, each column adding to the one before:

- **Round trips only (R).** T-01 to T-04, T-08 (a write never waits for origin; publish
  runs in the background), A3 (skip the fetch when the remote heads match), A4 (no
  version check at session start). Protocol v2, Dolt 1.88.2, no compaction.
- **R + protocol v0 (V).** Each network operation costs 0.2 s, not 0.33 s (A2).
- **R + V + compaction (C).** The table-file pass costs 0.52 s locally, not 1.28 s (B).
  It only matters when a fetch finds something new, because R already skips the pass
  when nothing is new.
- Dolt 2.5.0 (H) is not in the columns. It would cut the table-file pass by about two
  thirds at 227 files, and so lowers the C column further when the remote has not been
  compacted. Treat it as an extra reduction on "something new".

Model: time = local work + network operations × 0.33 s (0.2 s on v0) + table-file passes
× 1.28 s (0.52 s compacted). A read with nothing new is one `ls-remote` plus local work.
"Something new" is one fetch pass of 5 operations. Live figures come from the
330 ms emulation or from measured live runs.

| Action | Needs origin | Budget (proposed) | Now | R | R + V | R + V + C |
|---|---|---|---|---|---|---|
| Read what is open | no | 0.2 s | 0.08 s | 0.08 s | 0.08 s | 0.08 s |
| Add a task | no | 0.3 s | 0.2 s | 0.2 s | 0.2 s | 0.2 s |
| Mark a task done | no | 0.3 s | 0.2 s | 0.2 s | 0.2 s | 0.2 s |
| Record a decision or answer a question | no | 0.3 s | 0.2 s | 0.2 s | 0.2 s | 0.2 s |
| Mark done and publish: the write | no | 0.3 s | 2.1 s local (about 6 s live) | 0.2 s | 0.2 s | 0.2 s |
| Mark done and publish: the publish, in the background | yes | 1 s or background | about 6 s live | 3.0 s | 2.2 s | 2.0 s |
| Publish changes (`tsk ledger push`) | yes | 1 s or background | 1.9 s local with a change; about 6 s live | 3.0 s | 2.2 s | 2.0 s |
| See what is unpublished (`tsk ledger status`) | yes | 0.3 s with a local-only mode | 1.75 s live (measured by the review), 2.3 s here | 0.65 s nothing new; 2.65 s new | 0.5 s; 1.94 s | 0.5 s; 1.2 s |
| Start a thread | yes | 1 s | 3.2 s local; about 10 s live | 3.5 s | 2.8 s | 2.6 s |
| Pause a thread | yes | 1 s or background | 3.2 s local; 10.1 s live-equivalent | 3.5 s | 2.8 s | 2.6 s |
| Resume a thread | yes | 1 s | 0.85 s local; about 2.5 s live | 1.5 s | 1.2 s | 1.1 s |
| Start a session | yes | 1.5 s | 3.5 to 4.4 s live | 1.0 s nothing new; 3.0 s new | 0.8 s; 2.2 s | 0.8 s; 1.5 s |
| Stop hook, every turn | no | 0.15 s | 0.09 s | 0.09 s | 0.09 s | 0.09 s |
| List threads | yes | 0.3 s as a local read | 3.4 s local, 4.1 to 6.6 s live | 3.4 s | 3.4 s | 3.4 s |

Reading the projections:

- Only the first four rows and the guard meet their budgets today and after every
  scenario. The write half of "mark done and publish" meets 0.3 s once T-08 moves the
  publish off the critical path. This holds in every column, because it needs no origin.
- The remote cost is the same in all three scenarios for anything that pushes. A push
  needs one `ls-remote` and two pushes, 5 network operations at 0.33 s or 0.2 s,
  plus local work of about 1.4 s. No scenario brings a synchronous publish, start or
  pause to 1 s. They meet the budget only as background work, or with a smaller `dolt`
  process count (T-02), which is why those rows land at 2 to 3.5 s.
- Session start meets 1.5 s only when nothing is new (R and later). When another session
  pushed since the last fetch, it needs V and C to reach 1.5 s.
- `tsk thread list` is not a remote problem. It launches 38 `dolt` processes (measured
  above), about 2.7 s at 70 ms each. None of the three scenarios changes it. It needs the
  batching `mission list` already has, or a local-read mode, as the budget file says.
- `ledger status` meets 0.3 s only as a local-only mode, as the budget file proposes. A
  remote check costs at least one `ls-remote` (0.2 to 0.4 s).

## Decisions for Jim

1. **Order of work.** Agree that round trips and the Dolt bump come before compaction:
   A2, A4, H, T-01 to T-04, then A3, then B. Or move B earlier.
2. **A2, `protocol.version=0`.** May `tsk` set it for the git processes it spawns?
3. **A3, skip when the remote heads match.** Approve the design (remember the full SHA of
   `tsk/ledger` and `tsk/docs` after each fetch, one `ls-remote` for both refs, skip the
   fetch on a match). Say whether it replaces T-01's skip-the-second-fetch design or sits
   beside it.
4. **A4, self-update check.** May the version check move out of session start and run at
   most once a day?
5. **B, compaction.**
   - May `tsk` rewrite the tree of `refs/heads/tsk/ledger` with `--force-with-lease`?
   - What threshold (file count) and what trigger (a session-end step, a scheduled task,
     or a manual command only)?
   - May one compaction run on the live ledger as a supervised test? It is the only way
     to confirm the cloud proxy accepts a non-fast-forward update on a branch.
6. **E, upstream.** Should we file an issue against Dolt for the per-file `cat-file`
   loop, and who files it?
7. **F, server.** Confirm it stays ruled out. If not, adding `doltremoteapi.dolthub.com`
   or a self-hosted host under Allowed domains is the first step, and gRPC through the
   proxy still has to be tested.
8. **Corrections to the review.** May the review's Performance section carry a note
   pointing to this document, for the five contradictions listed above?
9. **Action budget.** Set the target per action in the Action budget section. It shows
   which rows no scenario can bring to the proposed budget as a synchronous call, so
   the choice is between loosening those budgets and running them in the background.
10. **Dolt version (H).** Raise the pinned Dolt to 2.5.0 or later? Say whether the minimum
    changes in `CLAUDE.md` and the installer.
11. **Beads reports.** The open issues #6359 and #7082 show the same growth in other
    users' remotes. Should tsk follow them, or file a link from the upstream Dolt issue
    (decision 6)?

## Commands run

All in `$S` (the scratch directory) with `. $S/env.sh`. `$S/bin2` holds the C wrapper.

```bash
# --- Finding 1 ---
# live cost of git operations (bare repo with the real URL)
git init -q --bare $S/live.git && cd $S/live.git && git remote add origin https://github.com/jimbarritt/tsk
git fetch --no-tags origin +refs/heads/tsk/ledger:refs/dolt/x/1       # repeat for timing
git fetch --no-tags origin refs/heads/tsk/docs
git ls-remote --heads origin
GIT_TRACE_CURL=$S/curl.txt tsk ledger fetch                           # in $S/live1; count requests and bytes
strace -f -tt -e trace=execve,connect -o $S/strace_live.txt tsk ledger fetch   # in $S/live1
# interleaved timing, live against local against delayed local (run in a loop, 6 times)
(cd $S/live1 && tsk ledger fetch)
(cd $S/mA    && tsk ledger fetch)
(cd $S/mB    && GITWRAP_DELAY_MS=330 PATH=$S/bin2:$PATH tsk ledger fetch)
# launch timeline
GITWRAP_LOG=$S/lg.txt PATH=$S/bin2:$PATH tsk ledger fetch; sort -n $S/lg.txt
# --- Finding 2 ---
tsk ledger path; ls -la $(tsk ledger path)/.dolt/git-remote-cache/*/repo.git
git ls-tree -r -l refs/remotes/origin/tsk/ledger                      # tree and sizes
dolt admin archive-inspect <file>.darc ; dolt admin storage
bash $S/scale.sh      # copies origin.git, adds 1, 10, 50 pushes from a writer clone, measures a reader and a fresh clone
# --- Finding 3 ---
cp -a $(tsk ledger path) $S/gcs/db && cd $S/gcs/db
dolt gc ; dolt gc --full ; dolt gc --full --archive-level=0
dolt remote add --ref refs/heads/tsk/ledger-gc fresh git+file://$S/origin_gc.git
dolt push fresh main
git -C $S/origin_gc.git update-ref refs/heads/tsk/ledger refs/heads/tsk/ledger-gc
$S/measclone.sh $S/origin_gc.git gc-push-fresh                        # fresh clone, 4 fetches, launches and time
dolt push --force origin main                                         # in a gc'd clone, same branch
dolt reset --soft <first-commit> ; dolt commit -m squashed ; dolt gc --full   # squash test
git -c protocol.version=0 fetch --no-tags origin +refs/heads/tsk/ledger:refs/dolt/x/9
GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.version GIT_CONFIG_VALUE_0=0 tsk ledger fetch
DOLT_DISABLE_EVENT_FLUSH=1 tsk ledger fetch
# --- Finding 4 ---
curl -sS -o /dev/null -w '%{http_code}' https://www.dolthub.com/      # also doltremoteapi.dolthub.com, dolthub.com
curl -sS "$HTTPS_PROXY/__agentproxy/status"
# --- Finding 5 ---
PATH=$S/bin2:$PATH GITWRAP_LOG=$S/n.txt strace -f -e trace=execve -o $S/n_strace.txt <command>
awk '{print $3}' $S/n.txt | sort | uniq -c
GITWRAP_DELAY_MS=330 tsk thread pause <thread> "" "bench"             # scratch origin only
```

Total scratch use: about 12 bare-repo and clone copies of the ledger, each under 3 MB.
