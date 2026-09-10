## Why

Agent-to-agent coordination is the reason Skwad exists. The Swift app runs a
local HTTP server exposing an MCP tool surface plus a message queue; the Rust
port needs that behavior stated as requirements, independent of Hummingbird.

## What Changes

- Add `mcp-server` spec: the local HTTP endpoint, JSON-RPC lifecycle, MCP
  tool-list / tool-call dispatch, the plain-JSON status endpoint, session
  tracking, enable/port configuration.
- Add `mcp-messaging` spec: the in-process message queue — send, broadcast,
  check, unread tracking, workspace scoping, companion routing rules, idle-time
  delivery nudge, retention cleanup.
- Add `mcp-tools` spec: the tool catalog and per-tool contracts
  (register-agent, list-agents, send-message, check-messages,
  broadcast-message, list-repos, list-worktrees, create-agent, close-agent,
  create-worktree, set-status, display-markdown, view-mermaid).
- No implementation code.

## Capabilities

### New Capabilities

- `mcp-server`: local MCP HTTP transport, JSON-RPC handling, tool dispatch,
  status endpoint, session lifecycle.
- `mcp-messaging`: agent-to-agent message queue with workspace and companion
  routing rules.
- `mcp-tools`: the concrete MCP tool catalog and each tool's inputs, outputs,
  and error strings.

### Modified Capabilities

None.

## Impact

- New specs under `openspec/specs/mcp-server/`, `openspec/specs/mcp-messaging/`,
  `openspec/specs/mcp-tools/`.
- Depends conceptually on `agent-lifecycle` (agents, companions, workspaces)
  and `activity-detection` (idle triggers delivery).
- Non-goals: hook HTTP routes (see `document-agent-hooks`), the desktop UI for
  messages, MCP resources (`resources/*` is declared but unused).
