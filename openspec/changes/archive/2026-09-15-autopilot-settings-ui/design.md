## Context

Depends on `settings-tabs-shell` for the Autopilot tab slot. Unlike every
other pane change so far, none of this pane's backing fields exist in
`knot_core::Settings` yet - this change adds them (a `settings-persistence`
delta) as well as the UI (a `settings-ui` delta). No autopilot decision-loop
logic exists anywhere in the Rust port; this change does not add any.

## Goals / Non-Goals

**Goals:**
- Five new decode-tolerant `Settings` scalars matching the Swift
  reference's field set and defaults exactly.
- A UI section per scalar group, matching the Swift reference's layout.

**Non-Goals:**
- The autopilot decision loop (detecting "needs input," calling an LLM,
  taking the resulting action) - a separate, much larger change requiring
  new backend infrastructure (an LLM HTTP client, integration with
  `knot-activity`'s state detection, and a new decision-loop crate or
  module). Explicitly deferred; see proposal.md.
- Encrypting or otherwise specially protecting `ai_api_key` beyond what the
  rest of `Settings` already gets (plain JSON on disk) - see Risks.

## Decisions

- **`ai_api_key` stored as a plain `String` in the settings blob, matching
  the Swift reference's `@AppStorage`** - not `Keychain`. The reference app
  never used Keychain for this value either, so matching it is parity, not
  a regression; a future hardening change could migrate both apps to
  Keychain/OS-provided secret storage, but that is out of scope here and
  shouldn't block shipping the setting.
- **Model name is a read-only derived display, not stored** - matches the
  Swift reference (`AppSettings.aiModel(for:)` is a pure function of
  provider, not a stored field); ported the same way rather than adding a
  redundant scalar.

## Risks / Trade-offs

- [Risk] `ai_api_key` is a real secret (an LLM provider API key) stored in
  plain text in a settings file that also gets backed up, synced, or
  potentially logged in error reports → Mitigation: matches existing,
  accepted behavior in the Swift reference; flagging here so it's a
  conscious carry-over rather than a silent gap. Do not log
  `Settings` debug output that includes this field; a future change should
  consider redacting it from any `Debug`/`Serialize` diagnostic path if one
  gets added later.
- [Risk] Shipping a fully-wired-looking Autopilot tab whose toggle has zero
  runtime effect could read as a bug report ("I turned it on and nothing
  happens") → Mitigation: called out explicitly in the proposal's
  Non-Goals; if this is a concern at review time, an inline note text in
  the tab ("Coming soon") is a one-line addition to reconsider before
  merge, deferred to task time rather than blocking the proposal.

## Migration Plan

Additive only - new scalars default to their off/empty state, matching a
disabled feature. No data migration for existing users.
