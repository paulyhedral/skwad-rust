## 1. Settings window scaffold

- [x] 1.1 Add a "Settings…" app-menu item bound to `Cmd+,` in
      `crates/knot/src/main.rs`, dispatching an action that opens (or
      focuses, if already open) a new secondary GPUI window. IMPLEMENTATION
      NOTE: kept in `main.rs` alongside the other window-open functions
      (`WorkspaceWindow::open`, `AgentEditor`'s opener) rather than a new
      module, matching this file's existing one-window-per-function
      convention; a separate `settings_window.rs` would be the first module
      split in this binary and isn't justified by one view. Dedup uses
      `AnyWindowHandle::update(..).is_ok()` to detect a still-open window
      (the standard GPUI idiom - an `Err` means the window closed) and
      reuses it via `window.activate_window()`; this is the same
      "try the handle, fall back to opening" pattern as the codebase's
      existing single-window architecture. No GPUI test harness exists
      anywhere in this codebase (grepped for `TestAppContext`/`gpui::test`
      - none), so exercising the actual open/dedup path needs the manual
      pass in 5.2, same caveat already accepted for notification delivery
      in `desktop-notifications-port`.
- [x] 1.2 Create the settings view (`SettingsWindow` struct + `Render` impl
      in `crates/knot/src/main.rs`, next to `WorkspaceWindow`/`AgentEditor`)
      holding an owned `Settings` clone and rendering the pane. Verified:
      `cargo build -p knot` compiles and `open_settings_window` opens it
      wrapped in `Root`, matching every other window in this file.

## 2. Appearance section

- [x] 2.1 Render an "Appearance" section with a picker bound to
      `appearance_mode` (Auto/System/Light/Dark). IMPLEMENTATION NOTE: used
      the existing `Button::dropdown_menu` picker pattern (already used
      throughout this file for avatar/agent-type/persona) instead of
      `gpui-component`'s `Select`, to keep one picker idiom in the file -
      see `design.md` Decisions. Verified with unit tests
      `appearance_label_maps_known_modes` /
      `appearance_label_defaults_to_auto` on the label-mapping logic; the
      picker's `on_click` itself is a direct one-line field assignment +
      persist with nothing further to unit test (same shape as the
      existing avatar/agent-type pickers, which have no click-handler
      tests either).

## 3. Startup section

- [x] 3.1 Render "Restore agents on launch" (`Switch` bound to
      `restore_layout_on_launch`) and "Keep running in menu bar when
      closed" (`Switch` bound to `keep_in_menu_bar`), each persisting on
      toggle. Verified: `cargo build -p knot` compiles; each `on_click`
      is a direct field assignment + `Settings::persist` call, mirroring
      the mutation pattern used everywhere else in this file (e.g.
      `create_agent`).
- [x] 3.2 Render "Restore last conversation" (`Switch` bound to
      `restore_conversation_on_launch`), gated on
      `restore_conversation_toggle_enabled` (extracted as a pure function
      so the enablement rule itself is unit-tested, matching the
      `should_notify`/`notification_body` extraction pattern from
      `desktop-notifications-port`) - disabled, not hidden, when
      `restore_layout_on_launch` is false. Verified with
      `restore_conversation_toggle_enabled_only_with_layout_restore`
      (enabled iff layout-restore is on) and
      `turning_off_layout_restore_does_not_touch_conversation_restore`
      (the two settings fields are independent, so toggling one never
      touches the other's stored value - inherent to them being separate
      struct fields, confirmed directly on `Settings`).

## 4. Notifications section

- [x] 4.1 Render "Desktop notifications" (`Switch` bound to
      `desktop_notifications_enabled`), persisting on toggle. Verified:
      `cargo build -p knot` compiles; same direct-mutation pattern as 3.1.

## 5. Final verification

- [x] 5.1 `cargo fmt --check -p knot`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and
      `cargo build --workspace` all pass clean (run outside the sandbox;
      inside it, `knot-mcp`'s HTTP tests fail on `bind` and
      `knot-discovery`'s watcher test fails on timing - both pre-existing
      sandbox artifacts unrelated to this change, confirmed unaffected by
      diffing this change against `crates/knot` only). Two pre-existing
      workspace files (`knot-agent-launch/src/lib.rs`,
      `knot-mcp/src/status.rs`, `knot-mcp/tests/http.rs`,
      `knot-mcp-tools/src/{agents,args,messaging}.rs`) show stable-vs-
      nightly import-order diffs under plain `cargo fmt --check`; nightly
      `rustfmt` isn't installed in this environment to confirm they're
      already clean under the project's mandated toolchain, so left
      untouched rather than "fixed" against the wrong formatter.
- [ ] 5.2 Manually launch the app, open Settings via the menu and via
      `Cmd+,`, confirm all four controls reflect and persist their current
      values across an app restart, and confirm the dependent-toggle
      disable behavior from 3.2 is visible. NOT PERFORMED this session -
      no interactive macOS session available. Flagged in the PR
      description as follow-up manual verification before merge.
