# 3. JSON-RPC 2.0 as IPC Message Protocol

Date: 2026-03-08

## Status

Accepted

## Context

tsk uses a client-daemon architecture where multiple processes (CLI, TUI, agents) communicate with a central daemon over a Unix domain socket. We need a message protocol for this communication that supports both commands (write operations) and queries (read operations).

The protocol must be:

- Simple to implement without external dependencies initially.
- Able to support multiple concurrent clients.
- Flexible enough to migrate to a network transport (TCP, WebSocket) later.
- Lightweight — messages are small (hundreds of bytes), throughput is low (tens of messages per minute, not thousands per second).

## Decision

Use JSON-RPC 2.0 over NDJSON-framed Unix domain stream sockets.

## Alternatives Considered

### 1. Tagged NDJSON (custom)

Each message is a JSON object with a `type` field used for dispatch. No standard structure for requests, responses, or errors.

```json
{"type":"thread_start","slug":"fix-login","priority":"normal"}
{"type":"thread_started","id":3,"dir":"/home/jim/.tsk/threads/fix-login"}
{"type":"error","message":"slug already exists"}
```

**Pros:**

- Minimal boilerplate. Fewest bytes on the wire.
- No spec to learn — entirely custom.

**Cons:**

- No request/response correlation. Relies on ordering — send one, read one. Cannot pipeline multiple requests on a single connection.
- No standard error format. Every client must implement its own error handling conventions.
- Custom protocol means custom documentation. New contributors or agents have nothing to reference.
- Migration to a network transport would require adding correlation IDs and error conventions — effectively reinventing JSON-RPC.

### 2. JSON-RPC 2.0 (chosen)

A lightweight, transport-agnostic standard. Requests have `method`, `params`, `id`. Responses echo the `id` and contain `result` or `error`.

```json
{"jsonrpc":"2.0","id":1,"method":"thread.start","params":{"slug":"fix-login","priority":"normal"}}
{"jsonrpc":"2.0","id":1,"result":{"id":3,"dir":"/home/jim/.tsk/threads/fix-login"}}
{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"slug already exists"}}
```

**Pros:**

- Request/response correlation via `id`. Supports pipelining and out-of-order responses.
- Standard error format with codes. Consistent across CLI, TUI, and agents.
- Well-known spec — agents and contributors can reference it without custom documentation.
- Transport-agnostic by design. Moving from Unix socket to TCP or WebSocket requires no protocol changes.
- Rust crates available (`jsonrpc-core`, `jsonrpsee`) if we later want validation, middleware, or transport switching. Not required initially — can implement by hand with serde.

**Cons:**

- Slightly more verbose per message (~40 extra bytes for `jsonrpc`, `id`, and structural fields).
- Spec has features we won't use initially (batch requests, notifications without `id`).

### 3. REST/HTTP over Unix socket

Full HTTP semantics (verbs, paths, headers, status codes) over the Unix socket. This is what Docker does.

**Pros:**

- Rich semantics — GET/POST/PUT/DELETE map naturally to queries and commands.
- Enormous ecosystem of tooling (curl, Postman, every HTTP client library).
- Docker validates the approach at scale.

**Cons:**

- Significant parsing overhead for HTTP headers on every request. Overkill for small local messages.
- Requires an HTTP server library (e.g. hyper, actix-web) — a heavy dependency for what is fundamentally a simple message exchange.
- The REST resource model (nouns + verbs) is more structure than tsk needs at this stage.

### 4. Protocol Buffers / gRPC

Binary serialisation with schema enforcement and code generation. gRPC provides the RPC framework on top.

**Pros:**

- Compact binary format. Faster serialisation/deserialisation than JSON at high throughput.
- Schema-enforced contracts between client and server. Breaking changes are caught at compile time.
- gRPC gives streaming, bidirectional communication, and deadline propagation.

**Cons:**

- Adds a code generation step (protoc) to the build.
- Significant dependency weight (tonic, prost for Rust).
- Binary format is not human-readable — harder to debug by tailing the socket.
- The compactness advantage is irrelevant for tsk's message sizes (hundreds of bytes) and throughput (low).
- Premature optimisation for a problem we don't have.

## Consequences

- Initial implementation is hand-rolled: serde for JSON, manual `id` tracking, newline framing. No external RPC framework dependency.
- Messages are human-readable — can be debugged with `socat` or `nc` against the socket.
- The `id` field enables future pipelining even though the first implementation will be synchronous request-response.
- If tsk later needs network access (remote agents, web dashboard), the protocol moves to TCP or WebSocket with zero changes to the message format.
- If throughput or schema enforcement becomes a concern, Protocol Buffers can be introduced as a serialisation layer beneath the same RPC semantics.
