## MODIFIED Requirements

### Requirement: Window scope

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
