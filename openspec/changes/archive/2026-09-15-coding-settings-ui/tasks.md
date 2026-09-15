## 1. Source folder section

- [x] 1.1 Render current `source_base_folder` (or "Not configured"), a
      "Choose…" button using `PathPromptOptions` to open a directory
      picker, and a clear button, each persisting on change.
      IMPLEMENTATION NOTE: `choose_source_folder` mirrors `AgentEditor`'s
      existing `choose_folder` (same `cx.prompt_for_paths` +
      `cx.spawn`/`cx.update` shape); `clear_source_folder` is a direct
      field write + `persist()`, matching every other settings mutation in
      this file. No GPUI test harness exists in this codebase (same
      finding as `settings-ui-port`/`settings-tabs-shell`), so the
      click-driven "sets and saves" behavior itself isn't exercised by an
      automated test — covered instead by `cargo build -p knot` compiling
      and manual verification in 4.2.

## 2. Agent options section

- [x] 2.1 Add a `selected_agent_type: String` field (default `"claude"`) to
      `SettingsWindow` and render the existing agent-type dropdown pattern
      to choose it. IMPLEMENTATION NOTE: `select_agent_type` updates the
      field and re-seeds `agent_options_input`'s text from the newly
      selected type's stored value (or empty). Verified with
      `agent_type_label_maps_known_types` /
      `agent_type_label_defaults_to_claude` on the label-mapping logic; the
      dropdown's `on_click` itself has no GPUI test harness to exercise it
      against (same caveat as 1.1).
- [x] 2.2 Render an options text field bound to
      `agent_options[selected_agent_type]` (empty when absent), persisting
      on edit. IMPLEMENTATION NOTE: subscribes to the input's
      `InputEvent::Change` (the same pattern `gpui-component`'s own
      `SearchState` uses internally) and calls `save_agent_options`, which
      inserts into `settings.agent_options` keyed by
      `selected_agent_type` and persists immediately. Switching type calls
      `set_value` with the newly selected type's stored value (or empty
      string when absent), so no type's edits leak into another's field.
      Not exercised by an automated test (no GPUI test harness, same
      caveat as 1.1); `cargo build -p knot` and `cargo test -p knot`
      confirm the surrounding logic compiles and existing tests still
      pass.

## 3. Wire into the tab shell

- [x] 3.1 Replace the Coding tab's placeholder (from `settings-tabs-shell`)
      with this pane's render method. Verified: `Render for SettingsWindow`
      dispatches `SettingsTab::Coding` to `render_coding` instead of
      `render_placeholder`.

## 4. Final verification

- [x] 4.1 `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace`, and `cargo build --workspace` all pass
      clean (run outside the sandbox). `cargo fmt --check -p knot` was not
      run against nightly rustfmt (not installed in this environment,
      same gap noted in `settings-ui-port`/`settings-tabs-shell`); stable
      `cargo fmt --check` shows only the same pre-existing import-order
      diffs in unrelated files noted in those changes, none in the files
      touched here.
- [ ] 4.2 Manually open Settings → Coding, choose/clear a source folder,
      edit options for two different agent types, confirm both persist
      across an app restart. NOT PERFORMED this session - no interactive
      macOS session available. Flagged in the PR description as follow-up
      manual verification before merge.
