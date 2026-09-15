## 1. Font section

- [ ] 1.1 Render a font-name picker (dropdown-menu pattern, static
      shortlist matching the Swift reference's `monospaceFonts`) bound to
      `terminal_font_name`, persisting on selection. Verify with a test
      that selecting a font saves `terminal_font_name`.
- [ ] 1.2 Render a numeric size field bound to `terminal_font_size`,
      persisting on change. Verify with a test that editing the field
      saves `terminal_font_size`.

## 2. Wire into the tab shell

- [ ] 2.1 Replace the Terminal tab's placeholder (from
      `settings-tabs-shell`) with this pane's render method. Verify the
      Terminal tab shows the Font section instead of the placeholder text.

## 3. Final verification

- [ ] 3.1 `cargo fmt --check -p knot`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and
      `cargo build --workspace` all pass clean.
- [ ] 3.2 Manually open Settings → Terminal, change font and size, confirm
      both persist across an app restart. Record whether this manual pass
      was performed (requires an interactive macOS session).
