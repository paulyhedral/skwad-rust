## 1. Crate scaffold

- [x] 1.1 Create `crates/skwad-mcp/` with `Cargo.toml` (workspace edition;
      `skwad-agents` path dep; `thiserror`, `serde`, `serde_json`, `uuid`,
      `tokio` via workspace; new workspace deps `axum` `0.8`,
      `tokio-stream` `0.1`, `async-trait` `0.1`) and empty `src/lib.rs`;
      verify `cargo build -p skwad-mcp` succeeds and `cargo metadata` lists
      the crate (workspace members is a `crates/*` glob, no root edit
      needed).
- [x] 1.2 Add `src/consts.rs` (`DEFAULT_PORT: u16 = 8766`,
      `DEFAULT_SESSION_TIMEOUT: Duration` = 1 hour, `PROTOCOL_VERSION`,
      `SERVER_NAME`, `SERVER_VERSION`) and `src/error.rs` with `McpError`
      (`thiserror`) and `pub type Result<T, E = McpError>`; re-export from
      `lib.rs`; verify `cargo build -p skwad-mcp`.

## 2. Session tracking

- [x] 2.1 Implement `McpSession { id: String, agent_id: Uuid, created_at:
      Instant, last_activity: Instant }` in `src/session.rs`; verify
      `cargo build -p skwad-mcp`.
- [x] 2.2 Implement `McpSessionManager` (`Arc<Mutex<SessionTable>>` wrapping
      `sessions: HashMap<String, McpSession>` and `agent_to_session:
      HashMap<Uuid, String>`) with `create_session(agent_id) -> McpSession`,
      `session(id) -> Option<McpSession>`, `session_for_agent(agent_id) ->
      Option<McpSession>`, `touch(id)`, `remove(id)`,
      `remove_for_agent(agent_id)`, `cleanup_stale(timeout: Duration)`;
      verify a unit test for "One session per agent" (create twice for the
      same agent id, old id no longer resolves, new one does) and a
      `cleanup_stale` test (session older than timeout is removed, newer one
      survives).

## 3. Status endpoint

- [x] 3.1 Implement `AgentStatusEntry` in `src/status.rs` (serde struct:
      `agent_id`, `name`, `folder`, `state`, `status`, `registered`,
      `agent_type`, `session_id: Option<String>`, `metadata:
      BTreeMap<String,String>` skipped when empty via
      `#[serde(skip_serializing_if)]`) and `pub fn agent_status(agents:
      &[skwad_agents::Agent]) -> Vec<AgentStatusEntry>`; verify a unit test
      mapping a registered agent (with session id) and an unregistered one,
      asserting two entries and that only the registered entry serializes a
      `session_id` key (spec: "Status reflects live agents").

## 4. JSON-RPC envelope and dispatch

- [x] 4.1 Implement `src/rpc.rs`: `JsonRpcRequest { jsonrpc, id:
      Option<JsonRpcId>, method: String, params: Option<serde_json::Value>
      }`, `JsonRpcId` (untagged string/int via serde), `JsonRpcResponse`
      with `success(id, result)` / `error(id, code, message)`
      constructors, `JsonRpcError { code, message, data:
      Option<serde_json::Value> }`; verify round-trip serde tests for a
      string id and an int id.
- [x] 4.2 Implement `src/tools.rs`: `ToolDefinition { name, description,
      input_schema: ToolInputSchema }`, `ToolInputSchema { type: "object",
      properties: BTreeMap<String, PropertySchema>, required: Vec<String> }`,
      `ToolCallResult { content: Vec<ToolContent>, is_error: Option<bool> }`,
      `ToolContent::text(String)`, and the `ToolCatalog` async trait
      (`list(&self) -> Vec<ToolDefinition>`, `async fn call(&self, name:
      &str, arguments: serde_json::Value) -> ToolCallResult`) plus
      `EmptyCatalog` (empty list, every call returns `is_error: true` with
      "unknown tool: {name}"); verify a unit test that `EmptyCatalog::call`
      on any name returns the error shape (spec: "Unknown tool").
- [x] 4.3 Implement `dispatch(request: &JsonRpcRequest, catalog: &dyn
      ToolCatalog) -> JsonRpcResponse` in `rpc.rs` (or a `dispatch.rs`
      submodule) handling `initialize` (protocol version, tools capability,
      server info), `tools/list` (delegates to `catalog.list()`),
      `tools/call` (validates `params.name` present, delegates to
      `catalog.call`), `shutdown` (empty success result), and
      method-not-found (-32601) for anything else; verify unit tests for
      "Initialize handshake", "Unknown method", and "Tool result shape"
      (success case, via a test catalog returning one tool).

## 5. HTTP server

- [x] 5.1 Implement `src/server.rs`: `McpServer::new(port: u16, catalog:
      Arc<dyn ToolCatalog>, agents: Arc<dyn Fn() -> Vec<skwad_agents::Agent>
      + Send + Sync>) -> Self` and axum router assembly for `GET /health`,
      `GET /`, `POST /mcp`, `GET /mcp`, `GET /api/v1/agent/status`; verify
      `cargo build -p skwad-mcp`.
- [x] 5.2 Implement `GET /health` (200, body indicating up) and `GET /`
      (200, JSON server info: name/version) handlers; verify an
      integration test hitting both on a bound ephemeral port.
- [x] 5.3 Implement `POST /mcp`: parse JSON-RPC body (parse error ->
      `-32700` JSON-RPC error response), short-circuit `notifications/*`
      methods to bare 202, otherwise call `dispatch`, mint a session id on
      `initialize` (else echo the `Mcp-Session-Id` request header or mint
      one), and reply as SSE (`Accept: text/event-stream`) or JSON per the
      Swift reference; verify integration tests for "Initialize handshake"
      (JSON path), the SSE path (`Content-Type: text/event-stream`, body
      starts `event: message`), and "Unknown method" over HTTP.
- [x] 5.4 Implement `GET /mcp`: SSE stream that emits one initial
      `connected` event and stays open; verify an integration test reading
      the first SSE frame.
- [x] 5.5 Implement `GET /api/v1/agent/status`: calls the injected agents
      closure, maps through `status::agent_status`, serializes as a JSON
      array with stable (sorted) keys; verify an integration test for
      "Status reflects live agents" over HTTP.
- [x] 5.6 Implement `McpServer::start(&self) -> Result<()>` (binds the
      configured port via `tokio::net::TcpListener`, spawns
      `axum::serve` on a stored `JoinHandle`) and `McpServer::stop(&self)`
      (aborts the handle); verify an integration test: `start` with port
      `0`, read back the bound port, hit `/health`, `stop`, confirm a
      subsequent request fails to connect.
- [x] 5.7 Verify "Default bind": a test constructing `McpServer` with
      `consts::DEFAULT_PORT` asserts the configured port without binding
      (bind behavior itself is exercised on port 0 elsewhere per repo
      convention against fixed ports in tests).

## 6. Integration and checks

- [x] 6.1 Re-export the public surface (`McpServer`, `McpSessionManager`,
      `McpSession`, `ToolCatalog`, `EmptyCatalog`, `ToolDefinition`,
      `ToolCallResult`, `AgentStatusEntry`, `agent_status`, `JsonRpcRequest`,
      `JsonRpcResponse`, `McpError`, `Result`) from `lib.rs` with module docs
      linking `openspec/specs/mcp-server/spec.md`; verify `cargo doc -p
      skwad-mcp` builds with no warnings.
- [x] 6.2 Run `make rust` (nightly fmt check + clippy `-D warnings` + test +
      build) for the whole workspace and confirm it passes.
- [x] 6.3 Run `openspec validate mcp-server-port` and confirm the change
      validates.
