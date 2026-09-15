## Why

`knot_core::Settings` already models the source base folder
(`source_base_folder`) and per-agent-type command/options overrides
(`agent_commands`, `agent_options`), but nothing in the Rust `knot` app lets
a user view or change them. The Coding tab (added as a placeholder by
`settings-tabs-shell`) is where the Swift reference exposes both.

## What Changes

- Fill in the Coding tab with two sections, matching the Swift reference's
  scope for what the Rust port already models:
  - **Source Folder**: shows the current `source_base_folder` (or "Not
    configured"), a "Choose…" button opening a directory picker (via the
    existing `PathPromptOptions` API already used elsewhere in
    `main.rs`), and a clear button.
  - **Agent Options**: an agent-type picker (Claude/Codex/OpenCode/
    Gemini/Copilot/Shell, matching the existing agent-type picker used in
    `AgentEditor`) plus a free-text "Options" field bound to
    `agent_options` for the selected type, persisting on change.
- **Non-goals**: the "Open With (⌘⇧O)" section and the two "Custom
  command" slots (`custom1`/`custom2` with editable base commands) — the
  Rust port has no "open in IDE" feature yet and no per-agent custom
  command scalar in `knot_core::Settings` (only `agent_options`, not a
  command override); both are separate feature work, not settings-surface
  work, and are out of scope here.

## Capabilities

### Modified Capabilities

- `settings-ui` (pending in `settings-ui-port` / `settings-tabs-shell`,
  not yet archived): fills in the Coding tab's placeholder with real
  requirements.

## Impact

- `crates/knot/src/main.rs`: `SettingsWindow::render_coding` (or
  equivalent), reusing `PathPromptOptions` and the existing dropdown-menu
  picker pattern.
- No `knot_core::Settings` schema changes — both fields already exist.
