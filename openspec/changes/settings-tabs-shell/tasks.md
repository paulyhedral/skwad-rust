## 1. Tab state

- [ ] 1.1 Add a `SettingsTab` enum (General, Coding, Personas, Autopilot,
      Voice, Mcp, Terminal) and a `selected_tab: SettingsTab` field
      (default `General`) on `SettingsWindow`. Verify it compiles and
      `SettingsWindow::default_tab` (or equivalent) returns `General`.

## 2. Tab strip and pane routing

- [ ] 2.1 Render a tab strip row above the pane body with one button per
      `SettingsTab` variant, highlighting the selected tab and updating
      `selected_tab` on click. Verify with a test that clicking a tab
      button updates `selected_tab` to the matching variant.
- [ ] 2.2 Move General's existing render body into `render_general`
      (unchanged), and add a `render_placeholder(name: &str)` used by the
      other six tabs, then dispatch on `selected_tab` in `render`. Verify
      `cargo build -p knot` compiles and each tab shows either General's
      content or the correct placeholder text.

## 3. Final verification

- [ ] 3.1 `cargo fmt --check -p knot`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and
      `cargo build --workspace` all pass clean.
- [ ] 3.2 Manually open Settings, click through all seven tabs, confirm
      General still behaves exactly as before and the other six show their
      placeholder. Record whether this manual pass was performed (requires
      an interactive macOS session).
