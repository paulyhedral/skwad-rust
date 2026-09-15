## 1. Server settings section

- [x] 1.1 Render "Enable MCP server" (`Switch` bound to
      `mcp_server_enabled`) and a port field bound to `mcp_server_port`,
      each persisting on change. IMPLEMENTATION NOTE: the port field is a
      plain `Input`/`InputState` (formatted from `mcp_server_port` as a
      string) subscribed to `InputEvent::Change`, same pattern as the
      Autopilot tab's API key field; `save_mcp_port` only writes/persists
      when the current text parses as a valid `u16`, otherwise it's a
      silent no-op (matches this being a free-text field with no
      validation UI elsewhere in this file either — e.g. Coding's options
      field). No `NumberInput` widget was introduced for one field. Not
      exercised by an automated test (no GPUI test harness, same finding
      as every prior settings-UI change); manual verification in 4.2.
- [x] 1.2 Render a read-only derived URL (`http://127.0.0.1:<port>`).
      Verified with `mcp_server_url_formats_localhost_with_port`.

## 2. Installation command section

- [x] 2.1 Port `mcp_install_command(agent_type: &str, url: &str) -> String`
      from `MCPCommandView.mcpCommandCopy` (Skwad → Knot renamed), covering
      claude/codex/opencode/gemini/copilot. Verified with
      `mcp_install_command_matches_swift_reference_per_agent`, one
      assertion per agent type including copilot's empty string.
- [x] 2.2 Render an agent-type picker (reusing the existing dropdown-menu
      pattern) and the command for the selected type, with a copy button
      calling `cx.write_to_clipboard`. IMPLEMENTATION NOTE: added a
      dedicated `mcp_selected_agent_type` field (not reusing Coding's
      `selected_agent_type`) so switching agents in one tab never affects
      the other, and its dropdown excludes `shell`/`custom1`/`custom2`
      matching the Swift reference's picker filter. When the selected
      type's command is empty (copilot), the label falls back to "No
      manual setup needed." and the Copy button is a no-op rather than
      copying an empty string. Not exercised by an automated test beyond
      the pure-function coverage above (no GPUI test harness, same
      caveat as 1.1).

## 3. Wire into the tab shell

- [x] 3.1 Replace the MCP tab's placeholder (from `settings-tabs-shell`)
      with this pane's render method. Verified: `Render for SettingsWindow`
      dispatches `SettingsTab::Mcp` to `render_mcp` instead of
      `render_placeholder`.

## 4. Final verification

- [x] 4.1 `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo
      build --workspace` all pass clean.
- [ ] 4.2 Manually open Settings → MCP, toggle the server, change the port,
      copy an installation command for two different agent types, and
      paste to confirm the clipboard content. NOT PERFORMED this session -
      no interactive macOS session available. Flagged in the PR
      description as follow-up manual verification before merge.
