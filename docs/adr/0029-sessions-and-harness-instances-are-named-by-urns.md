# 29. Sessions and harness instances are named by URNs in the tsk namespace

Date: 2026-10-30

## Status

Accepted. Decided by Jim on 2026-10-30 (mission M-TSK-ATTRIB, tasks T-10 and T-11).
Refines decision 6 of that mission, which replaced the `Claude-Session` trailer with one
named for an agent session.

## Context

A commit made in a Claude Code cloud session carries the trailer
`Claude-Session: https://claude.ai/code/session_<id>`. The trailer is the trace from a
commit back to the session that made it. ksobr will read it. It has three faults:

- The key names one vendor. tsk runs under more than one harness: Claude Code in the
  cloud, Claude Code on a machine, Copilot CLI today, and others such as pi later.
- The value is a URL that only a cloud session has. A CLI session has an ID and no URL.
- A session ID is minted by its harness and is unique only within that harness. Claude
  Code cloud mints `cse_…`, Claude Code CLI mints a UUID, Copilot mints its own. Two
  harnesses can mint the same string, so a bare ID is ambiguous for ever.

tsk also names a harness instance (see `docs/domain/ubiquitous-language.md`) with a key:
the cloud session ID, or `<clone-id>/<worktree-name>` for a code worktree. An older URN,
`urn:tsk:cloudsession:<id>` or `urn:tsk:worktree:<name>`, survives as the fallback value
of `written_by` in a continuation entry. It predates the term "harness instance", and its
worktree form holds no clone ID.

A commit trailer lives as long as the history. The form chosen here cannot change later
without a rewrite.

## Decision

1. **A session is named by a URN of four fixed segments:**

   ```
   urn:tsk:session:<harness>:<id>
   ```

   `<harness>` names the harness that minted the ID. `<id>` is the ID as that harness gives
   it, and may itself hold colons: a reader splits on the first four colons only.

2. **A harness instance is named by a URN of the same shape:**

   ```
   urn:tsk:instance:<kind>:<key>
   ```

   `<kind>` is `cloud` or `worktree`, the two kinds the `instances` table holds. `<key>` is
   the instance key: the cloud session ID, or `<clone-id>/<worktree-name>`. This form
   replaces `urn:tsk:cloudsession:<id>` and `urn:tsk:worktree:<name>`. A reader accepts the
   old forms, because stored continuation entries hold them.

3. **The commit trailer is `Agent-Session`, and its value is the session URN.**

   ```
   Agent-Session: urn:tsk:session:claude-cloud:cse_01Sw1xdrmM7YfnCqiZGrJ48L
   ```

   It replaces `Claude-Session` on new commits. The rewrite of `main` (M-TSK-ATTRIB T-08)
   replaces it on the commits before the cutoff.

4. **The prefix and the segment count are fixed. The set of harness names grows.** A new
   harness adds a name. No old commit changes. The names in use are recorded in
   `docs/domain/ubiquitous-language.md` under the term harness.

5. **The reverse lookup from a session to its harness instance is a ledger record.** A
   cloud session ID is also its instance key, so that case needs no record. A code
   worktree runs many sessions, one for each start and each `/clear`, so the `SessionStart`
   hook records the session URN, the instance key and the start time. That record is a
   layout change and a task of its own.

## Consequences

- The trailer and the URNs name no vendor. The same form serves Claude Code, Copilot, pi
  and a harness that does not exist yet.
- The URN tells a reader which harness to ask about an ID, without a table.
- A person cannot open the URN as a link. tsk derives the URL for a cloud session on
  request. The link is derived, not stored.
- The old `Claude-Session` URL holds `session_<id>`, and the environment of the same
  session holds `cse_<id>`. On the one session checked, the suffix is the same. The
  rewrite (T-07) confirms this on every commit before it maps one to the other.
- `urn:tsk:instance:` uses the word "instance" alone, where the domain term is "harness
  instance". The namespace `urn:tsk:` supplies the qualifier, so the segment stays short.
- The harness names are the next decision. The candidates are `claude-cloud`, `claude-cli`,
  `copilot-cli` and `pi`.
