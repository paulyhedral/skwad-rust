## 1. Tab state

- [x] 1.1 Add a `SettingsTab` enum (General, Coding, Personas, Autopilot,
      Voice, Mcp, Terminal) and a `selected_tab: SettingsTab` field
      (default `General`) on `SettingsWindow`. Verified: `cargo build -p
      knot` compiles; `settings_tab_default_is_general` asserts
      `SettingsTab::ALL[0] == SettingsTab::General` and
      `open_settings_window` constructs `SettingsWindow` with
      `selected_tab: SettingsTab::General`.

## 2. Tab strip and pane routing

- [x] 2.1 Render a tab strip row (`render_tab_strip`) with one button per
      `SettingsTab::ALL` variant, highlighting the selected tab via
      `Button::selected` and updating `selected_tab` + `cx.notify()` on
      click. IMPLEMENTATION NOTE: no GPUI test harness exists anywhere in
      this codebase (same finding as `settings-ui-port` task 1.1), so the
      click handler itself isn't exercised by an automated test; covered
      instead by `settings_tab_labels_are_distinct` and
      `settings_tab_covers_every_swift_pane`, which lock down the tab set
      and label text the strip renders from, plus manual verification in
      3.2.
- [x] 2.2 Moved General's existing render body into `render_general`
      (content unchanged, only the outer `.size_full().p_5()` moved to the
      new outer wrapper since the tab strip now shares that space), and
      added `render_placeholder(name, cx)` used by the other six tabs, then
      dispatch on `selected_tab` in `render`. Verified: `cargo build -p
      knot` compiles; `cargo test -p knot` passes (37 tests, including all
      prior General-pane tests unchanged).

## 3. Final verification

- [x] 3.1 `cargo fmt --check -p knot`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo
      build --workspace` all pass clean (run outside the sandbox; the
      sandbox blocks MCP HTTP bind and shows one pre-existing,
      unrelated `knot-discovery` timing-test failure, same as noted in
      `settings-ui-port`).
- [ ] 3.2 Manually open Settings, click through all seven tabs, confirm
      General still behaves exactly as before and the other six show their
      placeholder. NOT PERFORMED this session - no interactive macOS
      session available. Flagged in the PR description as follow-up manual
      verification before merge.
