# personas Specification

## Purpose
Defines reusable system-prompt personas: the model, the system-versus-user
distinction, the enabled / disabled / deleted lifecycle, the personas shipped
with the app, install-on-startup, restore-defaults, and the different delete
semantics for system and user personas.

## Requirements

### Requirement: Persona model

A persona SHALL carry an id, a name, instruction text, a type
(`system` or `user`), and a state (`enabled`, `disabled`, or `deleted`). New
personas default to type `user`, state `enabled`.

#### Scenario: New persona defaults

- **WHEN** a persona is created from a name and instructions only
- **THEN** it is a user persona in the enabled state

### Requirement: Active versus stored personas

The system SHALL distinguish the full stored list (including deleted) used for
persistence from the active list (excluding deleted) used for selection. The
active list SHALL be sorted by name. Persona lookup by id SHALL resolve only
against active personas.

#### Scenario: Deleted persona not selectable

- **WHEN** a persona is in the deleted state
- **THEN** it does not appear in the active list and lookup by its id returns
  nothing

### Requirement: Shipped defaults and install-on-startup

The system SHALL ship a fixed set of default system personas with stable ids.
On startup, any shipped default whose id is not already stored SHALL be
appended. Existing stored personas SHALL NOT be overwritten by this step.

#### Scenario: Missing default is added

- **WHEN** the app starts and a shipped default persona id is absent from the
  store
- **THEN** that default is added

#### Scenario: User edit to a default is preserved on startup

- **WHEN** a stored system persona was renamed by the user and the app restarts
- **THEN** startup install does not revert the name

### Requirement: Delete semantics differ by type

Deleting a system persona SHALL be a soft delete: its state becomes `deleted`
and the record is retained. Deleting a user persona SHALL be a hard delete: the
record is removed entirely.

#### Scenario: System persona soft-deleted

- **WHEN** a system persona is deleted
- **THEN** it remains in the stored list with state `deleted`

#### Scenario: User persona hard-deleted

- **WHEN** a user persona is deleted
- **THEN** it is absent from the stored list

### Requirement: Restore defaults

A restore-defaults action SHALL reset every shipped system persona to its
original name, instructions, and enabled state, add back any shipped default
that is missing, and leave user personas untouched.

#### Scenario: Restore re-enables a disabled default

- **WHEN** a system persona was disabled and restore-defaults runs
- **THEN** it returns to enabled with its original name and instructions, and
  user personas are unchanged
