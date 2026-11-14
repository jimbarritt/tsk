# Territory and nexus

Status: concepts revised 2026-10-16; several mechanics open. Source: 2026-09-14
bootstrap design session, revised by the analysis in
[personal-and-team-territories.md](personal-and-team-territories.md). Term definitions are in
[docs/domain/ubiquitous-language.md](ubiquitous-language.md#territory); this file
contains the rationale.

## Revised 2026-10-16

The section below records the first design. Two parts of it changed:

- A territory is a nexus-local overlay over repos or missions, not a bounding area. The
  same repo can be in several territories, one per nexus that indexes it. A territory
  lists its missions, and a mission does not name its territory.
- A nexus is optional and is shared by a group of actors. A personal nexus is an
  optional extra, and a nexus repo may contain a ledger.

The analysis, with an example and the open items, is in
[personal-and-team-territories.md](personal-and-team-territories.md).

## Why two separate concepts

Territory and nexus were fused in the first version of this design, then separated:
a territory is a bounding area, where isolation is defined; a nexus is only the
discovery and routing mechanism, with an index of projects and links to other
nexuses. A nexus does not define a boundary; the territory does. Isolation, whether a
link may be followed across a territory boundary, is a territory property, not a
nexus one.

A consequence: a tsk install does not have a fixed list of roots. It connects to one
nexus, and the wider network becomes available by following links from there. Each
repo keeps its own ledger; the nexus only records where to find it.

## A nexus can contain ledgers

A nexus is a git repo with a `nexus.json` at the root of its default branch. A tsk install
attaches one with `tsk config attach-nexus <url>`, which writes the user config.

A managed repo's entry in `nexus.json` sets where its ledger lives. With
`"ledger": "nexus"` the ledger is the branch `refs/heads/ledgers/<repo-id>` in the nexus repo. With
`"ledger": "repo"`, or no `ledger` field, or no entry, the ledger is the branch
`refs/heads/tsk/ledger` in the managed repo. The layout and the resolution rules are in
[ledger-layout.md](ledger-layout.md#location-and-ref).

An entry has these fields:

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | The repo ID, `[a-z0-9][a-z0-9-]*`. Names the ledger branch and is stored in the ledger's manifest as `repo_id`. |
| `url` | no | The repo's clone URL. tsk matches it, normalised, against the managed repo's `origin`. |
| `local` | no | A machine name. The entry is visible only on that machine and is found only through the repo ID cached in the clone. For a repo with no `url`. |
| `ledger` | no | `"repo"` or `"nexus"`. Absent means `"repo"`. |
| `ledger_refs` | no | `"branches"` or `"custom"`: which refs the repo's ledger uses. |
| `prefix` | no | The mission prefix of the repo's ledger, 3 or 4 capital letters. No two repos in a nexus share one. |
| `workflow` | no | `"main"` or `"pull-request"`: how the repo takes changes. Absent means the repo has no recorded answer. |

`nexus.json` has this shape:

```json
{
  "version": 1,
  "territories": [
    {
      "id": "work",
      "name": "Work",
      "repos": [
        {
          "id": "billing",
          "url": "https://example.test/acme/billing",
          "ledger": "nexus",
          "ledger_refs": "branches",
          "prefix": "BIL",
          "workflow": "pull-request"
        }
      ]
    }
  ]
}
```

Commands under `tsk nexus` edit the file on the nexus's default branch, commit and push
with the machine's git credentials:

| Command | Edit |
|---|---|
| `register-repo` | Adds this repo's entry. |
| `move-ledger` | Sets an entry to `"ledger": "nexus"`. |
| `add-territory <id> [--name <name>]` | Adds an empty territory. An existing id changes nothing. |
| `rename-territory <id> --name <name>` | Changes the display name. |
| `remove-territory <id>` | Removes a territory that has no repos. |
| `set-repo <repo-id> [--workflow main\|pull-request] [--prefix <PREFIX>] [--url <url>]` | Sets fields of an entry. A prefix that another repo uses is an error. |
| `move-repo <repo-id> --territory <id>` | Moves an entry to another territory. |
| `remove-repo <repo-id> [--force]` | Removes an entry. An entry whose ledger is in the nexus needs `--force`. |

Each command prints one line that names the change, or `unchanged`, then the commit that
it pushed.

## Why a coordination repo, not a GitHub feature

- GitHub custom properties are restricted to five value types, must be printable
  ASCII without double quotes, are scoped to one organisation, and need organisation
  owner permission to define. Their purpose is governance, not application data.
- GitHub Projects can span repositories but have their own data model.
- Nothing native provides an arbitrary, writable, cross-organisation metadata store.

## Comparison with Gastown

- Gastown is multi-repo, not a monorepo. Each managed project is a rig, and each rig
  is its own git repository.
- Gastown has a separate town repo for configuration and cross-rig coordination.
- Gastown keeps cross-rig coordination in town-level beads with an `hq-` prefix, and
  project work in rig-level beads.
- The town has a `routes.jsonl` routing table from ID prefix to rig. That file is
  the cross-repo index, the nearest existing equivalent to a nexus.
- Wyvern, cited as a Gastown example, is Steve Yegge's game project: one of the
  rigs, not a repo structure in its own right.

## Open

- Where the missions live in the tsk repository once cloud sessions must read them:
  a directory, or a git ref. (Resolved for the bootstrap: a directory,
  `~/.planning/tsk/missions/` initially, moving into the repository before the first
  cloud test, per M-BOOT-02.)
- What the nexus index contains per project beyond the entry fields above.
- Link direction: whether a link from nexus A to nexus B implies that B lists A, or
  links are one-way and a wider view is a traversal from wherever you start.
- How the active territory filter is expressed in the command line interface.

Expected territories, from the bootstrap sequence: work, personal, Ubiqtek, agentic
research. Territories are independent of GitHub organisations.
