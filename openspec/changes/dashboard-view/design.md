## Context

The Swift reference uses `DashboardView` both as an overlay inside the main
window (global, all attached workspaces) and inside `DetachedWorkspaceView`
(scoped to one workspace, `workspaceId` non-nil) - the workspace-scoped case
toggles in place against the terminal content (`agentManager.showDashboard`),
not a separate window. The Rust port's structural equivalent of a detached
workspace is `WorkspaceWindow`.

## Goals / Non-Goals

**Goals:**
- Workspace-scoped dashboard is an in-place view inside `WorkspaceWindow`,
  toggled against the terminal view the same way the Swift reference does -
  not a separate window.
- Global ("Command Center") dashboard remains its own window.
- One shared render function for the agent-card grid, used by both entry
  points, like the Swift `workspaceId: UUID?` parameter selects scope
  without duplicating the view.
- Reuse `AgentEditor` for the add-agent flow; reuse `knot-git`'s existing
  numstat parsing for diff stats; reuse `Workspace.color_hex` for the
  workspace color bar; reuse `state_color`/`state_label` for status.

**Non-Goals:**
- Inline quick-prompt sending (see proposal.md).
- Auto-ticking relative timestamps.
- Drag-to-reorder manual sort.

## Decisions

- **Workspace-scoped dashboard is an in-place view swap inside
  `WorkspaceWindow`, not a window.** `WorkspaceWindow` gains a view-mode
  field (e.g. `WorkspaceViewMode::{Terminal, Dashboard}`); the "Dashboard"
  button (already landed, currently inert) toggles it and `Render` branches
  on it, matching the Swift reference's in-place swap exactly instead of
  the port's usual "every secondary surface is its own window" pattern -
  this one is a genuine peer view of the terminal content, not a dialog.
- **Global ("Command Center") dashboard stays its own window.** Unlike the
  workspace-scoped case, there is no existing peer view inside `Shell` to
  swap against (the main window's body is the agent list, not a
  toggleable single-workspace terminal), so a window remains the closest
  fit without inventing broader view-switching infrastructure for `Shell`.
- **Git diff stats computed on open + manual refresh, not polled.** No
  polling/refresh-interval precedent exists elsewhere in this codebase for
  filesystem-derived data (contrast with the activity/hook-driven state,
  which is push-based). Start with compute-on-open (and on toggling into
  the view, for the workspace-scoped case); add polling only if a session
  reports this is unpleasant to use.
- **Reuse `AgentEditor` unmodified for "Add Agent".** The Swift reference's
  `AddAgentCardView` opens the same `AgentSheet` used elsewhere; no new
  dialog needed.

## Risks / Trade-offs

- [Risk] `WorkspaceWindow` gaining a view-mode field is a bigger change to
  that struct than this port's usual additive-window pattern →
  Mitigation: it's the correct shape here since the Swift reference treats
  dashboard/terminal as peer views of the same window, not a dialog; keep
  the mode field and its branch narrowly scoped to this toggle.
- [Risk] Git diff stats require shelling out to `git diff --numstat` per
  visible agent folder on open, which is synchronous work today in
  `knot-git` → Mitigation: `AgentEditor`'s folder validation already does
  a blocking filesystem check on the UI thread in this codebase's existing
  style; keep consistent, revisit with `spawn_blocking` if it's slow with
  many agents.

## Migration Plan

Additive only - new view mode and a new window, no existing data or
default-visible view is changed.
