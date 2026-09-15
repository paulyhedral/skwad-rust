## Why

`knot_core::Settings` already has full persona CRUD
(`add_persona`/`update_persona`/`remove_persona`/`restore_default_personas`,
each self-persisting) and `AgentEditor` already reads `self.settings.personas`
to populate its persona picker — but there is no way to create, edit, delete,
or restore personas from the UI. The Personas tab (a placeholder from
`settings-tabs-shell`) is where the Swift reference exposes this.

## What Changes

- Fill in the Personas tab: a list of personas (name + a truncated
  instructions preview), each row with edit and delete actions; an "Add
  Persona…" button; a "Restore Defaults" button with a confirmation dialog
  (matching the existing `about_knot` alert-dialog pattern already used in
  `main.rs`).
- Add/edit opens a small secondary window (matching `AgentEditor`'s
  existing pattern) with name and instructions fields; saving calls
  `add_persona`/`update_persona`, canceling discards the draft.
- Delete calls `remove_persona` directly (no confirmation, matching the
  Swift reference, which has no delete confirmation either).
- "Restore Defaults" calls `restore_default_personas` after the user
  confirms.

## Capabilities

### Modified Capabilities

- `settings-ui` (pending in `settings-ui-port` / `settings-tabs-shell`,
  not yet archived): fills in the Personas tab's placeholder with real
  requirements.

## Impact

- `crates/knot/src/main.rs`: `SettingsWindow::render_personas` plus a new
  `PersonaEditor` window struct mirroring `AgentEditor`'s shape.
- No `knot_core::Settings` schema or method changes — all backend already
  exists.
