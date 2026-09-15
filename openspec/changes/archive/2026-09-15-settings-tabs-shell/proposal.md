## Why

`settings-ui-port` deliberately shipped one pane (General) in a window with
no tab strip, since no other pane existed yet. Six more panes are being
proposed (Coding, Personas, Autopilot, Voice, MCP, Terminal) and each needs
a home; without a shared tab shell, every pane change would have to
reinvent its own tab-switching logic. This change builds that shell once.

## What Changes

- Add a tab strip to the settings window (General, Coding, Personas,
  Autopilot, Voice, MCP, Terminal — matching the Swift reference's tab
  order and icons where `gpui-kit` has an equivalent icon), with General
  pre-selected on open.
- Refactor the existing `SettingsWindow` so its General content becomes the
  first tab's body, selected by default; other tabs render a placeholder
  ("Not yet available") until their own change lands.
- Switching tabs SHALL NOT close or reset the window; each pane keeps its
  own in-memory edits (not just persisted values) if the user switches away
  and back before closing the window — relevant once panes with unsaved
  draft state (e.g. Persona sheet) exist.
- **Non-goals**: filling in the six placeholder panes' actual content —
  that is six separate changes. This change only builds the shell and
  wires General into it unchanged.

## Capabilities

### Modified Capabilities

- `settings-ui` (currently pending in `settings-ui-port`, not yet
  archived to `openspec/specs/`): supersedes that change's "Window scope"
  requirement, which explicitly forbade a tab strip. When both changes are
  eventually archived, "Window scope" is reconciled to describe a
  multi-pane window instead of a single-pane one.

## Impact

- `crates/knot/src/main.rs`: `SettingsWindow` gains tab-selection state and
  a tab-strip renderer; its current body becomes the General tab's content.
- No `knot_core::Settings` changes — this is pure window chrome.
