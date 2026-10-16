# Personal and team territories

Status: analysis, 2026-10-16, with three decisions recorded below. The rest is open. The terms are those in
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
| A mission briefing had a Territory row, removed 2026-10-16 | `mission-briefing-template.md` |
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
- Before the row was removed, the Territory row in a briefing was free text that nothing
  in the binary reads. Existing briefings in the ledger keep the row. It is harmless.
- The trial tests the repo boundary only. Requirements 2 and 3 are not needed for it.

## Decision 2026-10-16: personal and team nexuses are decoupled

Jim, 2026-10-16, on the first open decision:

> I think it needs to be decoupled between personal and team. Not a forced thing otherwise
> everyone needs a personal nexus repo to even participate. It's an optional extra you can
> do if you want to use tsk for things outside of a specific work repo/ territory
>
> Maybe if you do create a personal nexus it can have links to others so perhaps that's a
> kind of general graph feature of the nexus. Something like related nexus list - but it's
> the local config that decides which to show in say mission control.

The shape that follows combines options C and D:

| Where | What it holds |
|---|---|
| A nexus, in git | A related-nexus list. The graph. A property of every nexus, team or personal |
| The local config | The list of nexuses this install attaches, and which of them a surface such as Mission Control shows |
| Nothing | A personal nexus is optional. The repo-only path is the entry point, and nobody needs a nexus to work in a repo |

The config holds a list in any case: an actor in team A and team B with no personal nexus
attaches two. A cloud session reads the config of its environment, so the attached list
is set per environment, by the `SessionStart` hook or by hand. The graph in git is
visible to any session that reaches one nexus.

Not built: the related-nexus list, a config list with more than one URL, and a show
filter. `tsk nexus add` replaces the one URL today.

## Decision 2026-10-16: territory is an overlay, and a nexus repo holds a ledger

Jim, 2026-10-16, working an example with a repo shared by team A and team B:

> I think the problem is that territory is a kind of nexus local concept that groups
> repos. So the same repo can be in multiple trrritotires depending on your perspective.
>
> One additional factor - what if a mission has no repo? Or is cross cutting multiple
> repos?

and, on the answer:

> Yes, record it in both. And I think the thing about territories is correct, it's an
> overlay over the missions so territories know their missions but not the other way
> round

Two decisions:

1. **Territory is a nexus-local overlay.** A nexus defines a territory and groups repos or
   missions under it. The same repo or mission can be in several territories, one per
   nexus that indexes it. A territory knows its missions. A mission does not know its
   territory. The briefing's Territory row is removed, because a mission stores no
   territory.
2. **A nexus repo holds a ledger of its own.** It is for missions that belong to no
   single repo, or cut across several. A team's cross-repo mission goes in the team
   nexus's ledger. An actor's cross-cutting work goes in their personal nexus's ledger.
   This settles where a personal nexus holds its work.

Example, with R1 shared by teams A and B, R2 the actor's own repo, and R3 a team A repo:

| Nexus | Territory | Repos |
|---|---|---|
| Team A | `work` | R1, R3 |
| Team B | `client-x` | R1 |
| Personal | `collaborations` | R1 |
| Personal | `own` | R2 |

Mission M-1 is in R1's ledger. Through team A's nexus it is in `work`, through team B's in
`client-x`, and through the personal nexus in `collaborations`.

Cost: a mission in a nexus repo's ledger has the nexus's access boundary, not a repo's.
The whole nexus reads it.

Not built: the ledger on a nexus repo and a territory that lists missions. A mission
model has no repo field. A mission has a repo only through the ledger it sits in.

Updated on 2026-10-16: the glossary entries for territory and nexus,
`territory-and-nexus.md`, and the briefing template (the Territory row is removed, in the
template and in its worked example) match these decisions.

## Open

- Whether a territory lists repos, as `nexus.json` does today, or missions directly, or
  both. Jim's statement is that a territory knows its missions.
- A thread in the personal ledger that works a task in another repo's ledger. No
  cross-ledger reference exists.
- Isolation across a link, which the model gives to the territory, has no mechanics.
- The `Actor definition` idea in the ledger's future missions overlaps the personal
  nexus: an actor with a nexus of their own, skills and memory of their own.
