## Why

`knot_core::Settings` already models `mcp_server_enabled` and
`mcp_server_port`, and the running app already computes an MCP server URL
(`start_mcp_server` in `main.rs` binds to it), but nothing exposes these to
the user, and there is no way to see the copy-paste command needed to
register an external coding agent (Claude, Codex, OpenCode, Gemini,
Copilot) against Knot's MCP server. The MCP tab (a placeholder from
`settings-tabs-shell`) is where the Swift reference exposes both.

## What Changes

- Fill in the MCP tab with three sections:
  - **About**: one line of static text explaining what the MCP server does.
  - **Server Settings**: an "Enable MCP server" toggle bound to
    `mcp_server_enabled`, a numeric port field bound to `mcp_server_port`,
    and a read-only display of the resulting server URL
    (`http://127.0.0.1:<port>`).
  - **Installation Command**: an agent-type picker (Claude/Codex/OpenCode/
    Gemini/Copilot) and the exact command to run to register that agent
    type against Knot's MCP server, with a copy-to-clipboard action.
- Port a small static command table from the Swift reference's
  `MCPCommandView` (renamed Skwad → Knot), rather than reusing
  `knot-agent-launch::mcp_arguments` — that function builds inline
  auto-launch arguments for agents Knot itself starts, a different
  mechanism from the manual "register an external agent instance" command
  shown here.

## Capabilities

### Modified Capabilities

- `settings-ui` (pending in `settings-ui-port` / `settings-tabs-shell`,
  not yet archived): fills in the MCP tab's placeholder with real
  requirements.

## Impact

- `crates/knot/src/main.rs`: `SettingsWindow::render_mcp`, plus a small
  `mcp_install_command(agent_type, url) -> String` free function ported
  from `MCPCommandView.mcpCommandCopy`.
- No `knot_core::Settings` schema changes — both fields already exist.
