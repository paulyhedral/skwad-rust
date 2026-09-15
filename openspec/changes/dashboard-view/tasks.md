## 1. Launcher button (this session)

- [x] 1.1 Add an inert "Dashboard" icon button (image + tooltip, no
      on_click behavior) to the workspace window's sidebar, next to the
      existing "New agent" button, per user request to land the
      affordance ahead of the implementation.

## 2. Shared agent card grid

- [ ] 2.1 A standalone render function for the workspace-grouped agent
      card grid (color bar + name + per-workspace status summary +
      "Add Agent" tile per workspace section), used by both the
      workspace-scoped in-place view and the global `CommandCenterWindow`.
- [ ] 2.2 Agent card: avatar, name, status text/color (reuse
      `state_color`/`state_label`), folder last-path-component, git diff
      stats (`knot_git::parse_numstat` on the agent's folder, computed on
      open per design.md).
- [ ] 2.3 Empty state per workspace ("No agents").
- [ ] 2.4 Sort picker (manual/name/status), matching
      `DashboardSortPicker`'s three modes; manual mode keeps store order
      (no drag-to-reorder in this version - see proposal.md non-goals).

## 3. Workspace-scoped dashboard (in-place view)

- [ ] 3.1 Add a view-mode field to `WorkspaceWindow`
      (`WorkspaceViewMode::{Terminal, Dashboard}`) and branch `Render` on
      it - the dashboard is a peer view of the terminal content, not a
      dialog or separate window (per design.md).
- [ ] 3.2 Wire the already-landed "Dashboard" button to toggle the mode;
      when in dashboard mode, render the shared grid (task 2) scoped to
      this workspace's agents.
- [ ] 3.3 Card click switches back to terminal mode with that agent
      selected.
- [ ] 3.4 Add Agent tile wired to `open_new_agent_dialog` (`AgentEditor`),
      prefilling the workspace like the Swift reference's `addAgent(to:)`
      (same folder as an existing agent in that workspace, insert-after
      the last agent).

## 4. Command Center (global window)

- [ ] 4.1 `CommandCenterWindow` struct + `Render` impl, opened via
      `cx.open_window`, reusing the shared grid (task 2) across all
      attached workspaces. Header: title "Command Center" + overall
      status summary.
- [ ] 4.2 Card click opens or focuses that agent's `WorkspaceWindow` and
      selects it (exact focus-vs-open semantics TBD against however
      window-reuse currently works for `WorkspaceWindow::open`).
- [ ] 4.3 Add Agent tile wired the same way as task 3.4.
- [ ] 4.4 Decide + implement a launcher for this window (from `Shell` or
      elsewhere) - not yet placed anywhere in the UI.

## 5. Final verification

- [ ] 5.1 `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`, `cargo
      build --workspace` all pass clean.
- [ ] 5.2 Manual verification: toggle the dashboard in a workspace window,
      confirm cards match that workspace's agents and toggling back to
      terminal mode preserves the previously selected agent; confirm the
      Command Center window shows all attached workspaces; confirm diff
      stats match `git diff --numstat` run manually against the same
      folder; confirm Add Agent creates an agent in the right workspace
      from both entry points.
