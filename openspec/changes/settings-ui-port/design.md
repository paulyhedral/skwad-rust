## Context

`crates/knot/src/main.rs` is currently the whole GPUI app: window shell,
agent layout, terminal panes. There is no settings surface, no secondary
window, and no app-menu wiring yet beyond what GPUI's default menu gives
for free. `knot_core::Settings` already owns every scalar this window
needs and calls `save()` synchronously on mutation (see
`settings-persistence` spec), so this change is UI-only — no new
persistence logic.

`gpui-kit` re-exports `gpui-component`'s widget set, which includes
`switch::Switch` (toggle) and `select::Select` (a picker/dropdown), plus
`window_ext` helpers for opening secondary windows. Those cover every
control this pane needs; no new dependency is required.

## Goals / Non-Goals

**Goals:**
- A single new GPUI window, opened from the app menu, rendering the three
  General-pane sections against live `Settings` state.
- Each control's `on_change`/`on_click` handler writes directly to the
  shared `Settings` and calls `save()`, mirroring how `main.rs` already
  mutates and persists settings elsewhere (e.g. layout restore path).

**Non-Goals:**
- Any pane beyond General (tab strip, Coding/Personas/Autopilot/Voice/MCP/
  Terminal) — those are separate future changes once this window's
  skeleton exists.
- Live-reloading the main window's appearance when `appearance_mode`
  changes from this pane — wiring the theme system to react to settings
  changes is a separate concern from exposing the control.

## Decisions

- **One window, one view, three sections** — no internal routing or tab
  state, since only General exists yet. When later panes are added, this
  view becomes the first tab's content; no rework of the persistence
  wiring is needed.
- **`Switch` for booleans, the existing `Button::dropdown_menu` picker
  pattern for appearance mode** — matches the Swift reference 1:1
  (`Toggle` → `Switch`, `Picker` → dropdown). `dropdown_menu` is already
  used throughout `main.rs` for every other picker (avatar, agent type,
  persona); reusing it keeps one picker idiom in the file instead of
  introducing `gpui-component`'s `Select` for a single use.
- **Direct mutation over an event/message bus** — `Settings` is already
  shared as an `Rc`/`Arc`-wrapped model read elsewhere in `main.rs`, and
  every existing settings mutation in the codebase is a direct field-set +
  `save()`. Introducing an intermediate action/event type for this one
  window would be a new pattern this codebase doesn't otherwise use.
- **Disabled, not hidden, dependent toggle** — "Restore last conversation"
  stays visible but disabled when "Restore agents on launch" is off, per
  the spec, so the setting doesn't appear to vanish and its stored value
  stays legible.

## Risks / Trade-offs

- [Risk] `appearance_mode` change has no immediate visual effect since
  theme reactivity isn't wired yet → Mitigation: out of scope per
  Non-Goals; the value still persists correctly and takes effect on next
  launch, matching current behavior for every other GPUI-side setting.
- [Risk] Opening a second GPUI window is new territory for this codebase
  (only one window exists today) → Mitigation: `gpui-kit`'s `window_ext`
  is built for exactly this; keep the window's own state (a small struct
  holding a `Settings` handle) isolated from the main window's model to
  avoid coupling.

## Migration Plan

Additive only — no existing behavior changes, no data migration. Ship
behind no flag; the window simply becomes reachable once merged.
