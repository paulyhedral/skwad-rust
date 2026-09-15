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
General selected by default when the window opens. Every tab SHALL render
its real pane; none render a placeholder. No "check for updates" control
SHALL be shown anywhere in the window.

#### Scenario: General is the default tab

- **WHEN** the settings window opens
- **THEN** the General tab is selected and its three sections (Appearance,
  Startup, Notifications) are visible

### Requirement: Coding tab

The Coding tab SHALL show a "Source Folder" section (current
`source_base_folder`, a folder picker, and a clear action) and an "Agent
Options" section (an agent-type picker with a per-type options field bound
to `agent_options`). It SHALL NOT show an "Open With" section or editable
custom-command fields — those depend on features not yet in the Rust port.

#### Scenario: Choosing a source folder persists it

- **WHEN** the user picks a directory via "Choose…" in the Source Folder
  section
- **THEN** `source_base_folder` is saved as that directory's path
  immediately

#### Scenario: Clearing the source folder

- **WHEN** the user clicks the clear action next to a configured source
  folder
- **THEN** `source_base_folder` is saved as an empty string

#### Scenario: Editing options for an agent type persists it

- **WHEN** the user selects "Codex" in the agent-type picker and types
  `--flag value` into the Options field
- **THEN** `agent_options["codex"]` is saved as `"--flag value"`
  immediately

#### Scenario: Switching agent type shows that type's own options

- **WHEN** `agent_options["claude"]` is `"--foo"` and the user switches the
  picker from Claude to Codex (with no stored value yet)
- **THEN** the Options field shows empty, not `"--foo"`

### Requirement: Personas tab

The Personas tab SHALL list every non-deleted persona (name and a
truncated instructions preview) with per-row edit and delete actions, an
"Add Persona…" action, and a "Restore Defaults" action gated behind a
confirmation dialog.

#### Scenario: Empty list shows a message, not nothing

- **WHEN** no personas exist
- **THEN** the Personas tab shows "No personas defined" instead of an
  empty list

#### Scenario: Adding a persona

- **WHEN** the user clicks "Add Persona…", enters a name and instructions,
  and saves
- **THEN** a new persona is added via `add_persona` and appears in the list
  immediately

#### Scenario: Editing a persona

- **WHEN** the user clicks the edit action on an existing persona, changes
  its instructions, and saves
- **THEN** `update_persona` is called with that persona's id and the list
  reflects the new instructions

#### Scenario: Canceling an edit discards changes

- **WHEN** the user opens the editor for a persona, changes a field, and
  cancels instead of saving
- **THEN** the persona's stored name and instructions are unchanged

#### Scenario: Deleting a persona

- **WHEN** the user clicks the delete action on a persona
- **THEN** `remove_persona` is called for that persona's id immediately,
  with no confirmation prompt

#### Scenario: Restoring defaults requires confirmation

- **WHEN** the user clicks "Restore Defaults"
- **THEN** a confirmation dialog appears before `restore_default_personas`
  is called; canceling the dialog calls nothing

### Requirement: Autopilot tab

The Autopilot tab SHALL show: an "Enable autopilot" toggle bound to
`autopilot_enabled`; an AI Provider section with a provider picker
(OpenAI/Anthropic/Google) bound to `ai_provider`, an API key text field
bound to `ai_api_key`, and a read-only model-name display derived from the
selected provider; and an Action section with a picker (Mark
conversation/Ask me/Auto-continue/Custom) bound to `autopilot_action`, plus
a custom-prompt text area bound to `autopilot_custom_prompt` shown only
when "Custom" is selected. Every control persists on change. The tab SHALL
NOT claim or imply the feature is functional beyond storing these
preferences - no decision loop runs as a result of this change.

#### Scenario: Enabling autopilot persists

- **WHEN** the user turns on "Enable autopilot"
- **THEN** `autopilot_enabled` is saved as `true` immediately

#### Scenario: Changing provider persists and updates the model display

- **WHEN** the user selects "Anthropic" in the provider picker
- **THEN** `ai_provider` is saved as `"anthropic"` and the model display
  updates to the Anthropic model name

#### Scenario: API key field persists

- **WHEN** the user types a value into the API Key field
- **THEN** `ai_api_key` is saved with that exact value immediately

#### Scenario: Custom prompt only shown for the Custom action

- **WHEN** `autopilot_action` is not `"custom"`
- **THEN** the custom-prompt text area is not shown

#### Scenario: Selecting Custom reveals the prompt editor

- **WHEN** the user selects "Custom" in the action picker
- **THEN** the custom-prompt text area appears, bound to
  `autopilot_custom_prompt`, persisting on edit

### Requirement: Voice tab

The Voice tab SHALL show: an "Enable voice input" toggle bound to
`voice_enabled`; an engine picker showing "Apple SpeechAnalyzer" as the
only, disabled option (reflecting `voice_engine` always being `"apple"`);
a read-only display of the key name for `voice_push_to_talk_key`; and an
"Auto-insert transcription" toggle bound to `voice_auto_insert`. The
push-to-talk key display SHALL NOT be interactively changeable from this
tab. Every persistable control SHALL be disabled when `voice_enabled` is
false, matching the Swift reference's dependent-control disabling.

#### Scenario: Enabling voice input persists

- **WHEN** the user turns on "Enable voice input"
- **THEN** `voice_enabled` is saved as `true` immediately

#### Scenario: Dependent controls disabled while voice is off

- **WHEN** `voice_enabled` is false
- **THEN** the push-to-talk key display and "Auto-insert transcription"
  toggle are shown disabled

#### Scenario: Auto-insert toggle persists

- **WHEN** voice input is enabled and the user turns off "Auto-insert
  transcription"
- **THEN** `voice_auto_insert` is saved as `false` immediately

#### Scenario: Push-to-talk key is read-only

- **WHEN** the Voice tab is open
- **THEN** the configured key's name is shown as static text, with no
  control to record or change it

### Requirement: MCP tab

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

### Requirement: Terminal tab

The Terminal tab SHALL show a "Font" section with a font-name picker bound
to `terminal_font_name` and a numeric size field bound to
`terminal_font_size`, persisting on change. It SHALL NOT show an engine
picker or color pickers.

#### Scenario: Changing the font name persists

- **WHEN** the user picks "JetBrains Mono" in the font picker
- **THEN** `terminal_font_name` is saved as `"JetBrains Mono"` immediately

#### Scenario: Changing the font size persists

- **WHEN** the user sets the size field to `14`
- **THEN** `terminal_font_size` is saved as `14.0` immediately

### Requirement: Tab switching preserves window state

Switching tabs SHALL NOT close the settings window or discard any pane's
in-progress, unsaved state (e.g. a persona editor's draft fields). Returning
to a previously-visited tab SHALL show it exactly as it was left.

#### Scenario: Switching away and back preserves a draft

- **WHEN** the user has unsaved text in one pane's editable field and
  switches to another tab, then back
- **THEN** the unsaved text is still present, unchanged
