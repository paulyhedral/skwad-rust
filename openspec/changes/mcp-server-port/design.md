## Context

See proposal.md - Why. The contract is `openspec/specs/mcp-server/spec.md`.

The Swift reference is `Skwad/MCP/MCPServer.swift` (Hummingbird router +
handlers), `MCPTypes.swift` (JSON-RPC envelope, `AnyCodable`, tool
definitions), `MCPSessionManager.swift` (session actor), and
`MCPTransport.swift` (the transport protocol + `SSEEvent` formatter).
`MCPServer` also owns two endpoints outside this spec's scope -
`/api/v1/agent/register` and `POST /api/v1/agent/status` (hook ingestion,
dispatched to `ClaudeHookHandler`/`CodexHookHandler`) - which stay with
`agent-hooks`.

The stack mapping in `openspec/config.yaml` names axum/hyper for the HTTP
layer (replacing Hummingbird); the workspace has no HTTP framework dependency
yet, so this change adds one.

Constraints from repo conventions: `Result` + `thiserror`, no panics in
library code, constants in one module, functions <= 5-6 args, `cargo
+nightly fmt`. Unlike `skwad-git`/`skwad-history`/`skwad-agents`, an HTTP
server is inherently async, so `skwad-mcp` is the first crate to take a
`tokio` runtime dependency beyond what `skwad-discovery` already uses for its
watcher.

## Goals / Non-Goals

**Goals:**

- One crate, `skwad-mcp`, exposing a `McpServer` that implements every
  requirement in the spec: bind/lifecycle, health/info, JSON-RPC dispatch,
  SSE framing, the status endpoint, and session tracking.
- A `ToolCatalog` trait so `tools/list`/`tools/call` dispatch through an
  injected implementation rather than a hardcoded match statement - the
  concrete tools arrive in the `mcp-tools` change without touching this
  crate again.
- The status endpoint takes an `AgentSnapshot` (caller-supplied `&[Agent]`
  read, e.g. via a `Fn() -> Vec<Agent>` or a shared `Arc<Mutex<...>>` the
  caller owns) rather than depending on `skwad_agents::AgentStore` directly -
  keeps `skwad-mcp` decoupled from how the caller stores agents.
- Integration-style tests that start the real axum server on an ephemeral
  port and hit it with an HTTP client, since the spec's scenarios are
  wire-level (status codes, JSON-RPC envelopes, SSE framing).

**Non-Goals:**

- The concrete tool catalog, agent-hooks HTTP routes, or the messaging queue
  - see proposal.md - Non-goals.
- Long-lived server-push over the SSE `GET /mcp` stream beyond the initial
  `connected` event - the Swift reference itself only sends that one event
  today; the spec's scenario coverage stops there too.
- Graceful shutdown draining in-flight requests - `stop()` aborts the server
  task, matching the Swift reference's `serverTask?.cancel()`.

## Decisions

### Crate layout

`skwad-mcp` as a sibling of `skwad-agents`, depending on `skwad-agents` (for
the `Agent` type the status endpoint serializes) plus `axum`, `tokio`,
`tokio-stream`, `serde`, `serde_json`, `uuid`, `thiserror`. Modules:
`consts`, `error`, `session` (`McpSession`, `McpSessionManager`), `status`
(the `GET /api/v1/agent/status` handler + `AgentStatusEntry`), `rpc`
(JSON-RPC envelope types + the `initialize`/`tools/list`/`tools/call`
dispatch), `tools` (the `ToolCatalog` trait + `ToolDefinition`/
`ToolCallResult`), `server` (`McpServer`, router assembly, `start`/`stop`).
`lib.rs` re-exports the public surface.

Alternative: fold `mcp-server` into `skwad-agents`. Rejected - an HTTP
server is a distinct runtime concern from the in-memory `AgentStore`, and
`mcp-tools`/`mcp-messaging`/`agent-hooks` all need the server crate without
needing to depend on agent lifecycle internals.

### HTTP framework: axum

axum over a raw `hyper` service or `tiny_http`: axum's `Router` +
extractors map directly onto the Swift reference's Hummingbird router (same
shape: per-route handler closures), it's the de facto standard on tokio, and
`openspec/config.yaml`'s stack mapping already names it. Version `0.8.9`
(current stable per crates.io), pulling in `tower`/`hyper` transitively -
no direct dependency on either needed.

### JSON-RPC envelope: hand-rolled, not a crate

The Swift reference's `JSONRPCRequest`/`JSONRPCResponse`/`JSONRPCId`/
`JSONRPCError` are ~40 lines of `serde` structs; no workspace dependency
exists for JSON-RPC and the surface is small enough that a crate would cost
more (API-shape adaptation, another `Cargo.lock` entry) than it saves.
`AnyCodable` becomes `serde_json::Value` directly - Rust's `serde_json`
already has the untyped-JSON case Swift's `AnyCodable` hand-rolls.

### Tool dispatch: `ToolCatalog` trait, not a generic parameter

```rust
#[async_trait::async_trait]
pub trait ToolCatalog: Send + Sync {
    fn list(&self) -> Vec<ToolDefinition>;
    async fn call(&self, name: &str, arguments: serde_json::Value) -> ToolCallResult;
}
```

`McpServer` holds `Arc<dyn ToolCatalog>` rather than being generic over a
`ToolCatalog` type parameter - the server is constructed once at app
startup and stored as a trait object anywhere else in the app (e.g. behind
`Arc<McpServer>` in app state), so a generic would just push the same `dyn`
erasure to every call site. An empty `struct EmptyCatalog;` implementing
`ToolCatalog` with `list() -> vec![]` and `call()` always returning
`isError: true, "unknown tool: {name}"` is the crate's only concrete impl,
used by this change's own tests and available to any caller not yet wired
to `mcp-tools`.

`async_trait` is a new dependency (the trait needs `async fn` in a `dyn`
object); alternative was a boxed-future return type by hand
(`Pin<Box<dyn Future<Output = ToolCallResult> + Send>>`) - rejected as more
boilerplate for the same result the macro gives for free.

### Session storage: `Arc<Mutex<HashMap>>`, not an actor

Swift's `MCPSessionManager` is an actor (message-per-call serialization).
The direct Rust analogue for a small, short-held critical section is
`Arc<Mutex<SessionTable>>` inside `McpSessionManager`, wrapping two
`HashMap`s (`sessions: HashMap<String, McpSession>`,
`agent_to_session: HashMap<Uuid, String>`) exactly like the Swift reference's
two dictionaries. No channel/task actor needed - every operation is a
sub-microsecond map mutation, not an awaited operation.

### Status endpoint input: caller-supplied slice, not a shared store handle

`status::agent_status(agents: &[Agent]) -> Vec<AgentStatusEntry>` is a pure
function; the axum handler wraps it via a closure or small state struct the
caller provides at router-build time (e.g. `Arc<dyn Fn() -> Vec<Agent> + Send
+ Sync>`). Keeps `skwad-mcp` from depending on `skwad_agents::AgentStore`'s
concurrency story (the eventual app shell may wrap it in its own
`Arc<Mutex<...>>` or an actor) - `skwad-mcp` only needs read access to a
`Vec<Agent>` snapshot at request time.

### Server lifecycle: explicit `start`/`stop`, "disabled" = "never started"

The spec's "Disabled by configuration" scenario ("no socket is opened") maps
directly onto never calling `McpServer::start()` - there's no internal
enabled/disabled flag to plumb through the crate. `start(&self) ->
Result<()>` binds the listener and spawns the serve loop on a
`tokio::task::JoinHandle` stored in `self`; `stop(&self)` aborts it. Matches
the Swift reference's `serverTask?.cancel()`.

## Risks / Trade-offs

- [Risk] axum's `Router` requires `Clone` state, so `McpServer`'s shared
  pieces (`Arc<McpSessionManager>`, `Arc<dyn ToolCatalog>`, the status
  closure) must be `Clone`-cheap `Arc`s from the start, or every handler
  needs re-threading later. -> Design every piece of router state as an
  `Arc`-wrapped field from the first commit (task 2), verified by the crate
  compiling with axum's `State` extractor.
- [Risk] Integration tests that bind a real TCP port can flake in CI
  (port reuse, timing). -> Bind to port `0` (OS-assigned ephemeral) in
  every test and read the actual bound port back from the listener, never a
  fixed port.
- [Risk] `async_trait`'s boxed-future overhead is negligible here (tool
  calls happen at human-interaction cadence, not per-frame), so it's not
  treated as a real cost.

## Migration Plan

New crate, additive workspace member - no migration. `skwad-agents` and
earlier crates are unaffected.
