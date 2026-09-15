## 1. Settings window scaffold

- [ ] 1.1 Add a "Settings…" app-menu item bound to `Cmd+,` in
      `crates/knot/src/main.rs`, dispatching an action that opens (or
      focuses, if already open) a new secondary GPUI window. Verify with a
      unit/integration test asserting the action opens exactly one window
      and a second invocation does not open a duplicate.
- [ ] 1.2 Create the settings view module (e.g.
      `crates/knot/src/settings_window.rs`) holding a handle to the shared
      `Settings` and rendering an empty window shell. Verify it compiles
      and the window opens showing a blank pane.

## 2. Appearance section

- [ ] 2.1 Render an "Appearance" section with a `Select` bound to
      `appearance_mode` (Auto/System/Light/Dark). Verify a test that
      selecting an option calls `Settings::save` with the new value.

## 3. Startup section

- [ ] 3.1 Render "Restore agents on launch" (`Switch` bound to
      `restore_layout_on_launch`) and "Keep running in menu bar when
      closed" (`Switch` bound to `keep_in_menu_bar`), each persisting on
      toggle. Verify with tests asserting each toggle's `on_click` mutates
      and saves the correct field.
- [ ] 3.2 Render "Restore last conversation" (`Switch` bound to
      `restore_conversation_on_launch`), enabled only when
      `restore_layout_on_launch` is true; disabled (not hidden) otherwise,
      still reflecting its stored value. Verify with tests covering:
      enabled-and-toggleable when layout-restore is on; disabled and
      inert when layout-restore is off; turning layout-restore off does
      not change `restore_conversation_on_launch`'s stored value.

## 4. Notifications section

- [ ] 4.1 Render "Desktop notifications" (`Switch` bound to
      `desktop_notifications_enabled`), persisting on toggle. Verify with
      a test asserting toggle mutates and saves the field.

## 5. Final verification

- [ ] 5.1 `cargo fmt --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `cargo test --workspace`, and `cargo build --workspace`
      all pass clean.
- [ ] 5.2 Manually launch the app, open Settings via the menu and via
      `Cmd+,`, confirm all four controls reflect and persist their current
      values across an app restart, and confirm the dependent-toggle
      disable behavior from 3.2 is visible. Record in the PR description
      whether this manual pass was actually performed (an interactive
      macOS session is required).
