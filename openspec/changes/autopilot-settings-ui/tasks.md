## 1. Settings scalars

- [x] 1.1 Add `autopilot_enabled: bool` (default `false`), `ai_provider:
      String` (default `"openai"`), `ai_api_key: String` (default empty),
      `autopilot_action: String` (default `"mark"`), and
      `autopilot_custom_prompt: String` (default empty) to
      `knot_core::Settings`, decode-tolerant like other post-hoc fields.
      IMPLEMENTATION NOTE: the struct-level `#[serde(default)]` already
      makes every field decode-tolerant (a missing field falls back to
      `Settings::default()`'s value for it), so no per-field `#[serde]`
      annotation was needed — matching how `restore_conversation_on_launch`
      was added. New `AI_PROVIDER_DEFAULT`/`AUTOPILOT_ACTION_DEFAULT`
      consts added alongside the existing default consts. Verified:
      `loads_swift_shaped_document` (fixture predates these fields) now
      asserts all five default correctly, and `reserializes_with_swift_keys`
      checks all five camelCase keys round-trip.

## 2. Enable + provider section

- [x] 2.1 Render "Enable autopilot" (`Switch` bound to
      `autopilot_enabled`) and an AI Provider section (provider picker,
      API key field, read-only model display), each persisting on change.
      IMPLEMENTATION NOTE: provider picker uses the same `dropdown_menu`
      pattern as the Coding tab's agent-type picker; the API key field
      subscribes to `InputEvent::Change` exactly like the Coding tab's
      options field (same `cx.subscribe` pattern, own
      `ai_api_key_input`/`_ai_api_key_subscription` pair); the model name
      is a pure `ai_model_for(provider)` function (not a stored field),
      matching the Swift reference's `AppSettings.aiModel(for:)` being a
      pure function rather than `@AppStorage`. No GPUI test harness exists
      in this codebase (same finding as every prior settings-UI change),
      so click/edit-driven behavior isn't exercised by an automated test —
      covered by `ai_provider_label_maps_known_providers`,
      `ai_provider_label_defaults_to_openai`, and
      `ai_model_for_matches_swift_reference_defaults` on the pure
      label/model-mapping logic, plus manual verification in 5.2.

## 3. Action section

- [x] 3.1 Render the action picker (Mark/Ask/Auto-continue/Custom) bound
      to `autopilot_action`, persisting on change, and a custom-prompt text
      area bound to `autopilot_custom_prompt` shown only when
      `autopilot_action == "custom"`. IMPLEMENTATION NOTE: used a
      single-line `Input`/`InputState` for the custom prompt rather than
      introducing `gpui-component`'s separate `Textarea`/`TextareaState`
      type, since no multi-line input widget is used elsewhere in this
      file yet and one field doesn't justify adding a second widget
      pattern; can be upgraded to `Textarea` in a follow-up if the
      single-line field proves too cramped in practice. The non-custom
      branch shows the matching `autopilot_action_description` text (ported
      from the Swift reference's per-case `description`), verified by
      `autopilot_action_label_maps_known_actions`,
      `autopilot_action_label_defaults_to_mark`, and
      `autopilot_action_description_is_distinct_per_action`. Not exercised
      by an automated test beyond that (no GPUI test harness, same caveat
      as 2.1).

## 4. Wire into the tab shell

- [x] 4.1 Replace the Autopilot tab's placeholder (from
      `settings-tabs-shell`) with this pane's render method. Verified:
      `Render for SettingsWindow` dispatches `SettingsTab::Autopilot` to
      `render_autopilot` instead of `render_placeholder`.

## 5. Final verification

- [x] 5.1 `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace`, and `cargo build --workspace` all pass
      clean (run outside the sandbox). `cargo fmt --check` was not run
      against nightly rustfmt (not installed in this environment, same gap
      noted in every prior settings-UI change); stable `cargo fmt --check`
      shows only the same pre-existing import-order diffs in unrelated
      files, none in the files touched here.
- [ ] 5.2 Manually open Settings → Autopilot, exercise every control,
      confirm persistence across an app restart. NOT PERFORMED this
      session - no interactive macOS session available. Flagged in the PR
      description as follow-up manual verification before merge.
