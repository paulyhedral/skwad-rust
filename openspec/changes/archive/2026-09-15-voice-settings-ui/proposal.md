## Why

The Swift reference has a voice-input feature (push-to-talk speech-to-text
via Apple's on-device speech recognizer) with four backing settings. None
of that exists in the Rust port yet - neither the settings scalars nor the
speech-recognition/global-key-monitor backend (`gpui`/`gpui-kit` expose no
speech API and no global-hotkey-while-unfocused API). The Voice tab (a
placeholder from `settings-tabs-shell`) is where the Swift reference
exposes this feature's configuration.

## What Changes

- Add four new scalars to `knot_core::Settings`: `voice_enabled: bool`
  (default `false`), `voice_engine: String` (default `"apple"` - the only
  option in the Swift reference), `voice_push_to_talk_key: i32` (default
  matching the Swift reference's default modifier key code),
  `voice_auto_insert: bool` (default `true`).
- Fill in the Voice tab with an enable toggle, an engine picker (a single,
  disabled "Apple SpeechAnalyzer" option, matching there being only one
  engine), a read-only display of the configured push-to-talk key (no key
  *recording* UI - see Non-Goals), and an auto-insert toggle.
- **Non-goals, explicitly**: actual speech recognition, the global
  push-to-talk key monitor, and the interactive key-recorder control
  (`KeyRecorderNSView` in the Swift reference is raw AppKit `NSView`
  key-event capture with no `gpui` equivalent surfaced). This change adds
  settings storage and a read-only view of the configured key only; the
  key cannot be changed from this UI until a follow-up change adds a
  `gpui`-native key-capture control. The toggle and fields persist
  correctly but have no runtime effect - no speech recognition or
  push-to-talk backend exists in this port.

## Capabilities

### Modified Capabilities

- `settings-persistence`: adds the four new scalars to the "Single
  settings store" requirement's field list, each decode-tolerant.
- `settings-ui` (pending in `settings-ui-port` / `settings-tabs-shell`,
  not yet archived): fills in the Voice tab's placeholder, with the
  key-recording control explicitly out of scope for this change.

## Impact

- `crates/knot-core/src/settings/mod.rs`: four new `Settings` fields.
- `crates/knot-core/tests/settings.rs`: decode-tolerance coverage for the
  new fields.
- `crates/knot/src/main.rs`: `SettingsWindow::render_voice`.
- No speech-recognition or global-key-monitor implementation - out of
  scope, see Non-Goals.
