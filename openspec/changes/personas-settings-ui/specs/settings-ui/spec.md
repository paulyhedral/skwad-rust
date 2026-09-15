## MODIFIED Requirements

### Requirement: Window scope

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
