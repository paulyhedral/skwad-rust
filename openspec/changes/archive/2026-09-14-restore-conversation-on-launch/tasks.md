## 1. Settings

- [x] 1.1 Add `restore_conversation_on_launch: bool` (default `false`) to
      `knot_core::Settings` (`crates/knot-core/src/settings/mod.rs`),
      decode-tolerant like other post-hoc fields, and verify
      `crates/knot-core/tests/settings.rs` covers default-off and
      legacy-blob-without-field-defaults-off.

## 2. Persist the exact session id

- [x] 2.1 Add `session_id: Option<String>` to `SavedAgent`
      (`crates/knot-core/src/settings/mod.rs` or wherever `SavedAgent` is
      defined), decode-tolerant (missing field defaults to `None`). Verify
      with a `knot-core` test that a legacy blob without the field decodes
      with `session_id: None`.
- [x] 2.2 Update the agent-to-`SavedAgent` persist path so that, when
      `restore_conversation_on_launch` is enabled, the agent's current
      `session_id` is carried into the saved record; when disabled, the
      saved record's `session_id` is always `None` regardless of the
      agent's runtime value. Verify with unit tests for both branches.

## 3. Agent store: resume-session id resolution at load

- [x] 3.1 Add a method on `knot_agents::AgentStore` (or a free function
      taking the store plus a `(folder, agent_type) -> Option<SessionSummary>`
      resolver) that, for each agent currently missing a resume-session id,
      sets it from the agent's own persisted `session_id` if present (set on
      the reconstructed `Agent` from `SavedAgent` during `from_saved`),
      otherwise from the resolver's result if any. Verify with unit tests in
      `crates/knot-agents` covering: persisted id present (used directly,
      resolver not consulted or its result discarded), persisted id absent
      with resolver hit, and persisted id absent with resolver miss (left
      `None`).
- [x] 3.2 Verify agents of a type with no history provider, or with a
      resolver returning `None` and no persisted id, are left with
      `resume_session_id: None` (existing fresh-launch path unchanged) via a
      unit test.

## 4. Wiring in knot

- [x] 4.1 In `build_agent_store` (`crates/knot/src/main.rs`), when
      `settings.restore_layout_on_launch` and
      `settings.restore_conversation_on_launch` are both true, after
      `AgentStore::from_saved` resolve each agent's resume-session id via the
      method from 3.1, using `knot_history::provider(&agent.agent_type)` +
      `HistoryProvider::sessions(&agent.folder)` as the fallback resolver.
      Verify by an integration test asserting: (a) an agent with a persisted
      `session_id` gets that exact id as its `resume_session_id`; (b) an
      agent without one, but whose folder has a matching prior `claude`
      session via history, gets that session's id instead.
- [x] 4.2 Verify the setting being off, or `restore_layout_on_launch` being
      off, performs no persisted-id use and no history lookups, leaving
      `resume_session_id: None` for all agents (test asserts no behavior
      change from current `build_agent_store` output in that case).
- [x] 4.3 Verify manual "Restart" (existing `AgentStore::restart` /
      equivalent path) still clears runtime `session_id` and
      `resume_session_id` unconditionally, regardless of the new setting,
      and does not touch the agent's persisted `session_id` (that only
      changes on the next actual persist) — add/confirm a regression test.

## 5. Settings UI

- [ ] 5.1 SKIPPED: no settings screen exists in the Rust port yet — its
      sibling `restore_layout_on_launch` has no UI toggle either, nor does
      any other scalar setting. Adding one is out of scope for this change;
      revisit once a settings view lands. The scalar is fully wired and
      testable via `Settings` directly in the meantime.

      UI reference for when it does land: `images/screenshots/settings-general.png`
      (General pane, "Startup" section) and the Swift source it was taken
      from, `Skwad/Views/Settings/GeneralSettingsView.swift`, which has
      `Toggle("Restore agents on launch", isOn: $settings.restoreLayoutOnLaunch)`
      immediately above `Toggle("Keep running in menu bar when closed", ...)`
      in that section. The Swift app has no equivalent toggle for this
      feature yet (it's new, per GitHub #60) — but the new toggle belongs in
      that same Startup section, directly below "Restore agents on launch",
      to match the reference app's layout once ported.

## 6. Final verification

- [x] 6.1 Run `make rust` (fmt + clippy + test + build) and confirm it passes
      clean across the workspace.
