## MODIFIED Requirements

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
