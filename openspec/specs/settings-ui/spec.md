## Purpose

Defines the settings window in the `knot` app: what it shows, how each
control maps to a `Settings` scalar, and how edits are persisted.

## Requirements

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

The settings window SHALL show a tab strip with seven tabs — General,
Coding, Personas, Autopilot, Voice, MCP, Terminal — in that order, with
General selected by default when the window opens. A tab whose pane has not
yet been implemented SHALL render a placeholder stating it is not yet
available, rather than being omitted or disabled. No "check for updates"
control SHALL be shown anywhere in the window.

#### Scenario: General is the default tab

- **WHEN** the settings window opens
- **THEN** the General tab is selected and its three sections (Appearance,
  Startup, Notifications) are visible

#### Scenario: Unimplemented pane shows a placeholder

- **WHEN** a tab whose pane has not yet been built (e.g. Coding, before its
  own change lands) is selected
- **THEN** the pane shows a "not yet available" placeholder instead of an
  empty or missing tab

### Requirement: Tab switching preserves window state

Switching tabs SHALL NOT close the settings window or discard any pane's
in-progress, unsaved state (e.g. a persona editor's draft fields). Returning
to a previously-visited tab SHALL show it exactly as it was left.

#### Scenario: Switching away and back preserves a draft

- **WHEN** the user has unsaved text in one pane's editable field and
  switches to another tab, then back
- **THEN** the unsaved text is still present, unchanged
