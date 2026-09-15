## MODIFIED Requirements

### Requirement: Single settings store

The system SHALL expose one settings surface holding scalar values
(appearance mode, restore-layout-on-launch, restore-conversation-on-launch,
keep-in-menu-bar, MCP enabled, MCP port default `8766`, source base folder,
notification toggle, markdown/mermaid view options, per-agent-type command
and options strings, terminal font name and size, autopilot and AI-provider
scalars, voice-enabled, voice engine, push-to-talk key code, voice
auto-insert, and similar) and serialized collections (saved agents, saved
workspaces, personas, bench agents, recent repos). Writing a value SHALL
persist it immediately.

`restore-conversation-on-launch` defaults to off (disabled) and is
decode-tolerant: a persisted settings blob written before this field existed
SHALL load with it defaulted to off, not an error.

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

#### Scenario: Voice scalars default for legacy settings

- **WHEN** a persisted settings blob written before the voice scalars
  existed is decoded
- **THEN** it loads successfully with `voice_enabled` false, `voice_engine`
  `"apple"`, `voice_push_to_talk_key` `54`, and `voice_auto_insert` true
