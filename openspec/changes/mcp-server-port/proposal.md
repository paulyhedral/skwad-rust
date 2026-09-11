## Why

`skwad-agents` (from `agent-lifecycle-port`) now owns the runtime `Agent`
type, but nothing yet exposes it to the outside world. `mcp-server` is next
because `mcp-tools`, `mcp-messaging`, and `agent-hooks` all depend on the
HTTP/JSON-RPC scaffold this spec defines - the local MCP server, its session
tracking, and the plain-JSON status endpoint - rather than the other way
around.

## What Changes

- Add a new crate `crates/skwad-mcp` implementing
  `openspec/specs/mcp-server/spec.md`:
  - An axum HTTP server bound to `127.0.0.1:<port>` (default `8766`),
    started/stopped explicitly by the caller; not opened at all when the
    caller does not start it (satisfies "disabled by configuration" - the
    spec's "disabled" state is simply "never started").
  - `GET /health` and `GET /` (info) endpoints that respond without MCP
    initialization.
  - `POST /mcp`: JSON-RPC 2.0 dispatch for `initialize`,
    `notifications/initialized`, `tools/list`, `tools/call`, and a
    method-not-found error for anything else. Honors `Accept:
    text/event-stream` by wrapping the JSON-RPC response as a single SSE
    `message` event; otherwise returns plain JSON. Notifications (methods
    starting `notifications/`) get a bare 202 Accepted.
  - `GET /mcp`: opens an SSE stream with an initial `connected` event.
  - `GET /api/v1/agent/status`: JSON array of agent status entries (id,
    name, folder, state, status text, registered flag, agent type, session
    id when present, metadata when non-empty), sourced from a caller-supplied
    snapshot of `skwad_agents::Agent`s, sorted keys.
  - MCP session tracking (`McpSessionManager`): one session per agent,
    replace-on-recreate, idle-timeout reclamation (default 1 hour).
  - A `ToolCatalog` trait the server dispatches `tools/list`/`tools/call`
    through, so the concrete tool set (`mcp-tools`, a separate change) plugs
    in without changing this crate. An empty catalog is valid and yields an
    empty tool list and "unknown tool" for every call.
  - `McpError` (`thiserror`) with a crate `Result` alias.
  - Tests covering every scenario in the spec: default bind, health check,
    initialize handshake, unknown method, tool-result shape (success and
    unknown-tool), status reflects live agents, one-session-per-agent.
- Add `crates/skwad-mcp` to the workspace `Cargo.toml` members and adds
  `axum` and `tokio-stream` to `[workspace.dependencies]` (both new).

Non-goals:

- The concrete tool catalog (`list-agents`, `send-message`, ...) -
  `mcp-tools` is a separate spec/change; this change only defines the trait
  it plugs into.
- Agent-to-agent messaging routing rules - `mcp-messaging`.
- The `/api/v1/agent/register` and `/api/v1/agent/status` (POST) hook
  ingestion endpoints - `agent-hooks` owns those; this spec only covers the
  `GET /api/v1/agent/status` read endpoint.
- Wiring the server into the app shell, or scheduling the deferred
  registration prompt - a later change.

## Capabilities

### New Capabilities

None. This change implements the existing `mcp-server` spec without changing
its requirements.

### Modified Capabilities

None. `openspec/specs/mcp-server/spec.md` is the unchanged contract; this
change adds the implementation. `skip_specs: true`.

## Impact

- New crate: `crates/skwad-mcp/` (`Cargo.toml`, `src/lib.rs`, `consts.rs`,
  `error.rs`, `session.rs`, `status.rs`, `rpc.rs`, `tools.rs`, `server.rs`,
  `tests/`).
- Modified: root `Cargo.toml` (workspace members gains `skwad-mcp`; adds
  `axum`, `tokio-stream` to `[workspace.dependencies]`), `Cargo.lock`.
- `skwad-core`, `skwad-git`, `skwad-discovery`, `skwad-history`,
  `skwad-agents`, `skwad`, and the Swift build are unaffected. `skwad-mcp`
  depends on `skwad-agents` (for the `Agent` snapshot type used by the status
  endpoint) plus `axum`, `tokio`, `tokio-stream`, `serde`, `serde_json`,
  `uuid`, `thiserror` (all via workspace where applicable).
