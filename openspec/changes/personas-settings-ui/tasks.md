## 1. Persona list

- [ ] 1.1 Render the non-deleted personas list (name + truncated
      instructions), or "No personas defined" when empty, with per-row edit
      and delete buttons. Verify with a test that the empty-list message
      appears when `settings.personas` has no active entries.
- [ ] 1.2 Wire the delete button to `Settings::remove_persona` and
      `cx.notify()` the list. Verify with a test that clicking delete on a
      persona removes it from the rendered list.

## 2. Add / edit editor window

- [ ] 2.1 Add a `PersonaEditor` window struct (name + instructions
      `InputState` fields, optional `editing_id: Option<Uuid>`), opened via
      "Add Persona…" (no id) or a row's edit button (with id, fields
      pre-filled). Verify it compiles and opens pre-filled for edit, empty
      for add.
- [ ] 2.2 Wire Save to call `add_persona` (no id) or `update_persona` (with
      id) then close the editor window and notify the Personas tab. Wire
      Cancel to close without calling either. Verify with tests: Save-with-
      no-id calls `add_persona`; Save-with-id calls `update_persona` for
      that id; Cancel mutates nothing.

## 3. Restore defaults

- [ ] 3.1 Add a "Restore Defaults" button that opens a confirmation dialog
      (via the existing `open_alert_dialog` pattern from `about_knot`)
      before calling `restore_default_personas`. Verify with a test that
      the confirmed path calls `restore_default_personas` and the
      canceled path does not.

## 4. Wire into the tab shell

- [ ] 4.1 Replace the Personas tab's placeholder (from
      `settings-tabs-shell`) with this pane's render method. Verify the
      Personas tab shows the list instead of the placeholder text.

## 5. Final verification

- [ ] 5.1 `cargo fmt --check -p knot`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and
      `cargo build --workspace` all pass clean.
- [ ] 5.2 Manually open Settings → Personas, add, edit, delete, and restore
      defaults, confirming persistence across an app restart. Record
      whether this manual pass was performed (requires an interactive
      macOS session).
