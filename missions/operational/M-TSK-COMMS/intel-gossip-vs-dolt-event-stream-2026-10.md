# Intel: gossip protocols and a Dolt event stream for robot messages, October 2026

Date: 2026-10-25. Raised by Jim. Linked to M-TSK-COMMS tasks T-04 (an event stream in the
Dolt ledger) and T-05 (a gossip protocol that each robot runs, and the nexus as a rendezvous).
It builds on `intel-messaging-existing-sessions-2026-10.md` (the routes that push a message
into a session from outside; corrected on 2026-10-24: `fire_trigger` from a session other than
the one the task is bound to starts a new session) and on
`intel-self-armed-watches-background-command-and-monitor-2026-10.md` (a robot arms a watch
itself, and the notification wakes an idle session). Neither is repeated here.

Evidence classes: **official** (documentation or a paper fetched and read on 2026-10-25),
**observed** (output of a command run in this cloud container on 2026-10-25), **reasoned** (an
inference from the others). Every claim below has one, and no claim rests on a search summary
alone. Nothing was written to the ledger, and nothing was pushed.

## Summary

- A cloud container has no inbound path. Its only address is private (`192.0.2.2`), the only
  process bound to all interfaces is the platform's `process_api`, started with the option
  `block-local-connections` and controlled over vsock, and the documentation lists outbound
  levels only (observed, official). Every gossip family needs a peer that accepts a connection
  or a datagram, so no gossip protocol can run between cloud containers as they are (reasoned).
- Gossip through a relay is a star topology. The relay would be a server someone runs, GitHub,
  or the Dolt ledger. The ledger is already that relay, with history and merge (reasoned).
- The nexus can record presence (which robot is alive, on which mission) but not reachability,
  because a container has no address to publish (reasoned from section 2).
- A Dolt event stream fits what exists: `tsk events` (a single-consumer queue), the local Dolt
  server (a record command in 17 to 25 ms here), publish points, and the self-armed watch.
  End-to-end latency is set by the sender's push, the receiver's poll interval, and the 6 to
  12 s wake: about 10 to 30 s with a 5 s poll (reasoned from measured parts).
- Recommendation: T-05, do not go on with gossip among cloud containers (confidence high on
  the sandbox facts, medium on the recommendation itself). T-04, go on, with a random event ID, a
  read-mark row for each recipient, and a watch command (confidence medium to high).

## 1. Gossip protocols

The families, from the sources read. The Demers et al. 1987 paper did not load (see Not
established); the rumour-spreading facts are taken from the papers that build on it.

| Family | How a message is sent | What each node needs | Speed and reliability | Evidence |
|---|---|---|---|---|
| Epidemic broadcast (rumour spreading) | A node that receives a message for the first time sends it to `t` random nodes (the fanout). Each receiver repeats this | The whole membership, or a partial view of it, to pick targets | Delivery to all nodes needs a fanout near `log(n)`. In a 10,000 node simulation, `log(10,000) = 4`, and earlier membership protocols needed 5 to 6 for over 99 % reliability | official (HyParView paper, sections 1 and 3.1) |
| Anti-entropy | Two nodes exchange their full state and merge it, at a low rate | A peer to connect to over TCP | memberlist runs one full state sync every 30 s ("Low frequency"), over TCP with one node | official (memberlist `config.go`) |
| SWIM, memberlist, Serf | A failure detector (ping, then ping-req through `k` members, each protocol period). Membership updates are included in those messages, "λlog(n) times" each | A join address: "New nodes in an existing cluster must be given the address of at least one existing member." Gossip over UDP, full state sync over TCP, one port for both (7946 by default) | Expected time to first detection, false positive rate and message load per member are independent of group size. Defaults: gossip every 200 ms to 3 nodes, probe every 1 s, probe timeout 500 ms, suspicion timeout `4 * log(N+1) * ProbeInterval` | official (Lifeguard paper section III, Serf gossip page, memberlist `config.go`) |
| HyParView | Flooding over a small symmetric active view of size `log(n) + c` over TCP. A passive view of size `k(log(n) + c)` is the backup | A contact node: a new node "establishes a TCP connection to the contact node" | Recovers from failure of 90 % of nodes in as few as 4 membership rounds | official (HyParView paper, abstract, sections 1 and 4) |
| Plumtree | Eager push of the payload along tree branches. Lazy push of the message ID (IHAVE) on the other overlay links repairs the tree | A peer sampling service such as HyParView, and TCP connections between neighbours | Recovers from failures of up to 80 % of nodes. A fanout of 4 with HyParView in a 10,000 node simulation | official (Plumtree paper, abstract and section 3) |
| CRDT replication | State-based: replicas exchange and merge whole states. Operation-based: every operation is delivered to every replica | State-based: a connected synchronisation graph. Operation-based: reliable delivery of every operation, and causal order for most designs | Convergence is deterministic once all updates are received. The papers state nothing about speed | official (Preguiça, Baquero and Shapiro 2018, section on synchronisation) |

What every family has in common (reasoned): a node must accept a connection or a datagram from
a peer, each node must have the address of at least one other node to join, and the protocol runs
as a process in each node for as long as the node is a member. Serf also caps a user event at
9 KB because it is sent in one UDP packet (official, Serf gossip page).

## 2. What a cloud container can open and accept

### Official

- Four network access levels, all for outbound connections: None ("No outbound network access
  through the session's network"), Trusted (allowlisted domains), Full, Custom. GitHub, MCP
  connectors, network-secret hosts and the Anthropic API take paths that do not go through the
  allowlist. No level describes an inbound connection (cloud environments page).
- "All outbound internet traffic from an Anthropic-hosted session passes through this proxy"
  (the security proxy). "Each cloud session runs in an isolated, Anthropic-managed VM." The
  pages state nothing about two sessions sharing a host or a network (cloud environments page,
  security page).
- "After a few minutes without activity, a session's VM pauses with its files saved, and a paused
  VM can later be reclaimed." A reopened session restores the conversation and not shell
  commands, subagents or session-scoped scheduled tasks (cloud environments page, cloud page).
- The agent proxy README in this container: not supported through the proxy are "gRPC /
  HTTP/2-only APIs, WebSocket upgrades, client-mTLS, certificate-pinned clients ... non-443
  HTTPS ports, raw-TCP databases" (`/root/.ccr/README.md`).

### Observed

Each row is one test run here. No other host was scanned.

| Test | Command | Result |
|---|---|---|
| Addresses | `/proc/net/fib_trie`, `/proc/net/route` | `127.0.0.1` and `192.0.2.2/24` on `eth0`, default route via `192.0.2.1`. IPv6 is off (`ipv6.disable=1` on the kernel command line). `hostname` is `vm` and resolves to `127.0.0.1` |
| Runtime | `/proc/1/cmdline`, `dmesg` | PID 1 is `/process_api` with the options `firecracker-init`, `addr 0.0.0.0:2024`, `block-local-connections`, `control-vsock-port 5001` and `listen-vsock-port 2024` (the leading hyphens of each option are left out here). Kernel `6.18.44-fc-v153`. `/etc/hosts` contains `127.0.0.1 runsc`. The VM is a Firecracker microVM controlled over vsock |
| Sockets bound | `/proc/net/tcp` decoded | Bound on loopback: `127.0.0.1:38889` (the agent proxy, in the `claude` process), `127.0.0.1:46031` (`dolt sql-server` of the ledger store), `127.0.0.1:43641` (`environment-manager`). Bound on all interfaces: `0.0.0.0:2024` (`process_api`, PID 1). Nothing else |
| Loopback bind | Python: bind `127.0.0.1:0`, connect, echo | Worked: `loopback reply b'echo:hello'` |
| All-interfaces bind | Python: bind `0.0.0.0:0`, connect to `192.0.2.2:<port>` | Worked from inside the container: `accepted from ('192.0.2.2', 38056)`. Nothing outside the VM was tested against it, and the address is private |
| Dolt server port | `/dev/tcp/127.0.0.1/46031` | Connected. ADR 0018 states that this port has no password, so any process in the VM can open the store |
| Outbound through the proxy | `curl https://github.com/`, `curl https://example.com/` | 400 (GitHub's response to a bare GET) in 0.09 s and 200 in 0.33 s. `recentRelayFailures` stayed empty |
| Outbound without the proxy, port 443 | `curl https://github.com/` with the proxy switched off for that call, `openssl s_client example.com:443` | 200 in 0.10 s. The certificate for `example.com` is issued by `O = Anthropic, CN = Egress Gateway SDS Issuing CA (production)`. Port 443 is intercepted by an egress gateway whether or not the proxy is used |
| Outbound without the proxy, other ports | `/dev/tcp/example.com/80`, `/dev/tcp/github.com/22` | Port 80 connected and a plain GET returned 200. Port 22 timed out after 3 s |
| DNS | `getent hosts github.com` | Answered. `/etc/resolv.conf` names `8.8.8.8` and `8.8.4.4` |
| A host not on the Trusted list | `openssl s_client -proxy ... www.cs.cornell.edu:443` | The CONNECT returned 200. The TLS handshake failed with `verify error:num=20: unable to get local issuer certificate`, and the certificate shown was Cornell's (issuer InCommon). `pages.lip6.fr` failed with `ws_closed_mid_exchange`. Whether these are allowlist blocks is not established |
| Live Dolt server | `tsk ledger server status` | `server: running`, socket in the store, port 46031, idle stop 1800 s, switch on from config |

### Conclusions from the tests

- Inbound: nothing outside the VM has a route to `192.0.2.2`, no documented feature exposes a
  port, and the one process bound to all interfaces is the platform's `process_api`, started
  with the option `block-local-connections`. Two cloud sessions of one account have no path to each other
  (reasoned from the rows above; no second container was available to confirm the negative).
- Same host: the kernel is a Firecracker guest kernel, so each session runs in a separate
  microVM. Whether two VMs share a network segment is not established.
- Outbound: TCP to ports 80 and 443 connects to hosts on the internet, through the proxy or
  through the egress gateway. UDP is confirmed only for DNS to the configured resolvers. Port
  22 did not connect. Nothing supports the UDP gossip of SWIM, memberlist or Serf, and the
  README rules out WebSocket upgrades and raw TCP through the proxy (observed, official).

## 3. Discovery through the nexus

T-05 proposes the nexus as the rendezvous where each robot publishes that it is present, with
an address for it. The nexus is a git repository with a `nexus.json` at the
root of its default branch, attached by `tsk config attach-nexus`, and its entries are added by
hand today (`docs/domain/territory-and-nexus.md`; official for this repo).

Sketch (reasoned):

1. A presence record for each harness instance: actor name, instance key (the `cse_` session ID
   or `<clone-id>/<worktree-name>`), repo ID, thread, mission, a time of last heartbeat, and an
   address field.
2. Where it is stored: a table in the nexus's ledger (the nexus repo may have a ledger, decided
   and not built), or in each repo's ledger beside `instances`. The ledger
   already has `actors` and `instances` (layout 9), so a heartbeat column on `instances` is the
   smaller change and is what T-02 specifies.
3. How it is published: with `tsk ledger push` and a fetch, like every record. There is no
   other path.

What the sketch gives and does not give:

- It gives presence and liveness: who is alive, since when, on what. That serves objective 1
  and T-02, and a coordinator reads it with `tsk thread list` or `tsk actor list`.
- It does not give reachability. The address field would contain `192.0.2.2` for every cloud
  robot, to which no other VM has a route (section 2). A laptop robot could publish a real
  address, and a cloud robot still could not accept a connection. Gossip needs both sides.

Gossip cannot work between cloud robots without a relay. The candidate relays:

| Relay | What it needs | What it gives | Evidence |
|---|---|---|---|
| A server someone runs | A public host on port 443, in the environment's allowlist (Custom level), plain HTTPS because the proxy rejects WebSocket upgrades. Someone hosts, patches and secures it | Low latency with long polling. A second system to run, with a separate identity and trust model | official (cloud environments page, proxy README), reasoned |
| GitHub | Nothing new: every container connects to it through the GitHub proxy at every level | Refs, issues or comments as a message board. A pull request subscription already wakes a session on PR activity (route 6 of the first intel) | official, reasoned |
| The Dolt ledger | Nothing new: `tsk ledger push` and the ledger refs are already sent through GitHub | A table with history, merge by row, and the same publish points as the records | official for this repo (`ledger-layout.md`), reasoned |

With any relay, every message is sent from the sender to the relay, then from the relay to the
receiver. That is a star topology, and the epidemic and membership parts of gossip are not
used. The relay through the Dolt ledger is the design of T-04.

## 4. The Dolt event stream

What exists today (official for this repo, from `docs/domain/ledger-layout.md`,
`cli/src/events`, ADR 0018, 0019 and 0020):

- `external_events` (`position` bigint primary key, `envelope` longtext) and `watermark` (one
  row, `processed_through`). `tsk events append` and `append-batch` fetch, append and push in
  one cycle. `read-new` fetches and prints the rows past the watermark. `advance-watermark`
  writes the row and pushes. The queue has one consumer: one watermark for everyone, and a
  `position` minted by the appending clone. "Two writers that append an external event at the
  same moment take the same `position` and conflict."
- Dolt merges by row and cell: "If two operations modify the same row, column pair to be
  different values, a conflict is detected" (official, Dolt documentation). Two new rows with
  different primary keys merge. `tsk ledger push` fetches, merges, pushes, and retries up to 10
  times with a 50 to 250 ms wait.
- Records are published only at a publish point (ADR 0019): thread start, pause and stop, the
  end of a turn through the `Stop` hook in the background (ADR 0020), the `SessionEnd` hook,
  and `tsk ledger push` when asked. The event commands push at once by rule (ADR 0019,
  decision 6).
- The local Dolt server (ADR 0018) returns a SQL result in under a millisecond, on by default
  since 0.29.0. Measured here: `tsk mission list` 18 to 25 ms over five runs, `tsk task list
  M-TSK-COMMS` 17 to 19 ms over three (observed). This matches T-04's figure of about 16 ms.
- The watch: Bash in background mode (one notification at exit; 30 minutes by default and at
  most 2 hours in a cloud session) or Monitor (one notification for each stdout line; "5
  minutes by default, at most 30 minutes", then "Claude gets one notice, so it can start the
  watch again") (official, tools reference). A notification woke an idle session 6 to 12 s
  after the event (observed in the second intel document).

The design T-04 sketches, in those terms (reasoned):

| Part | Design | Why |
|---|---|---|
| Table | `events`: `id` (random UUID, primary key), `sender` (actor name), `sender_instance`, `recipient_kind` enum (`actor`, `thread`, `mission`, `all`), `recipient`, `kind`, `body` longtext, `created_at`. Append only | A random key means two clones that append at once never collide, unlike `position`. A random key has no renumbering step, because a person never types an event ID |
| Read marks | `event_reads`: (`event_id`, `actor`) primary key, `state` enum (`read`, `done`, `declined`), `at`, `note`. Each recipient writes only its own row | No shared watermark, so no conflict between recipients, and the sender can count rows |
| Send | `tsk events send <recipient> <kind> <body>` writes the row and pushes at once, as `append` does | A message is for another process, so it is not progress that can wait (ADR 0019, decision 6) |
| Receive | `tsk events list` with an option that names the actor, and `tsk events wait` that polls and prints one line for each new event addressed to this actor, then exits, or continues | The line is the event the watch acts on. The format is tsk's, not each robot's shell loop |
| Session start | `tsk thread session-start` prints the unread events for the bound actor | A robot that was asleep, ended, or joined later reads the stream at its next start |

Latency, part by part (observed and official for this repo, total reasoned):

| Step | Time | Source |
|---|---|---|
| Sender's `tsk ledger push` to the GitHub origin | 1.7 to 1.9 s (a `git push` through the cloud proxy is 1.2 to 1.4 s whatever its size) | ADR 0019 and its amendment |
| Receiver's poll probe, `git ls-remote origin refs/heads/tsk/ledger` | 399 to 579 ms over three runs. `tsk ledger status`, which runs one `ls-remote` check, 235 to 269 ms | observed |
| Poll interval | 5 s in the second intel. The proxy returned 429 beyond two concurrent clones of one repository, so many robots need a longer interval | second intel document |
| Fetch of the new commit | 1.9 s for a fetch from GitHub on git protocol version 0 (median of 6) | `ledger-layout.md` |
| Notification to a new turn | 6 to 12 s | second intel document |
| Total, one robot, 5 s poll | about 10 to 20 s; with a 30 s poll, 35 to 45 s | reasoned |

Nothing is sent between two clones except at a push and a fetch, so the minimum is the push
plus the poll interval. The local server reduces the cost of each poll, not the interval.

## 5. Broadcast with an action

The use case: a robot or the coordinator sends every robot an event that a major change has
happened and that each must act, for example run `tsk self-update`. The design below is not
built; every point is reasoned, with the sources named:

- **Addressing.** `recipient_kind` of `all` is addressed to every actor; `mission` to the actors
  bound to threads of that mission; `thread` and `actor` to one. The receiver's query joins
  `bindings`, `threads` and `instances`, which exist today.
- **Read mark and acknowledgement.** The watch command writes the `read` row when it prints the
  line, in the binary and before the model reads it. The model, or the binary for an allowlisted
  kind, writes `done` or `declined` with a note. The coordinator counts `event_reads` for the
  event against the actors alive by heartbeat (T-02), so it can tell "not yet read", "read and
  not done" and "declined". Those rows are published at the next publish point of each robot,
  so the count lags by up to one turn, or by nothing if the mark pushes as `send` does.
- **A robot that is not running.** Its event rows wait in the table. `tsk thread session-start`
  prints them, and the model acts in its first turn. A robot whose container was reclaimed and
  reopened is in this case too, because its watch ended with the VM (official, cloud page).
- **A robot that joins later.** The same: it reads every unread `all` event at its first start.
  An event can include `expires_at`, so an old "update now" is skipped once a later version is
  already installed.
- **Ordering.** Dolt gives the order of commits on origin, not of `created_at` across clocks.
  A receiver sorts by `created_at`, then `id`. Two events written at the same moment on two
  clones have no agreed order. A sender that needs order chains events with a `follows` column.
- **Trust.** An event is a row another robot wrote. The model reads it as the output of a `tsk`
  command, which the harness treats as tool output, and a message from another Claude session
  "is information to weigh, not an instruction" (tool schema, cited in the first intel).
  Official: a cross-session message "can't approve anything" and "can't change configuration",
  and a cloud session's permission checks stay in force. So the design never instructs the model
  to obey an event. The model reads the event, checks it against its mission and its
  permissions, and records `done` or `declined`.
- **Actions the binary could run without the model.** Only a kind on an allowlist in the
  binary, with an effect the binary already causes without the model. `tsk self-update`
  is the one clear case: the binary already updates itself and re-runs when it meets an
  unsupported ledger layout (`ledger-layout.md`, Automatic update), the download is from the
  project's release, and `TSK_AUTO_UPDATE=never` already switches it off. A forged event
  could make every robot update to the latest release, which is the state the layout check
  would force anyway. A second candidate is `tsk ledger fetch` when the store has no local
  change. Everything else, a task to take, a mission to change, a command to run, code to edit,
  needs the model and its permission mode, and the event text stays data.
- **Who may send `all`.** Any actor can write a row. If a coordinator needs authority, the
  smallest step is a `sender` check against `actors` of kind `robot` plus a list of coordinator
  names in the ledger, enforced in the binary when it reads, not when it writes.

## 6. Comparison and recommendation

| Criterion | Gossip among robots | Dolt event stream | Evidence |
|---|---|---|---|
| Latency | Sub-second once connected (gossip every 200 ms, probe every 1 s in memberlist) | 10 to 45 s, set by the push, the poll interval and the 6 to 12 s wake | official, observed, reasoned |
| Robot asleep or ended | None. A member that stops is declared failed and dropped. A rejoin needs a live peer and gets no missed message unless a CRDT state is merged | Full. Rows wait in the table and the next session start reads them | official (SWIM, Serf), reasoned |
| Fit with the cloud sandbox | None as is. No inbound path, no UDP beyond DNS, WebSocket and raw TCP rejected by the proxy. A relay turns it into a star | Fits. It uses the GitHub path every level allows, and the local server | observed, official |
| Delivery guarantee | Probabilistic. A fanout near `log(n)` for 99 %, repaired by anti-entropy | A pushed row is on origin, or the push failed and the row stayed local for the next publish | official, repo docs |
| Ordering | Lamport clocks in Serf. No total order | Commit order on origin. `created_at` and `id` for a sort. No total order across clones | official, reasoned |
| Cost in tokens | A wake costs a turn, the same as the stream. The protocol itself costs no tokens but needs a process in every robot for its whole life | A wake costs a turn. A read mark by the binary costs none. A broadcast to `n` robots costs at least `n` turns | second intel, reasoned |
| Failure modes | A false failure under slow processing (the problem the Lifeguard paper fixes). Partitions. A member with a stale view. A message over 9 KB dropped (Serf) | A container reclaimed before a publish loses rows written since the last one (ADR 0019). A 429 from the proxy under many polls. A watch that expires and is not re-armed | official, repo docs, second intel |
| Security | Shared symmetric key for gossip encryption (Consul). Any member can inject. A new transport to secure | The ledger's git access through the GitHub proxy. Rows are data. The binary enforces any allowlist | official (Consul gossip page), reasoned |
| Size of the build | A gossip library in the binary (memberlist is Go, tsk is Rust: a port or a new crate), a daemon per robot, a relay, discovery, encryption and keys | Two tables, four or five commands, a session-start step, and the watch the second intel tested | reasoned |
| What exists today | Nothing in tsk. The sandbox blocks the first step | `external_events`, `tsk events`, the Dolt server, the publish points, the measured watch | repo docs, observed |

### Recommendation

**T-05: do not go on with a gossip protocol among cloud robots.** Record the decision with this
document as its evidence. Keep the presence part of the nexus idea as a liveness record for T-02
(a heartbeat on `instances`), because it is useful without an address. Confidence: high that
cloud containers cannot connect to each other today (observed and official agree, and no page
or test shows an inbound path); medium on the recommendation itself, because the sandbox facts
are the only evidence, and they rest on one container's tests.

**T-04: go on.** Build the stream as section 4 sketches: a random event ID, per-recipient read
marks, `send`, `list`, a watch command that prints one line per new event, and a session-start
step. Keep `external_events` as it is for the GitHub poll. Confidence: medium to high. The
parts are measured (the server, the wake, the push) and the design reuses them. The untested
part is the end-to-end latency and the proxy's limit under many polls.

What would change the recommendation:

- An official statement, or a test with two containers, that cloud sessions can accept a
  connection or share a network. Then a gossip library behind a tsk daemon becomes possible,
  and the comparison turns on build size and the asleep case, which still favour the stream.
- A self-hosted environment for the robots (official: egress and isolation are then the
  deployment's). A relay server or direct peers become a deployment choice, and T-05 reopens.
- A measured end-to-end latency over a minute that the coordinator use case cannot accept.
  Then the fix is a shorter poll with a batched `ls-remote`, or route 1 or 4 of the first
  intel as a faster notice, before any gossip.
- A finding that the model treats an event row as an instruction in auto mode. Then the
  binary must own the read mark and the allowlist, and the body must be shown as quoted data.

## Not established

- Whether two cloud sessions of one account share a network segment or a host. Only one
  container was available; the Firecracker kernel and private address are indirect evidence.
- Whether a Custom allowlist admits a relay server on port 443, and whether long polling
  through the proxy stays open long enough. Not tested.
- The cause of the TLS failure for `www.cs.cornell.edu` and the closed tunnel for
  `pages.lip6.fr` through the proxy. The servers' certificates were shown, so the cause
  may be a missing intermediate certificate rather than an allowlist block.
- Whether UDP is sent out of the VM for anything but DNS. Only DNS was observed.
- The end-to-end latency of the stream. The parts were measured; no event was sent, because
  that would write to the ledger.
- The Demers et al. 1987 paper. Five copies failed to load (ACM 403, two 503s, a scanned PDF
  with no text, a login page). The `log(n)` rumour-spreading facts here are taken from the
  HyParView and Lifeguard papers, which build on it.
- The SWIM paper itself (three 503s). Its mechanics are taken from the Lifeguard paper's section
  III and from Serf's page, both of which describe it at length.
- The Serf convergence simulator's numbers. The page loaded as a form with no values.
- Whether the proxy's 429 on concurrent git operations applies to `ls-remote` as it did to
  clones, and at how many robots it applies.
- What the model does with an event row under auto mode (open since the first intel).

## Sources

- Claude Code docs: [Use Claude Code in the cloud](https://code.claude.com/docs/en/claude-code-on-the-web.md),
  [Configure cloud environments](https://code.claude.com/docs/en/cloud-environments.md),
  [Security](https://code.claude.com/docs/en/security.md),
  [Tools reference](https://code.claude.com/docs/en/tools-reference.md) (Monitor tool, background commands),
  [Run prompts on a schedule](https://code.claude.com/docs/en/scheduled-tasks.md).
- Agent proxy README in this container: `/root/.ccr/README.md`, and
  `curl -sS "$HTTPS_PROXY/__agentproxy/status"`.
- Gossip: [memberlist README](https://github.com/hashicorp/memberlist/blob/master/README.md) and
  [memberlist `config.go`](https://raw.githubusercontent.com/hashicorp/memberlist/master/config.go);
  [Serf gossip protocol page](https://github.com/hashicorp/serf/blob/master/docs/internals/gossip.html.markdown);
  [Consul gossip page](https://developer.hashicorp.com/consul/docs/architecture/gossip);
  Dadgar, Phillips, Currey, [Lifeguard: SWIM-ing with Situational Awareness](https://arxiv.org/abs/1707.00788) (PDF read);
  Leitão, Pereira, Rodrigues, [Epidemic Broadcast Trees](https://asc.di.fct.unl.pt/~jleitao/pdf/srds07-leitao.pdf) (Plumtree, PDF read);
  Leitão, Pereira, Rodrigues, [HyParView](https://asc.di.fct.unl.pt/~jleitao/pdf/dsn07-leitao.pdf) (PDF read);
  Preguiça, Baquero, Shapiro, [Conflict-free Replicated Data Types](https://arxiv.org/abs/1805.06358) (PDF read).
- Dolt: [Merges](https://www.dolthub.com/docs/sql-reference/version-control/merges),
  [Conflicts](https://www.dolthub.com/docs/concepts/dolt/git/conflicts).
- This repo: `docs/domain/ledger-layout.md`, `docs/domain/territory-and-nexus.md`,
  `docs/domain/ubiquitous-language.md`, `docs/adr/0018-a-local-dolt-server-per-ledger-store-behind-a-switch.md`,
  `docs/adr/0019-publish-less-often-by-rule.md`,
  `docs/adr/0020-publish-in-the-background-with-a-detached-one-shot.md`, `cli/src/events/`,
  the help of `tsk events`, `tsk mission list M-TSK-COMMS` with the JSON option, and
  `tsk task list M-TSK-COMMS`.
- Ledger documents: `missions/operational/M-TSK-COMMS/intel-messaging-existing-sessions-2026-10.md`,
  `missions/operational/M-TSK-COMMS/intel-self-armed-watches-background-command-and-monitor-2026-10.md`.
