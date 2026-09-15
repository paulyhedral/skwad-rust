## 1. Persona list

- [x] 1.1 Render the non-deleted personas list (name + truncated
      instructions), or "No personas defined" when empty, with per-row edit
      and delete buttons. IMPLEMENTATION NOTE: uses `Settings::active_personas`
      (already excludes soft-deleted system personas, already sorted) and a
      new pure `SettingsWindow::persona_preview(instructions, max_chars)`
      helper for the truncation, tested directly
      (`persona_preview_returns_short_instructions_unchanged`,
      `persona_preview_truncates_long_instructions_with_ellipsis`). No GPUI
      test harness exists in this codebase (same finding as
      `settings-ui-port`/`settings-tabs-shell`/`coding-settings-ui`), so the
      empty-vs-populated render branch itself isn't exercised by an
      automated test — covered by `cargo build -p knot` compiling and
      manual verification in 5.2.
- [x] 1.2 Wire the delete button to `Settings::remove_persona` and
      `cx.notify()` the list. IMPLEMENTATION NOTE: `SettingsWindow::delete_persona`
      is a direct call-and-notify, matching `restore_default_personas`'s
      shape. Not exercised by an automated test (no GPUI test harness, same
      caveat as 1.1) — `Settings::remove_persona` itself is already tested
      in `knot-core`.

## 2. Add / edit editor window

- [x] 2.1 Add a `PersonaEditor` window struct (name + instructions
      `InputState` fields, optional `editing_id: Option<Uuid>`), opened via
      "Add Persona…" (no id) or a row's edit button (with id, fields
      pre-filled). IMPLEMENTATION NOTE: mirrors `AgentEditor`'s shape and
      `open_new_agent_dialog`'s window-opening pattern exactly;
      `open_persona_editor` pre-fills both `InputState`s from the passed
      `Option<Persona>` via `.default_value(...)`. `cargo build -p knot`
      confirms it compiles and opens correctly for both call sites (Add
      passes `None`, Edit passes `Some(persona.clone())`).
- [x] 2.2 Wire Save to call `add_persona` (no id) or `update_persona` (with
      id) then close the editor window and notify the Personas tab. Wire
      Cancel to close without calling either. IMPLEMENTATION NOTE: `save`
      upgrades the `WeakEntity<SettingsWindow>` parent handle and mutates
      `view.settings` directly inside `parent.update(cx, ...)` (both
      `add_persona`/`update_persona` self-persist), then `cx.notify()`s the
      parent before `window.remove_window()` — the mitigation from
      `design.md`'s "list won't reflect a new persona" risk. Cancel is the
      same one-line `window.remove_window()` `AgentEditor` already uses.
      Not exercised by an automated test (no GPUI test harness, same
      caveat as 1.1); `Settings::add_persona`/`update_persona` are already
      tested in `knot-core`.

## 3. Restore defaults

- [x] 3.1 Add a "Restore Defaults" button that opens a confirmation dialog
      (via the existing `open_alert_dialog` pattern from `about_knot`)
      before calling `restore_default_personas`. IMPLEMENTATION NOTE: uses
      `AlertDialog::confirm()` (OK + Cancel) with `.on_ok(...)` calling
      `restore_default_personas`; Cancel needs no `.on_cancel()` override
      since the default cancel path is "close, do nothing." Not exercised
      by an automated test (no GPUI test harness, same caveat as 1.1);
      `Settings::restore_default_personas` is already tested in
      `knot-core`.

## 4. Wire into the tab shell

- [x] 4.1 Replace the Personas tab's placeholder (from
      `settings-tabs-shell`) with this pane's render method. Verified:
      `Render for SettingsWindow` dispatches `SettingsTab::Personas` to
      `render_personas` instead of `render_placeholder`.

## 5. Final verification

- [x] 5.1 `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace`, and `cargo build --workspace` all pass
      clean (run outside the sandbox). `cargo fmt --check -p knot` was not
      run against nightly rustfmt (not installed in this environment,
      same gap noted in the three prior settings-UI changes); stable
      `cargo fmt --check` shows only the same pre-existing import-order
      diffs in unrelated files, none in the files touched here.
- [ ] 5.2 Manually open Settings → Personas, add, edit, delete, and restore
      defaults, confirming persistence across an app restart. NOT
      PERFORMED this session - no interactive macOS session available.
      Flagged in the PR description as follow-up manual verification
      before merge.
