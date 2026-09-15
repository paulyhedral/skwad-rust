## Context

Depends on `settings-tabs-shell` for the Personas tab slot. Every backend
operation this pane needs already exists and self-persists
(`Settings::add_persona`/`update_persona`/`remove_persona`/
`restore_default_personas`), and `AgentEditor` already establishes the
"small secondary window with `InputState` fields, `store`/`settings`
handles, Save/Cancel" pattern this pane's editor reuses.

## Goals / Non-Goals

**Goals:**
- List + add + edit + delete + restore-defaults, exactly matching the
  Swift reference's Personas tab.

**Non-Goals:**
- Persona *usage* (picking a persona for an agent) — already implemented
  in `AgentEditor`; untouched here.
- Any change to persona data shape or CRUD semantics — pure UI over
  existing methods.

## Decisions

- **A `PersonaEditor` window struct, mirroring `AgentEditor`** — same
  shape (owned `Settings` clone, `InputState` fields, Save/Cancel), opened
  via the same `cx.open_window` + `Root::new` pattern already used for
  `AgentEditor`/`WorkspaceWindow`. A sheet/modal-in-window pattern doesn't
  exist elsewhere in this codebase; a secondary window does, twice over.
- **Delete has no confirmation, restore-defaults does** — matches the
  Swift reference exactly (`Button { settings.removePersona(persona) }` vs.
  `.alert("Restore Defaults", ...)`), not a UI improvisation.
- **Editor holds its own `Settings` clone, not the live one** — same
  isolation `AgentEditor` and `WorkspaceWindow` already have; Save calls
  `add_persona`/`update_persona` on that clone (which persists to disk),
  Cancel simply closes the window without calling either, so nothing is
  written until Save.

## Risks / Trade-offs

- [Risk] The Personas tab's list won't reflect a persona added/edited via
  the secondary window until the tab re-renders → Mitigation: `cx.notify()`
  on the parent `SettingsWindow` entity after the editor's Save action,
  same as how `WorkspaceWindow::create_agent` already notifies after
  mutating shared state from a related window.

## Migration Plan

Additive only — fills in an existing placeholder tab, no data migration.
