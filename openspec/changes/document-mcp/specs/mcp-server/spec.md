## Purpose

Defines the local MCP HTTP server: its endpoints, JSON-RPC lifecycle, MCP
tool-list and tool-call dispatch, the plain-JSON status endpoint used by
external tooling, and MCP session tracking. Behavior is the intended target for
the Rust port; the transport is an implementation choice (axum/hyper), not part
of the contract.

## ADDED Requirements

### Requirement: Local bind and configuration

The system SHALL run the MCP server bound to loopback only (`127.0.0.1`) on a
configurable port, default `8766`. The server SHALL be enabled by default and
MAY be disabled by configuration; when disabled, no agent registration prompts
are scheduled and no port is opened.

#### Scenario: Default bind

- **WHEN** the app starts with default settings
- **THEN** the MCP server listens on `127.0.0.1:8766`

#### Scenario: Disabled by configuration

- **WHEN** the MCP server is disabled in settings
- **THEN** no socket is opened and agents receive no registration prompt

### Requirement: Health and info endpoints

The system SHALL expose `GET /health` returning a success indicator and
`GET /` returning basic server info. These SHALL respond without requiring MCP
initialization.

#### Scenario: Health check

- **WHEN** `GET /health` is requested
- **THEN** the response status is 200 with a body indicating the server is up

### Requirement: JSON-RPC endpoint and lifecycle

The system SHALL accept MCP JSON-RPC 2.0 requests at `POST /mcp` and SHALL
implement `initialize` (returning protocol version, server capabilities
declaring tools, and server info), `notifications/initialized`, `tools/list`,
and `tools/call`. Unknown methods SHALL return a JSON-RPC method-not-found
error. A `GET /mcp` request SHALL open a Server-Sent Events stream.

#### Scenario: Initialize handshake

- **WHEN** a client sends `initialize`
- **THEN** the response contains a protocol version, a `tools` capability, and
  server name/version

#### Scenario: Unknown method

- **WHEN** a client calls a method that is not implemented
- **THEN** the response is a JSON-RPC error with code for method-not-found and
  the request id echoed

### Requirement: Tool-list and tool-call dispatch

`tools/list` SHALL return every tool in the catalog with name, description, and
a JSON input schema. `tools/call` SHALL route to the named tool handler and
return an MCP tool result whose `content` is a single text item; failures
SHALL set `isError` true with a human-readable message rather than a transport
error.

#### Scenario: Tool result shape

- **WHEN** any tool call succeeds
- **THEN** the result has one text content item and `isError` is absent or
  false

#### Scenario: Unknown tool

- **WHEN** `tools/call` names a tool not in the catalog
- **THEN** the result has `isError` true and text naming the unknown tool

### Requirement: Status endpoint

The system SHALL expose `GET /api/v1/agent/status` returning a JSON array with
one entry per agent: agent id, name, folder, state, agent-set status text,
registered flag, agent type, session id when present, and hook metadata when
non-empty. Keys SHALL be stably ordered.

#### Scenario: Status reflects live agents

- **WHEN** two agents exist, one registered with a session id
- **THEN** the array has two entries and the registered agent's entry includes
  its `session_id`

### Requirement: MCP session tracking

The system SHALL create an MCP session per agent on demand, keyed by an opaque
id, tracking created-at and last-activity times. Creating a session for an
agent that already has one SHALL replace the old session. Sessions idle beyond
a timeout (default 1 hour) MAY be reclaimed.

#### Scenario: One session per agent

- **WHEN** a session is created for an agent that already has one
- **THEN** the previous session id is no longer resolvable and the new one is
