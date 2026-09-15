## 1. Source folder section

- [ ] 1.1 Render current `source_base_folder` (or "Not configured"), a
      "Choose…" button using `PathPromptOptions` to open a directory
      picker, and a clear button, each persisting on change. Verify with
      tests asserting: choosing a directory sets and saves
      `source_base_folder`; clearing sets it to an empty string and saves.

## 2. Agent options section

- [ ] 2.1 Add a `selected_agent_type: String` field (default `"claude"`) to
      `SettingsWindow` and render the existing agent-type dropdown pattern
      to choose it. Verify with a test that selecting a type updates
      `selected_agent_type`.
- [ ] 2.2 Render an options text field bound to
      `agent_options[selected_agent_type]` (empty when absent), persisting
      on edit. Verify with tests: editing the field for the selected type
      saves that type's entry; switching to a type with no stored entry
      shows an empty field rather than another type's value.

## 3. Wire into the tab shell

- [ ] 3.1 Replace the Coding tab's placeholder (from `settings-tabs-shell`)
      with this pane's render method. Verify the Coding tab shows the two
      sections instead of the placeholder text.

## 4. Final verification

- [ ] 4.1 `cargo fmt --check -p knot`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and
      `cargo build --workspace` all pass clean.
- [ ] 4.2 Manually open Settings → Coding, choose/clear a source folder,
      edit options for two different agent types, confirm both persist
      across an app restart. Record whether this manual pass was performed
      (requires an interactive macOS session).
