# Personal and team territories

Status: analysis, 2026-10-16. Nothing here is decided. The terms are those in
[ubiquitous-language.md](ubiquitous-language.md) and the rationale in
[territory-and-nexus.md](territory-and-nexus.md). The mechanics are those in
[ledger-layout.md](ledger-layout.md).

Raised by Jim, 2026-10-16, ahead of a trial with a collaborator. Two requirements and one
question:

1. tsk must operate without a nexus. Work starts in a repository, and a nexus is added
   on top if wanted.
2. A nexus is for a group of actors. Team A has a nexus. Team B has another. Both teams
   collaborate on one repo. An actor in team A sees team A's context and not team B's,
   and has no access to team B's nexus.
3. An actor also has work of their own: tasks that cut across territories, and threads
   in no territory. Where does that work live, and how does a nexus relate to a
   territory? Jim's candidate answer: a team nexus and a personal nexus, and tsk holds
   the list of nexuses.

## What the model holds today

| Fact | Source |
|---|---|
| With no nexus attached, the ledger is in-repo, on `refs/heads/tsk/ledger` | `ledger-layout.md`, "Choosing the location", rule 1 |
| A nexus is a git repo with `nexus.json`. A tsk install attaches one, by URL, in the user config | `territory-and-nexus.md` |
| `nexus.json` nests repos inside territories. A repo entry is in one territory | `nexus.json` example in `new-nexus-on-a-clean-machine.md`; `NexusIndex` in `cli/src/ledger/nexus.rs` |
| A repo's ledger is in-repo by default, or in the nexus with `"ledger": "nexus"`, for a repo the operator cannot push a branch to | `ledger-layout.md`, "Location and ref" |
| A nexus holds links to other nexuses, and a tsk install reaches the wider network by following links. Whether a link may be followed across a territory boundary is a property of the territory | `territory-and-nexus.md`; `ubiquitous-language.md` |
| A mission briefing has a Territory row | `mission-briefing-template.md`, line 31 |
| An actor is whoever holds a thread: a human, who holds many, or an agent session, bound to one | `ubiquitous-language.md`, "Actor" |
| Nexus links are not built. `tsk nexus` has `add`, `list` and `register-repo`. The config holds one URL | `tsk nexus --help`; `ledger-layout.md`, "Attaching a nexus" |

Requirement 1 holds today. This cloud session runs with no `config.toml` and no nexus,
and reads and writes this repo's ledger in-repo. The collaborator trial needs no nexus.

## Two access boundaries

The scenario has two boundaries that the current model treats as one.

| Boundary | Who is inside | What it protects | Mechanism today |
|---|---|---|---|
| Repo | Everyone with access to the repo | The code, and the ledger when it is in-repo | Git hosting permissions |
| Nexus | A group of actors | The index: which repos, which territories, and any ledger held in the nexus | Git hosting permissions on the nexus repo |

A shared repo with an in-repo ledger sits inside both teams' repo boundary. Each team's
nexus indexes it. Neither team reads the other's nexus. That is the shape Jim describes,
and the current model allows it, with one exception: a repo whose ledger is held in a
nexus is readable only by that nexus's members. For a repo two teams share, the ledger
belongs in the repo. The nexus-held ledger stays what `ledger-layout.md` says it is for:
a repo the operator cannot push a branch to.

Consequence: ledger access follows repo access. Nexus access follows group membership.
A design that keeps those apart keeps the scenario simple.

## Where a territory is defined

Today a territory is a list inside one `nexus.json`, and a repo is nested inside it. Two
facts follow:

- A territory cannot span nexuses.
- A repo is in exactly one territory, in the one nexus that indexes it.

Both break in the scenario. Team A's nexus puts the shared repo in territory `work`.
Team B's puts it in `client-x`. Each is correct for its own actors. So a territory is a
property of the pair (nexus, repo), not of the repo. The mission briefing's Territory row
stores the label in the shared ledger, which makes one team's label the record for both.
The row is a projection of the reader's nexus, not a stored field. That is the one place
the earlier fusion of territory and nexus is still visible.

## Where an actor's own work lives

Jim's own work has two parts, and each needs a home:

| Work | Home today | Gap |
|---|---|---|
| Threads on a repo's missions | That repo's ledger | None |
| Tasks that cut across territories, and threads in no territory | None. A ledger is a branch in a managed repo or a nexus-held branch named by a repo ID | No ledger exists without a repo |

A personal nexus repo closes the gap. It is a git repo, so it can hold an in-repo ledger
of its own, on `refs/heads/tsk/ledger`, for the actor's cross-cutting missions and
threads. The nexus-held form, `refs/heads/ledgers/<id>`, also works with a synthetic ID
and no `url`. The in-repo form needs no new rule.

## Options for the personal space

| Option | What it is | Finding |
|---|---|---|
| A. One nexus with a personal territory | The team nexus gains a territory per actor | The personal territory is visible to the whole team, and the external project is still in a nexus the actor is not a member of. Fails requirement 2 |
| B. Personal nexus as local config only | A list of territories and repos in `config.toml` | A cloud session clones repos and reads nothing from the laptop, so the list is not shared. No ledger, so no Dolt. Jim ruled it out |
| C. A list of nexuses in the user config | `config.toml` holds several URLs: personal, team A, team B | Reaches every nexus. The list is on one machine, so a cloud session needs the same list in its environment, by hand |
| D. One attached nexus, the personal one, with links | The personal `nexus.json` links to the team nexuses the actor belongs to, and indexes the external repo directly, without a link to team B's nexus | The same reach as C. The list is in git, so every clone and every cloud session of the actor's reads it. It is the mechanism `territory-and-nexus.md` already names, and it is not built |

C and D hold the same information. D stores it where cloud sessions read it, which is the
reason Jim gave for ruling out B. D also keeps one rule in the binary: attach one nexus,
follow links. Under D, "a team nexus and a personal nexus" is the same object type twice,
with different membership.

Under D, the external repo of requirement 2 is an entry in the actor's personal nexus with
its own territory, such as `collaborations`, and its ledger in-repo. The actor never
reads team B's nexus. Team B's nexus also indexes the repo, in its own territory.

## What a cloud agent attaches

An agent session is an actor. Its nexus is whichever URL its environment carries, by the
`SessionStart` hook. For an actor's own cloud sessions, that is the personal nexus. For
a team's shared cloud environment, that is the team nexus. The binary has one rule in
both cases.

## Effect on the trial

- No nexus. The collaborator clones the repo. Both parties write the in-repo ledger
  through `tsk`, and `tsk ledger push` serialises writes at push time.
- The Territory row in a briefing is free text with nothing to validate it. Leave it
  blank or agree a label. Nothing in the binary reads it.
- The trial tests the repo boundary only. Requirements 2 and 3 are not needed for it.

## Open

Three items need a decision:

1. One attached nexus with links (D), or a list of nexuses in the config (C).
2. Whether territory becomes a property of the (nexus, repo) pair, and the briefing's
   Territory row becomes a projection.
3. Whether the personal nexus repo holds the actor's own ledger in-repo, or as a
   nexus-held ledger with a synthetic ID.

Three items are open with no decision needed yet:

- A thread in the personal ledger that works a task in another repo's ledger. No
  cross-ledger reference exists.
- Isolation across a link, which the model gives to the territory, has no mechanics.
- The `Actor definition` idea in the ledger's future missions overlaps the personal
  nexus: an actor with a nexus of their own, skills and memory of their own.
