## Context

Depends on `settings-tabs-shell` for the Voice tab slot. None of this
pane's backing fields exist in `knot_core::Settings` yet. No speech
recognition, global key monitor, or push-to-talk backend exists anywhere in
the Rust port; `gpui`/`gpui-kit` (checked the crate source) expose no
speech API and no raw global-key-capture API equivalent to the Swift
reference's `KeyRecorderNSView` (an `NSView` subclass overriding
`keyDown`/`flagsChanged` while first responder).

## Goals / Non-Goals

**Goals:**
- Four new decode-tolerant `Settings` scalars matching the Swift
  reference's field set and defaults.
- A UI section per scalar, with the dependent-disable behavior the Swift
  reference has (push-to-talk key and auto-insert both disabled while
  voice is off).

**Non-Goals:**
- Speech recognition and the push-to-talk global key monitor - large,
  separate backend work with no `gpui` primitive to build on yet.
- An interactive key-recording control - no `gpui` API surfaces raw
  key-down capture outside normal focused-widget input handling the way
  `KeyRecorderNSView` does; building one (or finding/adding the right
  `gpui` primitive) is its own investigation, not bundled into a settings
  screen. The stored key can still be *set* by editing the settings file
  directly, or via a future change once a capture control exists - this
  change makes it visible, not yet editable.

## Decisions

- **Engine picker shown but disabled with one option, not omitted** -
  matches the Swift reference's `Picker` (which lists all
  `voiceEngines`, currently just one); showing it as a real, disabled
  control communicates "this is where engine choice would go" rather than
  hiding the concept entirely.
- **Push-to-talk key rendered read-only rather than stubbed with a fake
  "Record" button that does nothing** - a button that visibly does nothing
  when clicked is worse than an honest read-only display; see Non-Goals.

## Risks / Trade-offs

- [Risk] Shipping a Voice tab whose enable toggle has zero runtime effect
  could read as a bug report → Mitigation: same as `autopilot-settings-ui`
  - called out explicitly in Non-Goals; an inline "Coming soon" note is a
  one-line addition to reconsider at task time if this is a concern at
  review.

## Migration Plan

Additive only - new scalars default to their off/default state, matching a
disabled feature. No data migration for existing users.
