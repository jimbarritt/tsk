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
