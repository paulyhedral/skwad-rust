## 1. Settings scalars

- [x] 1.1 Add `voice_enabled: bool` (default `false`), `voice_engine:
      String` (default `"apple"`), `voice_push_to_talk_key: i32` (default
      `54`), and `voice_auto_insert: bool` (default `true`) to
      `knot_core::Settings`, decode-tolerant like other post-hoc fields.
      IMPLEMENTATION NOTE: new `VOICE_ENGINE_DEFAULT`/
      `VOICE_PUSH_TO_TALK_KEY_DEFAULT` consts added alongside the existing
      default consts; struct-level `#[serde(default)]` makes the fields
      decode-tolerant automatically (same as the autopilot scalars).
      Verified: `loads_swift_shaped_document` (fixture predates these
      fields) asserts all four default correctly, and
      `reserializes_with_swift_keys` checks all four camelCase keys
      round-trip.

## 2. Voice tab

- [x] 2.1 Add a `key_name_for_code(code: i32) -> String` helper ported
      from `ModifierKeyCode.name(for:)`, covering the same modifier-key
      table. Verified with `key_name_for_code_maps_known_modifier_codes`
      (Right Command/Left Shift/Fn) and
      `key_name_for_code_falls_back_for_unknown_codes` (`"Key 999"`).
- [x] 2.2 Render "Enable voice input" (`Switch` bound to `voice_enabled`),
      a disabled engine picker showing "Apple SpeechAnalyzer", a read-only
      push-to-talk key display (via `key_name_for_code`), and
      "Auto-insert transcription" (`Switch` bound to `voice_auto_insert`) -
      the latter two disabled when `voice_enabled` is false.
      IMPLEMENTATION NOTE: the engine picker is a `Button` with
      `.disabled(true)` (via the `Disableable` trait, newly imported in
      this file — every other picker so far used `.dropdown_menu(...)`
      instead of a plain disabled button, since this is the first pane
      needing a genuinely non-interactive "picker"); the read-only key
      display dims via `.opacity(0.5)` when `voice_enabled` is false,
      since it has no interactive state for `Switch`'s own disabled
      styling to key off of. No `InputState`/`InputEvent` subscriptions
      needed in this pane at all - every control is a toggle, a disabled
      button, or static text. No GPUI test harness exists in this codebase
      (same finding as every prior settings-UI change), so
      click-driven persistence isn't exercised by an automated test —
      covered by the pure `key_name_for_code` tests above and manual
      verification in 4.2.

## 3. Wire into the tab shell

- [x] 3.1 Replace the Voice tab's placeholder (from `settings-tabs-shell`)
      with this pane's render method. Verified: `Render for SettingsWindow`
      dispatches `SettingsTab::Voice` to `render_voice` instead of
      `render_placeholder`.

## 4. Final verification

- [x] 4.1 `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo
      build --workspace` all pass clean (stable rustfmt 1.9.0 was
      available this session, unlike the earlier settings-UI changes - see
      the separate rustfmt-drift fix that landed on `develop`).
- [ ] 4.2 Manually open Settings → Voice, toggle both switches, confirm
      persistence across an app restart and the dependent-disable
      behavior. NOT PERFORMED this session - no interactive macOS session
      available. Flagged in the PR description as follow-up manual
      verification before merge.
