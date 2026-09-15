## 1. Server settings section

- [ ] 1.1 Render "Enable MCP server" (`Switch` bound to
      `mcp_server_enabled`) and a port field bound to `mcp_server_port`,
      each persisting on change. Verify with tests asserting each control's
      handler mutates and saves the correct field.
- [ ] 1.2 Render a read-only derived URL (`http://127.0.0.1:<port>`).
      Verify with a unit test that the derived-URL function produces the
      expected string for a given port.

## 2. Installation command section

- [ ] 2.1 Port `mcp_install_command(agent_type: &str, url: &str) -> String`
      from `MCPCommandView.mcpCommandCopy` (Skwad → Knot renamed), covering
      claude/codex/opencode/gemini/copilot. Verify with unit tests covering
      each agent type's exact expected string.
- [ ] 2.2 Render an agent-type picker (reusing the existing dropdown-menu
      pattern) and the command for the selected type, with a copy button
      calling `cx.write_to_clipboard`. Verify with a test that selecting a
      different agent type updates the displayed command to match
      `mcp_install_command` for that type.

## 3. Wire into the tab shell

- [ ] 3.1 Replace the MCP tab's placeholder (from `settings-tabs-shell`)
      with this pane's render method. Verify the MCP tab shows the three
      sections instead of the placeholder text.

## 4. Final verification

- [ ] 4.1 `cargo fmt --check -p knot`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and
      `cargo build --workspace` all pass clean.
- [ ] 4.2 Manually open Settings → MCP, toggle the server, change the port,
      copy an installation command for two different agent types, and
      paste to confirm the clipboard content. Record whether this manual
      pass was performed (requires an interactive macOS session).
