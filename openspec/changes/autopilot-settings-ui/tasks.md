## 1. Settings scalars

- [ ] 1.1 Add `autopilot_enabled: bool` (default `false`), `ai_provider:
      String` (default `"openai"`), `ai_api_key: String` (default empty),
      `autopilot_action: String` (default `"mark"`), and
      `autopilot_custom_prompt: String` (default empty) to
      `knot_core::Settings`, decode-tolerant like other post-hoc fields.
      Verify `crates/knot-core/tests/settings.rs` covers default values and
      a legacy-blob-without-these-fields case.

## 2. Enable + provider section

- [ ] 2.1 Render "Enable autopilot" (`Switch` bound to
      `autopilot_enabled`) and an AI Provider section (provider picker,
      API key field, read-only model display), each persisting on change.
      Verify with tests: toggling autopilot saves the field; changing
      provider saves `ai_provider` and updates the model display; editing
      the API key field saves `ai_api_key`.

## 3. Action section

- [ ] 3.1 Render the action picker (Mark/Ask/Auto-continue/Custom) bound
      to `autopilot_action`, persisting on change, and a custom-prompt text
      area bound to `autopilot_custom_prompt` shown only when
      `autopilot_action == "custom"`. Verify with tests: selecting each
      action saves the field; the custom-prompt area is absent for
      non-custom actions and present (and persists edits) for custom.

## 4. Wire into the tab shell

- [ ] 4.1 Replace the Autopilot tab's placeholder (from
      `settings-tabs-shell`) with this pane's render method. Verify the
      Autopilot tab shows the three sections instead of the placeholder
      text.

## 5. Final verification

- [ ] 5.1 `cargo fmt --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `cargo test --workspace`, and `cargo build --workspace`
      all pass clean.
- [ ] 5.2 Manually open Settings → Autopilot, exercise every control,
      confirm persistence across an app restart. Record whether this
      manual pass was performed (requires an interactive macOS session).
