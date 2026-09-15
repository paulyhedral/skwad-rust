## Why

`knot_core::Settings` already persists `terminal_font_name` and
`terminal_font_size`, but nothing exposes them, and the Rust port's
terminal rendering (`knot-terminal`) does not yet read either field — they
are currently dead settings. The Terminal tab (a placeholder from
`settings-tabs-shell`) is where the Swift reference exposes terminal
configuration, though its scope there is split by terminal engine
(Ghostty vs. SwiftTerm); the Rust port keeps Ghostty only.

## What Changes

- Fill in the Terminal tab with a "Font" section: a font-name picker
  (reusing the existing dropdown-menu pattern, offering the same
  monospace-font shortlist as the Swift reference's SwiftTerm branch) and
  a size field, bound to `terminal_font_name` / `terminal_font_size`,
  persisting on change.
- **Non-goals**: an engine picker (Ghostty is the only engine in the Rust
  port — see `CLAUDE.md`'s stack mapping), background/foreground color
  pickers (no matching `knot_core::Settings` fields exist, and the Swift
  reference's Ghostty branch reads colors from the user's Ghostty config
  file rather than app settings, which this port doesn't parse yet), and
  the terminal preview swatch. Wiring `terminal_font_name`/
  `terminal_font_size` into actual Ghostty rendering is separate work in
  `knot-terminal`; this change only exposes the already-persisted values,
  matching the precedent set by `appearance_mode` in `settings-ui-port`
  (persisted and editable before anything reads it for rendering).

## Capabilities

### Modified Capabilities

- `settings-ui` (pending in `settings-ui-port` / `settings-tabs-shell`,
  not yet archived): fills in the Terminal tab's placeholder with real
  requirements.

## Impact

- `crates/knot/src/main.rs`: `SettingsWindow::render_terminal`.
- No `knot_core::Settings` schema changes — both fields already exist.
- No `knot-terminal` changes — rendering these values is a separate,
  later change once this UI exists to edit them.
