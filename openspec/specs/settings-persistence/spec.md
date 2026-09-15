# settings-persistence Specification

## Purpose
Defines Knot's single configuration and durable-object store: the scalar
settings, the serialized collections (saved agents, saved workspaces, personas,
bench templates, recent repos), the durable agent field set, first-launch
source-folder detection, the recent-repos MRU, and decode-tolerant migration.
The Rust port MAY use any backing store; the persisted shapes and behaviors are
the contract.

## Requirements

### Requirement: Single settings store

The system SHALL expose one settings surface holding scalar values (appearance
mode, restore-layout-on-launch, restore-conversation-on-launch,
keep-in-menu-bar, MCP enabled, MCP port default `8766`, source base folder,
notification toggle, markdown/mermaid view options, per-agent-type command and
options strings, terminal font name and size, autopilot-enabled, AI provider,
AI API key, autopilot action, autopilot custom prompt, voice-enabled, voice
engine, push-to-talk key code, voice auto-insert, and similar) and serialized
collections (saved agents, saved workspaces, personas, bench agents, recent
repos). Writing a value SHALL persist it immediately.

`restore-conversation-on-launch` defaults to off (disabled) and is
decode-tolerant: a persisted settings blob written before this field existed
SHALL load with it defaulted to off, not an error.

`autopilot_enabled` defaults to `false`, `ai_provider` defaults to
`"openai"`, `ai_api_key` defaults to empty, `autopilot_action` defaults to
`"mark"`, and `autopilot_custom_prompt` defaults to empty. All five are
decode-tolerant: a persisted settings blob written before they existed SHALL
load with each defaulted, not an error.

`voice_enabled` defaults to `false`, `voice_engine` defaults to `"apple"`,
`voice_push_to_talk_key` defaults to `54` (Right Command, matching the
Swift reference's `ModifierKeyCode.rightCommand`), and `voice_auto_insert`
defaults to `true`. All four are decode-tolerant: a persisted settings blob
written before they existed SHALL load with each defaulted, not an error.

#### Scenario: Scalar persists across restart

- **WHEN** the MCP port is set to `9000` and the app restarts
- **THEN** the MCP port reads back as `9000`

#### Scenario: Restore-conversation-on-launch defaults off for legacy settings

- **WHEN** a persisted settings blob written before `restore-conversation-on-launch`
  existed is decoded
- **THEN** it loads successfully with `restore-conversation-on-launch` set to
  off

#### Scenario: Autopilot scalars default for legacy settings

- **WHEN** a persisted settings blob written before the autopilot scalars
  existed is decoded
- **THEN** it loads successfully with `autopilot_enabled` false,
  `ai_provider` `"openai"`, `ai_api_key` empty, `autopilot_action`
  `"mark"`, and `autopilot_custom_prompt` empty

#### Scenario: Voice scalars default for legacy settings

- **WHEN** a persisted settings blob written before the voice scalars
  existed is decoded
- **THEN** it loads successfully with `voice_enabled` false, `voice_engine`
  `"apple"`, `voice_push_to_talk_key` `54`, and `voice_auto_insert` true

### Requirement: Durable agent field set

A persisted agent (`SavedAgent`) SHALL store exactly: id, name, avatar (a
non-empty string, default a robot emoji), folder, agent type (default
`claude`), created-by, is-companion, shell command, persona id, and an
optional session id. Loading SHALL reconstruct agents with all runtime fields
at defaults except session id, which SHALL be reconstructed from the
persisted value when present (see `agent-lifecycle` - Durable versus runtime
fields for how it is then used to resolve resume-session id).

Session id SHALL only ever be written to a non-`None` value when
`restore-conversation-on-launch` is enabled at persist time; whenever it is
disabled, saving an agent SHALL persist session id as `None` regardless of
the agent's runtime session id.

#### Scenario: Avatar default on save

- **WHEN** an agent with no avatar is saved
- **THEN** its persisted avatar is the default robot emoji

#### Scenario: Session id omitted by default

- **WHEN** `restore-conversation-on-launch` is disabled (the default) and an
  agent with an active runtime session id is saved
- **THEN** the persisted agent's session id is `None`

### Requirement: Decode-tolerant migration

Decoding a collection SHALL tolerate records that predate later fields:
`SavedAgent` and `BenchAgent` without created-by / is-companion / persona id
SHALL default those, and `SavedAgent` without session id SHALL default it to
`None`; a `Persona` without type / state SHALL default to user / enabled. A
collection blob that fails to decode SHALL yield an empty collection rather
than an error.

#### Scenario: Legacy persona record

- **WHEN** a stored persona has no `type` or `state`
- **THEN** it loads as a user persona in the enabled state

#### Scenario: Legacy SavedAgent record without session id

- **WHEN** a persisted `SavedAgent` record predates the session id field
- **THEN** it loads with session id defaulted to `None`

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
