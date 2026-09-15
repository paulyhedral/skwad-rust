## Why

`knot_core::Settings` already has full scalar coverage (appearance mode,
restore-layout-on-launch, restore-conversation-on-launch, keep-in-menu-bar,
desktop notifications) but the Rust `knot` binary exposes none of it in the
UI — every value is stuck at its default with no way for a user to change
it. This also left two prior changes with a permanently-skipped task
(`desktop-notifications-port` and `restore-conversation-on-launch` each
deferred their settings-toggle task with "no settings screen exists yet").

## What Changes

- Add a General settings window to the `knot` GPUI app, opened via the
  standard app-menu "Settings…" item (`Cmd+,`).
- Cover the same three sections as the Swift reference's General pane,
  scoped to what the Rust port already models in `knot_core::Settings`:
  - **Appearance**: appearance-mode picker (Auto/System/Light/Dark).
  - **Startup**: "Restore agents on launch" toggle
    (`restore_layout_on_launch`), "Restore last conversation" toggle
    (`restore_conversation_on_launch`, disabled/hidden unless restore-layout
    is on, since it has no effect otherwise), "Keep running in menu bar
    when closed" toggle (`keep_in_menu_bar`).
  - **Notifications**: "Desktop notifications" toggle
    (`desktop_notifications_enabled`).
- Toggling a control writes through to `Settings` and persists immediately
  (existing `Settings::save` path), matching the "writing a value SHALL
  persist it immediately" contract already specified.
- **Non-goals**: the Coding / Personas / Autopilot / Voice / MCP / Terminal
  panes, the tab strip chrome between them, and "Automatically check for
  updates" (no updater exists in the Rust port yet). Those are separate,
  later changes once this window's structure exists.

## Capabilities

### New Capabilities

- `settings-ui`: a General settings window in the `knot` GPUI app backed by
  `knot_core::Settings`, covering appearance mode, startup toggles, and the
  desktop-notifications toggle.

### Modified Capabilities

(none — this only adds a UI surface over existing, already-specified
`settings-persistence` scalars; no persistence behavior changes)

## Impact

- `crates/knot/src/main.rs`: add a settings window/action, likely factored
  into a new `crates/knot/src/settings_window.rs` (or similar) module.
- `gpui-kit`: use its existing toggle/picker/section primitives; no new
  dependency.
- No changes to `knot-core`, `knot-agents`, or any other crate's public API.
