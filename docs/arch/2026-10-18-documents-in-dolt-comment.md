# Comment: documents back in Dolt?

Date: 2026-10-18. A comment on an idea Jim floated, not a plan. Answers M-PERF Q-01.

The idea: the ledger syncs two branches, `tsk/ledger` (Dolt data) and `tsk/docs`
(Markdown documents). Every sync is two syncs. Should documents go back into Dolt as
long-text rows, so the ledger is one branch with one sync?

My view in one line: the second branch is the small cost, and the second sync of each
branch is the large one. Fix the large one first; after that the branch costs almost
nothing, and moving documents into Dolt would buy under a second per push that carries a
document, at the price of files, line merges and a second migration in a week.

## 1. Where the time goes

Measured on the live origin through the cloud proxy, 2026-10-18:

| Piece | Live cost | Source |
|---|---|---|
| One git connection | about 0.35 s | `git ls-remote` of both refs, 0.35 to 0.58 s |
| A `git fetch` of `tsk/docs` | 0.61 to 0.66 s | one connection plus negotiation |
| A `dolt fetch` over the git remote | about 1.7 to 1.9 s | `tsk ledger fetch` at 2.3 to 2.5 s minus the docs fetch; several `git fetch` and about 420 `git cat-file` processes |
| A second branch in the same connection | about 0 s | both refs in one `git fetch`, 0.35 s |

A thread command such as `tsk thread pause` runs: two `dolt fetch` (`refresh_data`,
then `push_data` again), two docs fetches (`docs::fetch` in `refresh_with`, then
`fetch_tip` at the top of `push_worktree`), one code-repo fetch, a `dolt push` and about
13 `dolt sql`. Live, that is about 3.6 s of Dolt fetch, 1.2 s of docs fetch, 0.6 s of
code fetch, about 1 s of push and 2.5 s of SQL: roughly 9 s, against 3.2 s on a local
origin.

So the split by cause:

- The documents branch as such: one docs fetch per sync, 0.6 s. Two per thread command,
  1.2 s. About 13 percent of the command.
- Syncing each branch twice in one command: one Dolt fetch and one docs fetch, 2.4 s.
- The Dolt fetch itself, driven by the remote's table-file count: 1.8 s each time, and
  the one cost that grows.

The docs push is already conditional. `push_worktree` returns `AlreadyOnRemote` when
HEAD is on the remote, so a push with no document change sends nothing. It still pays the
`fetch_tip` to find that out.

## 2. What documents in Dolt would save and cost

Saved: the docs fetch, 0.6 s per sync, and the docs CAS push when a document changed,
about 0.6 to 1 s. Per thread command today: 1.2 s of 9 s. The ordering in
`commit_and_push` (documents first, then `check_links`, then data) goes away, and a
record and its document arrive in one Dolt commit. The link check could become a foreign
key from `mission_documents.path` to the documents table, enforced by Dolt and reported
through `dolt_constraint_violations` at merge. That is the one design gain I would miss.

Cost, concretely:

- Merge behaviour. Dolt merges by cell. Two sessions that each edit `design.md` conflict
  as a whole document, and tsk's push then aborts the merge and exits 1 naming the table.
  Resolution is `ours` or `theirs`: one side's edit is lost, or an agent reconstructs it
  by hand from `dolt diff`. On `tsk/docs` the same two edits rebase by line, and conflict
  only when the same lines moved. The documents that get repeated edits are exactly the
  shared ones: the M-TSK-SCHEMA briefing had 17 of the branch's 34 commits, `design.md`
  6, `future-missions-tbd.md` 9. The 30 daily vulnerability reports are new files and
  never conflict either way. `standing-notes.md` is appended, and two appends at the end
  conflict in git too. The history has no merge commit, so no two sessions have yet
  changed one document at the same time. The risk is in front, not behind, and it grows
  with the number of concurrent sessions, which is the direction of tsk.
- Size and growth. Documents are 388 KB against 955 KB of table files. Dolt stores a
  long text as a tree of chunks of a few kilobytes, so one edit writes the changed chunks
  and the path to the root, not the whole body. Bytes per push rise by a few kilobytes to
  tens of kilobytes. The real cost of the remote is its file count, which grows with
  pushes, not bytes, and documents in Dolt do not change the number of pushes. So the
  growth problem gets no worse in the dimension that costs time. It gets worse in the other:
  every fresh clone's `dolt fetch` streams the whole document history through
  `git cat-file`.
- Files for tools. `tsk ledger doc path` could write the row to a cache file on demand.
  Lost: GitHub renders `tsk/docs` today as a branch of Markdown; an edit in marq or an
  editor is picked up by the next `tsk ledger push` without a `doc put`, because the
  worktree is the truth; `git log` and `git blame` on one document; file-level lines in
  `tsk ledger status`. A write-back cache that notices an external edit is a small sync
  engine, and it is the one tsk just deleted.
- Reversibility. `migrate_docs.rs` moved documents out at layout 3, with a reverse path
  nobody wrote. Moving them back is another layout, another `migrate` command, and every
  clone upgrades again. The storage seam makes it possible. Two flips in one week is
  churn that users of the ledger would notice.

## 3. The same saving without moving documents

| Option | Saves, live, per command | Effort | Risk |
|---|---|---|---|
| T-01: one fetch per command. Keep the remote tips from the refresh and skip the second `dolt fetch` and the second `fetch_tip` | 2.4 s on every thread command and on `ledger push` | small | a few seconds of staleness between refresh and push; the CAS push and Dolt's non-fast-forward rejection still catch a moved remote |
| T-09: one `git ls-remote` of both refs before any fetch, skip both fetches when the tips match | about 2 s on session start, `status`, `thread list` and the pre-push fetch, when origin has not moved, which is the common case for one user | small | none for correctness; the fetch or the push finds a remote that moved in between |
| Lazy docs fetch: fetch `tsk/docs` only when `doc get/put/path/list`, `mission link` or the push-time link check needs it, and the ls-remote says it moved | 0.6 s on every command that touches no document | medium, state threaded through `Session` | `doc get` must fetch first or it reads stale text; `check_links` at push must too |
| One `git fetch` for both refs | 0.35 s at most, and only if `dolt`'s git remote reads from a clone tsk can fetch into first | unknown | depends on Dolt internals |
| Push docs only when changed | already so; with T-01 its remaining `fetch_tip` goes too | none | none |

With T-01 and T-09 in place the documents branch costs nothing on a read, because the
ls-remote covers both refs in one connection, and nothing on a push that changes no
document. Documents in Dolt would then save under a second, and only on a push that
carries a document edit.

## 4. Recommendation

Keep the documents on `tsk/docs`. The right lever is the number of round trips per
command, not the number of branches.

Order of work: T-01, then T-09, then T-02 (fewer `dolt sql`) and T-03 (code fetch only
when needed), then T-08 (push off the critical path). The lazy docs fetch is a small
follow-on to T-09 and is worth doing when its 0.6 s still shows.

Action budget rows each option moves: T-01 moves `thread start`, `thread pause` and
`publish changes`. T-09 moves `start a session`, `see what is unpublished`,
`list threads` and the fetch before a publish. T-08 moves `mark a task done and publish`.
Documents in Dolt moves `publish changes` by under a second when a document changed, and,
before T-09 only, the three read rows by 0.6 s.

Measure first, if Jim wants data: a `TSK_TRACE` environment variable that prints each
`git` and `dolt` launch with its duration, run on the live origin for `thread pause`,
`ledger push` with no change, with a record change and with a document change, and
`session-start`. That turns the derived split above into a measured one in an hour.

What would change my mind: the trace shows a docs fetch far above 0.6 s live; documents
become state that tsk parses (then they are data and belong in rows by the core
principle); or two-session edits of one document turn out to be rare in practice and the
atomic record-plus-document commit is needed for a real bug. The one thing to protect in
any move: a document is a file a tool can open and edit, and `git log` can follow.

## Summary

- The docs branch costs 0.6 s per sync; syncing each branch twice costs 2.4 s per command.
- Documents in Dolt save 1.2 s now, under 1 s after T-01 and T-09, and only on pushes with a document.
- They cost whole-document conflicts, the files tools open, and a second migration in a week.
- Do T-01, T-09, T-02, T-03, T-08; then a lazy docs fetch if 0.6 s still shows.
- Measure with a per-process trace on the live origin before deciding anything else.
