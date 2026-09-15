## 1. Settings scalars

- [ ] 1.1 Add `voice_enabled: bool` (default `false`), `voice_engine:
      String` (default `"apple"`), `voice_push_to_talk_key: i32` (default
      `54`), and `voice_auto_insert: bool` (default `true`) to
      `knot_core::Settings`, decode-tolerant like other post-hoc fields.
      Verify `crates/knot-core/tests/settings.rs` covers default values and
      a legacy-blob-without-these-fields case.

## 2. Voice tab

- [ ] 2.1 Add a `key_name_for_code(code: i32) -> String` helper ported
      from `ModifierKeyCode.name(for:)`, covering the same modifier-key
      table. Verify with unit tests for at least three known codes and one
      unknown code (falls back to `"Key <code>"`).
- [ ] 2.2 Render "Enable voice input" (`Switch` bound to `voice_enabled`),
      a disabled engine picker showing "Apple SpeechAnalyzer", a read-only
      push-to-talk key display (via `key_name_for_code`), and
      "Auto-insert transcription" (`Switch` bound to `voice_auto_insert`) -
      the latter two disabled when `voice_enabled` is false. Verify with
      tests: toggling voice-enabled saves the field; auto-insert toggling
      saves the field when enabled; both dependent controls report
      disabled when `voice_enabled` is false.

## 3. Wire into the tab shell

- [ ] 3.1 Replace the Voice tab's placeholder (from `settings-tabs-shell`)
      with this pane's render method. Verify the Voice tab shows the four
      controls instead of the placeholder text.

## 4. Final verification

- [ ] 4.1 `cargo fmt --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `cargo test --workspace`, and `cargo build --workspace`
      all pass clean.
- [ ] 4.2 Manually open Settings → Voice, toggle both switches, confirm
      persistence across an app restart and the dependent-disable behavior.
      Record whether this manual pass was performed (requires an
      interactive macOS session).
