# SpiceDB

Status: exploratory research, no decision made. 2026-10-01.

Question: whether SpiceDB can store the permissions attached to a tsk
[post](../domain/ubiquitous-language.md#post).

Sources: the SpiceDB [README](https://github.com/authzed/spicedb) and the AuthZed
documentation pages listed under Sources. Pages were fetched as raw HTML and read as
text. Quoted phrases were matched against that text. Claims about scale and maturity are
AuthZed's own and were not run independently.

## What it is

SpiceDB is an open source authorization database written in Go, built by AuthZed and
licensed Apache 2.0. The README calls it "the most mature open source project inspired by
Google's internal authorization system: Zanzibar". It answers one question: "can subject
X perform action Y on resource Z?" It also answers "What can `subject` do?" and "Who can
access `resource`?" (reverse indexes). It handles authorization only and is agnostic to
authentication.

Reported by AuthZed: in production at AuthZed since 2021, "5ms p95 when scaled to
millions of queries/s, billions of relationships". Users named in the README include
IBM, Red Hat and GitPod.

## Model

| Concept | Meaning |
|---|---|
| Schema | Object type definitions, the relations between them, and permissions computed from those relations. Written in the `.zed` schema language. |
| Definition | An object type, such as `user`, `team` or `document`. |
| Relation | A named connection a definition allows, to one or more subject types. |
| Relationship | The data: one instance of a relation, written `document:readme#editor@user:emilia`. |
| Subject relation | A relationship to a set, such as `team:engineering#member`: every member of the team. |
| Permission | A computed expression over relations. Operators: `+` union, `&` intersection, `-` exclusion, `->` arrow (follow a relation, then evaluate a permission on the target). |
| Wildcard | A grant to every object of a type. |
| Caveat | A named CEL expression that returns true or false, attached to a relationship. The relationship counts only when the expression is true at check time. Context comes partly from the stored relationship and partly from the check request. |
| Expiration | A relationship can have an `optionalExpiresAt` time in RFC 3339 format. The schema opts in with `use expiration`. |

A permission check follows a chain of relationships. The documentation states it as a
graph reachability problem: "is there a chain of relationships starting at this resource
and relation that ultimately reaches this subject?"

The application writes and updates relationships. SpiceDB does not read them from another
store. APIs are gRPC and HTTP. The CLI is `zed`. Client libraries and a browser
Playground exist.

## Mechanics

| Aspect | Behaviour |
|---|---|
| Datastores | CockroachDB, recommended for self-hosted high throughput or multi-region. Cloud Spanner. PostgreSQL, recommended for self-hosted single-region. MySQL, not recommended. memdb. |
| memdb | "Fully ephemeral; all data is lost when the process is terminated." It cannot run highly available. Recommended for local development and integration tests. |
| Consistency | Set per request: `minimize_latency`, `at_least_as_fresh`, `at_exact_snapshot`, `fully_consistent`. A `ZedToken` from a write fixes a point in time for later reads. Writes and schema calls default to `fully_consistent`. Other calls default to `minimize_latency`. The documentation names the stale-cache risk the "new enemy problem". |
| Watch | A streaming API of relationship changes. History is kept for the datastore's garbage collection window, "typically 24 hours". The application persists any longer audit history. |
| Audit | Reflection APIs list the permissions available on a resource. A bulk check runs many checks in one call. The documentation separates an access grant (a relationship) from a permission (the computed result). |
| Deployment | Binaries for Linux, macOS and Windows on AMD64 and ARM64. Container images. Kubernetes, which the README recommends for self-hosting, with an operator. A managed service, AuthZed Cloud. |
| Caveat types | `int`, `uint`, `bool`, `string`, `double`, `bytes`, `duration`, `timestamp`, `list`, `map`, `ipaddress`. |
| Expiry clock | The datastore's clock marks a relationship as expired. On CockroachDB or Spanner the documentation warns about clock uncertainty. |

## Not read or not documented

- The FAQ and the getting-started overview page. Both fetches failed with a connection
  reset.
- The Zanzibar paper.
- The documentation titles list tutorials on authorization for AI agents and RAG
  pipelines. They were not opened.
- Running SpiceDB as an embedded library, without a server process. The pages read do not
  describe it.
- Prices for AuthZed Cloud.
- Any example of a spend-approval threshold. Caveats can compare numeric values, but the
  pages read show no such example.

## Comparison with tsk

Post is a placeholder in tsk. No permission model is designed. The rows below set the
SpiceDB mechanism beside the tsk term it would touch.

| Aspect | SpiceDB | tsk |
|---|---|---|
| Subject | Any object type: a user, a group, a service. | Actor: a human or an agent session. |
| Position and holder | Each is a defined type. A relationship connects them. | A post exists whether or not anyone fills it. An actor is appointed to it. |
| Authority from a position | A permission computed through a relationship chain, such as actor to post to permission, with an arrow. | Holding the post grants its permissions. Not designed. |
| Threshold approval | A caveat compares a supplied value with a stored one. | A spend above a threshold is outside a post's authority. Not designed. |
| Time-limited appointment | An expiring relationship. | Not designed. |
| Find who can approve | The reverse index: "Who can access `resource`?" | An agent addresses a post, not its holder. Not designed. |
| Source of truth | SpiceDB's datastore. | An append-only NDJSON event log per actor. SQLite is a disposable cache rebuilt from the log. |
| Process | A separate server, with PostgreSQL, CockroachDB, Spanner or MySQL for persistence. | A planned Rust daemon, `tskd`, built for tsk. The bootstrap ledger is plain files on a git branch. |
| History | Watch API, about 24 hours by default. The application keeps the rest. | Append-only logs keep history. |
| Standing instructions | Not modelled. | Post holds standing instructions. Not designed. |
| Escalation routing | Not modelled. SpiceDB returns whether a check passes. | An agent escalates to another post over a comms medium. Not designed. |

## Open

- Whether a permission store belongs in tsk's event log or in a separate service.
  `persistence-and-sync.md` rejected Dolt on stated grounds, among them its Go core, which
  means a separate process or a wire connection from Rust. SpiceDB is also a Go service
  reached over gRPC or HTTP. Not decided.
- Whether SpiceDB could be rebuilt from the event log, as the SQLite cache is. The write
  and bulk import APIs exist. The fit was not tested.
- Whether tsk needs a graph of relationships at all, or a table of posts and their
  authority.

## Sources

- [SpiceDB README](https://github.com/authzed/spicedb) and
  [licence](https://github.com/authzed/spicedb/blob/main/LICENSE)
- [Schema language reference](https://authzed.com/docs/spicedb/concepts/schema)
- [Relationships](https://authzed.com/docs/spicedb/concepts/relationships)
- [Caveats](https://authzed.com/docs/spicedb/concepts/caveats)
- [Relationships that expire](https://authzed.com/docs/spicedb/concepts/expiring-relationships)
- [Datastores](https://authzed.com/docs/spicedb/concepts/datastores)
- [Consistency](https://authzed.com/docs/spicedb/concepts/consistency)
- [Watching relationship changes](https://authzed.com/docs/spicedb/concepts/watch)
- [Access control management](https://authzed.com/docs/spicedb/modeling/access-control-management)
  and [access control audit](https://authzed.com/docs/spicedb/modeling/access-control-audit)

## Related

- [underlying-energy-constraints-of-running-a-factory.md](orchestration-ecosystem/underlying-energy-constraints-of-running-a-factory.md):
  posts as an escalation target, and token spend as a budget decision.
- [docs/domain/ubiquitous-language.md](../domain/ubiquitous-language.md): Post, Actor.
- [docs/domain/persistence-and-sync.md](../domain/persistence-and-sync.md): the event log
  and the disposable cache.
