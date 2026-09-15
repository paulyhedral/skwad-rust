## Purpose

Defines the General settings window in the `knot` app: what it shows, how
each control maps to a `Settings` scalar, and how edits are persisted.

## ADDED Requirements

### Requirement: Settings window is reachable from the app menu

The system SHALL expose a "Settings…" item in the app menu, bound to the
platform-standard shortcut (`Cmd+,`), that opens a single General settings
window. Activating the item while the window is already open SHALL bring
the existing window forward rather than opening a second one.

#### Scenario: Opening settings from the menu

- **WHEN** the user selects "Settings…" from the app menu
- **THEN** a settings window opens showing the General pane

#### Scenario: Reactivating an open settings window

- **WHEN** the settings window is already open and the user selects
  "Settings…" again
- **THEN** the existing window is raised and focused; no second window opens

### Requirement: Appearance control

The window SHALL show an "Appearance" section with a picker bound to
`appearance_mode`, offering Auto, System, Light, and Dark. Changing the
selection SHALL persist the new value immediately.

#### Scenario: Changing appearance mode persists

- **WHEN** the user picks "Dark" in the Appearance picker
- **THEN** `appearance_mode` is saved as `"dark"` before the picker closes

### Requirement: Startup controls

The window SHALL show a "Startup" section with:

- A "Restore agents on launch" toggle bound to `restore_layout_on_launch`.
- A "Restore last conversation" toggle bound to
  `restore_conversation_on_launch`, enabled only when
  `restore_layout_on_launch` is on (it has no effect otherwise) and
  disabled — not hidden — when it is off, so the setting's existence stays
  visible.
- A "Keep running in menu bar when closed" toggle bound to
  `keep_in_menu_bar`.

Each toggle SHALL persist its new value immediately on change.

#### Scenario: Restore-conversation toggle disabled when layout restore is off

- **WHEN** `restore_layout_on_launch` is off
- **THEN** the "Restore last conversation" toggle is shown disabled,
  reflecting its stored value but not accepting input

#### Scenario: Turning off layout restore does not clear conversation restore

- **WHEN** `restore_conversation_on_launch` is on and the user turns off
  "Restore agents on launch"
- **THEN** `restore_conversation_on_launch`'s stored value is unchanged, and
  its toggle becomes disabled

#### Scenario: Toggling a startup switch persists

- **WHEN** the user turns on "Keep running in menu bar when closed"
- **THEN** `keep_in_menu_bar` is saved as `true` immediately

### Requirement: Notifications control

The window SHALL show a "Notifications" section with a "Desktop
notifications" toggle bound to `desktop_notifications_enabled`, persisting
immediately on change.

#### Scenario: Toggling desktop notifications persists

- **WHEN** the user turns off "Desktop notifications"
- **THEN** `desktop_notifications_enabled` is saved as `false` immediately

### Requirement: Window scope

The General pane SHALL be the only pane in this window; no tab strip or
navigation to other panes (Coding, Personas, Autopilot, Voice, MCP,
Terminal) SHALL be present, and no "check for updates" control SHALL be
shown.

#### Scenario: No other panes present

- **WHEN** the settings window is open
- **THEN** only the General pane's three sections (Appearance, Startup,
  Notifications) are visible, with no tabs for other panes
