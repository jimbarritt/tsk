# Future missions, TBD

Ideas for missions that have not been shaped into briefings yet. Recorded so they are
not lost. Listing an idea here implies no commitment, no ordering and no decision that
it is a good idea.

## Agents interrupting each other

Raised by Jim, 2026-09-15.

Agents should be able to interrupt each other. The mechanism suggested is tsk running a
peer to peer "gossip" protocol that lets agents send each other messages.

Open: whether this is needed at all, given that agents can already communicate through
git. Recorded as an idea, not a direction.

## Generalising the harness's cross-repo needs

Raised by Jim, 2026-09-15, alongside M-BOOT-02 T-06.

tsk's `CLAUDE.md` now carries a tsk-specific instruction: attach `ksobr-transcripts`
with `add_repo` before pushing a transcript at session end. That is a stopgap. Once the
harness is meant to run in more than one repo, a harness needs a general way to declare
"this repo also needs this other repo attached" that does not mean hand-writing the
instruction into every consuming repo's `CLAUDE.md`.

Open: what that declaration looks like, and whether it can be read by a `SessionStart`
hook at all, given `add_repo` is an MCP tool call the agent makes, not something a bash
script can invoke.
