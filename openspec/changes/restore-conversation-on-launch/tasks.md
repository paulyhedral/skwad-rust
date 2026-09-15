## 1. Settings

- [ ] 1.1 Add `restore_conversation_on_launch: bool` (default `false`) to
      `skwad_core::Settings` (`crates/skwad-core/src/settings/mod.rs`),
      decode-tolerant like other post-hoc fields, and verify
      `crates/skwad-core/tests/settings.rs` covers default-off and
      legacy-blob-without-field-defaults-off.

## 2. Agent store: resume-session id lookup at load

- [ ] 2.1 Add a method on `skwad_agents::AgentStore` (or a free function taking
      the store plus a lookup closure/trait) that, given a `(folder, agent
      type) -> Option<SessionSummary>` resolver, sets `resume_session_id` on
      each agent currently missing one. Verify with a unit test in
      `crates/skwad-agents` that resume-session id is set when the resolver
      returns a session and left `None` when it returns `None`.
- [ ] 2.2 Verify agents of a type with no history provider, or with a
      resolver returning `None`, are left with `resume_session_id: None`
      (existing fresh-launch path unchanged) via a unit test.

## 3. Wiring in skwad

- [ ] 3.1 In `build_agent_store` (`crates/skwad/src/main.rs`), when
      `settings.restore_layout_on_launch` and
      `settings.restore_conversation_on_launch` are both true, after
      `AgentStore::from_saved` resolve each agent's most recent session via
      `skwad_history::provider(&agent.agent_type)` +
      `HistoryProvider::sessions(&agent.folder)` (or the cache, if already
      warm) and apply it using the method from 2.1. Verify by unit/integration
      test asserting an agent whose folder has a matching prior `claude`
      session gets `resume_session_id` populated after `build_agent_store`.
- [ ] 3.2 Verify the setting being off, or `restore_layout_on_launch` being
      off, performs no history lookups and leaves `resume_session_id: None`
      for all agents (test asserts no behavior change from current
      `build_agent_store` output in that case).
- [ ] 3.3 Verify manual "Restart" (existing `AgentStore::restart` /
      equivalent path) still clears `resume_session_id` unconditionally,
      regardless of the new setting — add/confirm a regression test.

## 4. Settings UI

- [ ] 4.1 Add a toggle for "Restore conversation on launch" next to the
      existing "Restore layout on launch" control in the settings view, wired
      to the new scalar. Verify by running the app and confirming the toggle
      persists across restart (manual check, noted in the PR description).

## 5. Final verification

- [ ] 5.1 Run `make rust` (fmt + clippy + test + build) and confirm it passes
      clean across the workspace.
