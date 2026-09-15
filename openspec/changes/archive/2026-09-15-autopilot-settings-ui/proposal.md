## Why

The Swift reference has an Autopilot feature (auto-detect when an agent
needs input, then mark/ask/auto-continue/custom-prompt via an LLM call) with
five backing settings. None of that exists in the Rust port yet — neither
the settings scalars nor the decision-loop backend. The Autopilot tab (a
placeholder from `settings-tabs-shell`) is where the Swift reference exposes
this feature's configuration.

## What Changes

- Add five new scalars to `knot_core::Settings`: `autopilot_enabled: bool`
  (default `false`), `ai_provider: String` (default `"openai"`,
  `"anthropic"`, or `"google"`), `ai_api_key: String` (default empty,
  stored in plain text, matching the Swift reference's own
  `@AppStorage("aiApiKey")` - see design.md Risks), `autopilot_action:
  String` (default `"mark"`; `"ask"` / `"continue"` / `"custom"`), and
  `autopilot_custom_prompt: String` (default empty).
- Fill in the Autopilot tab with the same three sections as the Swift
  reference: an enable toggle; an AI Provider section (provider picker,
  API key field, read-only model name); an Action section (a picker for
  what happens when input is detected, plus a custom-prompt text area
  shown only when "Custom" is selected).
- **Non-goals, explicitly**: the autopilot decision loop itself - detecting
  "needs input," calling the configured LLM provider, and taking the
  resulting action. None of that backend exists in this port. This change
  only adds the settings surface; the toggle and fields persist correctly
  but have no runtime effect until a separate change implements the
  feature. This mirrors the existing precedent of `appearance_mode` and
  `terminal_font_name`/`terminal_font_size` being persisted before
  anything reads them.

## Capabilities

### Modified Capabilities

- `settings-persistence`: adds the five new scalars to the "Single
  settings store" requirement's field list, each decode-tolerant
  (defaulting per above when absent from a legacy blob).
- `settings-ui` (pending in `settings-ui-port` / `settings-tabs-shell`,
  not yet archived): fills in the Autopilot tab's placeholder.

## Impact

- `crates/knot-core/src/settings/mod.rs`: five new `Settings` fields.
- `crates/knot-core/tests/settings.rs`: decode-tolerance coverage for the
  new fields.
- `crates/knot/src/main.rs`: `SettingsWindow::render_autopilot`.
- No autopilot decision-loop implementation - out of scope, see Non-Goals.
