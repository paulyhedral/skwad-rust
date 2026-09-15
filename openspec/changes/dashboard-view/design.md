## Context

The Swift reference uses `DashboardView` both as an overlay inside the main
window (global, all attached workspaces) and inside `DetachedWorkspaceView`
(scoped to one workspace, `workspaceId` non-nil). The Rust port's structural
equivalent of a detached workspace is `WorkspaceWindow`; there is no Rust
equivalent of the main window's overlay/tab switching yet (`Shell` renders
the agent list directly, not a dashboard-vs-terminal toggle).

## Goals / Non-Goals

**Goals:**
- One dashboard implementation usable both globally (from `Shell`) and
  scoped to a single workspace (from `WorkspaceWindow`), like the Swift
  `workspaceId: UUID?` parameter.
- Reuse `AgentEditor` for the add-agent flow; reuse `knot-git`'s existing
  numstat parsing for diff stats; reuse `Workspace.color_hex` for the
  workspace color bar; reuse `state_color`/`state_label` for status.

**Non-Goals:**
- Inline quick-prompt sending (see proposal.md).
- Auto-ticking relative timestamps.
- Drag-to-reorder manual sort.

## Decisions

- **Open as its own window (`DashboardWindow`), not an in-place view swap
  inside `Shell`/`WorkspaceWindow`.** The Rust port's `Shell` and
  `WorkspaceWindow` don't currently have a concept of swapping their body
  between "terminal" and "dashboard" modes (no state field, no toggle) -
  every other secondary view in this port (Settings, persona editor, agent
  editor, workspace manager) is its own `cx.open_window`. A window keeps
  this change additive (no `Shell`/`WorkspaceWindow` render-mode branching)
  at the cost of not matching the Swift reference's in-place overlay exactly.
  Revisit if a future change adds general view-switching to those two.
- **Git diff stats computed on open + manual refresh, not polled.** No
  polling/refresh-interval precedent exists elsewhere in this codebase for
  filesystem-derived data (contrast with the activity/hook-driven state,
  which is push-based). Start with compute-on-open; add polling only if a
  session reports this is unpleasant to use.
- **Reuse `AgentEditor` unmodified for "Add Agent".** The Swift reference's
  `AddAgentCardView` opens the same `AgentSheet` used elsewhere; no new
  dialog needed.

## Risks / Trade-offs

- [Risk] A separate window (vs. Swift's in-place overlay) means the
  dashboard and the workspace's terminal can't be visible at once in the
  same window → Mitigation: matches this port's existing pattern for every
  other secondary view; acceptable given no in-place view-switching
  infrastructure exists yet to build on.
- [Risk] Git diff stats require shelling out to `git diff --numstat` per
  visible agent folder on open, which is synchronous work today in
  `knot-git` → Mitigation: `AgentEditor`'s folder validation already does
  a blocking filesystem check on the UI thread in this codebase's existing
  style; keep consistent, revisit with `spawn_blocking` if it's slow with
  many agents.

## Migration Plan

Additive only - new window, no existing data or view is changed.
