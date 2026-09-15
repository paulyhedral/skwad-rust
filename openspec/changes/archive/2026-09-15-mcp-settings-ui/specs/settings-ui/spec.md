## MODIFIED Requirements

### Requirement: Window scope

The MCP tab SHALL show an "Enable MCP server" toggle bound to
`mcp_server_enabled`, a port field bound to `mcp_server_port`, a read-only
server URL derived from the current port, and an installation-command
generator (agent-type picker + the exact command for that type + copy
action), each persisting on change.

#### Scenario: Toggling the MCP server persists

- **WHEN** the user turns off "Enable MCP server"
- **THEN** `mcp_server_enabled` is saved as `false` immediately

#### Scenario: Changing the port persists and updates the URL

- **WHEN** the user sets the port field to `9000`
- **THEN** `mcp_server_port` is saved as `9000` and the displayed URL
  updates to reflect port `9000`

#### Scenario: Installation command matches the selected agent type

- **WHEN** the user selects "Codex" in the agent-type picker
- **THEN** the displayed command is the Codex-specific registration
  command containing the current server URL

#### Scenario: Copy action copies the exact displayed command

- **WHEN** the user clicks the copy action
- **THEN** the system clipboard receives exactly the currently-displayed
  command text
