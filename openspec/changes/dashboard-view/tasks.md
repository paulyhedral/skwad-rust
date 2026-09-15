## 1. Launcher button (this session)

- [x] 1.1 Add an inert "Dashboard" icon button (image + tooltip, no
      on_click behavior) to the workspace window's sidebar, next to the
      existing "New agent" button, per user request to land the
      affordance ahead of the implementation.

## 2. Dashboard window shell

- [ ] 2.1 `DashboardWindow` struct + `Render` impl, opened via
      `cx.open_window` (see design.md - own window, not an in-place view
      swap). Header row: title ("Command Center" for global, workspace
      name for scoped) + `StatusSummaryView`-equivalent counts.
- [ ] 2.2 Sort picker (manual/name/status), matching
      `DashboardSortPicker`'s three modes; manual mode keeps store order
      (no drag-to-reorder in this version - see proposal.md non-goals).

## 3. Agent card grid

- [ ] 3.1 Workspace section: color bar (`Workspace.color_hex`) + name +
      per-workspace status summary + "Add Agent" tile.
- [ ] 3.2 Agent card: avatar, name, status text/color (reuse
      `state_color`/`state_label`), folder last-path-component, git diff
      stats (`knot_git::parse_numstat` on the agent's folder, computed on
      open per design.md).
- [ ] 3.3 Empty state per workspace ("No agents").
- [ ] 3.4 Card click navigates to/focuses the agent (opens or focuses its
      `WorkspaceWindow` and selects it - exact focus-vs-open semantics
      TBD against however window-reuse currently works for
      `WorkspaceWindow::open`).

## 4. Add Agent tile

- [ ] 4.1 Wire the tile's click to `open_new_agent_dialog`
      (`AgentEditor`), prefilling the workspace like the Swift
      reference's `addAgent(to:)` (same folder as an existing agent in
      that workspace, insert-after the last agent).

## 5. Launcher wiring

- [ ] 5.1 Wire the inert button from task 1.1 to open `DashboardWindow`
      scoped to that workspace.
- [ ] 5.2 Decide + implement a global launcher (from `Shell`) once
      `DashboardWindow`'s global (`workspace_id: None`) mode is
      implemented - out of this change's task 1 scope, which only covers
      the workspace-scoped button already visible in the UI.

## 6. Final verification

- [ ] 6.1 `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, `cargo
      build --workspace` all pass clean.
- [ ] 6.2 Manual verification: open the dashboard from a workspace
      window, confirm cards match the agents in that workspace, confirm
      diff stats match `git diff --numstat` run manually against the same
      folder, confirm Add Agent creates an agent in the right workspace.
