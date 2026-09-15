## MODIFIED Requirements

### Requirement: Window scope

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
