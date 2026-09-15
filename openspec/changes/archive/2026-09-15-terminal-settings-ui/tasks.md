## 1. Font section

- [x] 1.1 Render a font-name picker (dropdown-menu pattern, static
      shortlist matching the Swift reference's `monospaceFonts`) bound to
      `terminal_font_name`, persisting on selection. IMPLEMENTATION NOTE:
      ported the list verbatim as `SettingsWindow::TERMINAL_FONTS`, without
      the Swift reference's `NSFontManager`-based availability filter (no
      font-enumeration API is surfaced through `gpui-kit`) — matches
      design.md's explicit decision to show the full static list. Verified
      with `terminal_fonts_matches_swift_reference_monospace_list`.
- [x] 1.2 Render a numeric size field bound to `terminal_font_size`,
      persisting on change. IMPLEMENTATION NOTE: a plain `Input`/
      `InputState` (formatted from the `f64` as a string) subscribed to
      `InputEvent::Change`, same pattern as the MCP tab's port field;
      `save_terminal_font_size` only writes/persists when the current text
      parses as a valid `f64`. No slider widget exists anywhere in this
      codebase (checked, per design.md), so a numeric field was used
      instead of porting the Swift reference's `Slider`, matching every
      other scalar field in this window. Not exercised by an automated
      test (no GPUI test harness, same finding as every prior
      settings-UI change); manual verification in 3.2.

## 2. Wire into the tab shell

- [x] 2.1 Replace the Terminal tab's placeholder (from
      `settings-tabs-shell`) with this pane's render method. Verified:
      `Render for SettingsWindow` dispatches `SettingsTab::Terminal` to
      `render_terminal` instead of `render_placeholder`.

## 3. Final verification

- [x] 3.1 `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo
      build --workspace` all pass clean.
- [ ] 3.2 Manually open Settings → Terminal, change font and size, confirm
      both persist across an app restart. NOT PERFORMED this session - no
      interactive macOS session available. Flagged in the PR description
      as follow-up manual verification before merge.
