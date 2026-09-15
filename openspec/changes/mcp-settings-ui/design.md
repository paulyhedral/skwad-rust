## Context

Depends on `settings-tabs-shell` for the MCP tab slot. `mcp_server_enabled`
and `mcp_server_port` already exist and are read by `start_mcp_server` in
`main.rs`; this only surfaces them. `gpui`'s `App::write_to_clipboard`
(re-exported through `gpui_kit`) provides the copy action with no new
dependency.

## Goals / Non-Goals

**Goals:**
- Toggle + port field + derived URL display, matching `settings-ui-port`'s
  existing Switch/persist pattern.
- A ported, static installation-command table (Skwad → Knot renamed) with
  a copy button.

**Non-Goals:**
- Actually restarting the running MCP server when the port changes at
  runtime — `mcp_server_port` already only takes effect on next launch
  (per how `start_mcp_server` reads `settings` once at startup); this
  change doesn't alter that, it only exposes the value.
- Reusing `knot-agent-launch::mcp_arguments` — different mechanism (inline
  auto-launch args vs. a manual external-registration command); see
  proposal.md.

## Decisions

- **Static string table ported from `MCPCommandView.mcpCommandCopy`,
  not derived from `knot-agent-launch`** — the two "MCP install command"
  concepts in the Swift app are already distinct
  (`mcp_arguments`'s Rust equivalent doesn't exist for this use case); a
  small standalone table is simpler and more honest than forcing a shared
  abstraction over two different problems.
- **Read-only URL, not editable** — matches the Swift reference
  (`LabeledContent("URL") { Text(...) }`, no `TextField`); the URL is
  derived, not stored.

## Risks / Trade-offs

- [Risk] Displaying "enabled" while the running server actually started
  with a different (stale) enabled/port value, since it only re-reads at
  launch → Mitigation: matches existing Swift/Rust behavior exactly (the
  setting has always been launch-time-effective only); out of scope to
  change here.

## Migration Plan

Additive only — fills in an existing placeholder tab, no data migration.
