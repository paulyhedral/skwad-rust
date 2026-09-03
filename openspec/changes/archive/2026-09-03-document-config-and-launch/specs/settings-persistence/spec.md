## Purpose

Defines Skwad's single configuration and durable-object store: the scalar
settings, the serialized collections (saved agents, saved workspaces, personas,
bench templates, recent repos), the durable agent field set, first-launch
source-folder detection, the recent-repos MRU, and decode-tolerant migration.
The Rust port MAY use any backing store; the persisted shapes and behaviors are
the contract.

## ADDED Requirements

### Requirement: Single settings store

The system SHALL expose one settings surface holding scalar values (appearance
mode, restore-layout-on-launch, keep-in-menu-bar, MCP enabled, MCP port default
`8766`, source base folder, notification toggle, markdown/mermaid view options,
per-agent-type command and options strings, terminal font name and size, and
similar) and serialized collections (saved agents, saved workspaces, personas,
bench agents, recent repos). Writing a value SHALL persist it immediately.

#### Scenario: Scalar persists across restart

- **WHEN** the MCP port is set to `9000` and the app restarts
- **THEN** the MCP port reads back as `9000`

### Requirement: Durable agent field set

A persisted agent (`SavedAgent`) SHALL store exactly: id, name, avatar (a
non-empty string, default a robot emoji), folder, agent type (default
`claude`), created-by, is-companion, shell command, persona id. Loading SHALL
reconstruct agents with all runtime fields at defaults.

#### Scenario: Avatar default on save

- **WHEN** an agent with no avatar is saved
- **THEN** its persisted avatar is the default robot emoji

### Requirement: Decode-tolerant migration

Decoding a collection SHALL tolerate records that predate later fields:
`SavedAgent` and `BenchAgent` without created-by / is-companion / persona id
SHALL default those; a `Persona` without type / state SHALL default to user /
enabled. A collection blob that fails to decode SHALL yield an empty
collection rather than an error.

#### Scenario: Legacy persona record

- **WHEN** a stored persona has no `type` or `state`
- **THEN** it loads as a user persona in the enabled state

#### Scenario: Corrupt blob

- **WHEN** the saved-agents blob cannot be decoded
- **THEN** the loaded agent list is empty and the app still starts

### Requirement: First-launch source-folder detection

On first launch with no source base folder set, the system SHALL pick the
first existing directory among the common source locations (`~/src`,
`~/source`, `~/sources`) and record it, marking detection done so it does not
run again.

#### Scenario: Picks the first that exists

- **WHEN** `~/src` is absent but `~/source` exists on first launch
- **THEN** the source base folder becomes `~/source`

### Requirement: Recent repositories MRU

The system SHALL maintain a recent-repositories list as a bounded
most-recently-used sequence: adding an entry moves it to the front and
de-duplicates.

#### Scenario: Re-adding moves to front

- **WHEN** repo `B` is added while the list is `[A, B, C]`
- **THEN** the list becomes `[B, A, C]`

### Requirement: Bench templates

The system SHALL store bench agents (reusable templates: id, name, avatar,
folder, agent type, optional shell command, optional persona id). Adding a
bench entry SHALL replace any existing entry with the same folder.

#### Scenario: Same-folder bench entry replaced

- **WHEN** a bench entry is added for a folder that already has one
- **THEN** the old entry is removed and only the new one remains
