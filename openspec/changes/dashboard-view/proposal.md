## Why

The Swift reference has a Dashboard (`Skwad/Views/Dashboard/`: `DashboardView`,
`DashboardAgentGrid`, `AgentCardView`, `AddAgentCardView`, `GitStatsView`,
`QuickPromptField`, `StatusSummaryView`, `DashboardSortPicker`) that gives an
at-a-glance grid of agent cards grouped by workspace, each showing status,
live git diff stats, and last-activity time, with an inline prompt field and
an "Add Agent" tile. The Rust port has no equivalent view - agents are only
browsable one workspace-window at a time. This is a real porting gap, not a
new feature: the Swift view exists and is in active use.

## What Changes

- Add a `dashboard` capability: a scrollable, workspace-grouped grid of agent
  cards (avatar, name, status text, relative last-activity time, git diff
  stats via the existing `knot-git` numstat parsing), a per-workspace status
  summary (counts by state), a manual/name/status sort picker, and an
  "Add Agent" tile per workspace that opens the existing `AgentEditor`
  dialog. Two entry points, matching the Swift reference's dual use:
  - **Workspace-scoped**: an in-place view inside `WorkspaceWindow`, toggled
    with the agent terminal view the same way the Swift reference swaps
    `DashboardView`/terminal content inside `DetachedWorkspaceView` - not a
    separate window.
  - **Global ("Command Center")**: all attached workspaces, its own window
    (matching the Swift reference's main-window overlay, ported here as a
    window rather than an overlay since `Shell` has no view-switching
    infrastructure to build on - see `design.md`).
- Add an inert "Dashboard" launcher button now (icon + tooltip, no
  wiring) so the affordance exists in the UI ahead of the real
  implementation - this lands in this session, the rest of this proposal's
  scope does not.

**Non-goals for the first version:**
- Inline quick-prompt sending from the card (`QuickPromptField` sends text
  directly into an agent's terminal session from the grid) - needs a
  decision on how dashboard-initiated input reaches a session that may not
  be the focused terminal; deferred until a workspace window's session
  plumbing is reviewed for this.
- Live auto-refresh timer for relative timestamps ("2 minutes ago" ticking
  without user interaction) - cosmetic, easy to add once the static view
  exists.
- Drag-to-reorder for the manual sort mode - the Swift reference supports
  it; this port can ship sortable-but-not-draggable first, matching how
  `settings-ui` shipped without every Swift interaction on day one.

## Capabilities

### New Capabilities

- `dashboard`: workspace-grouped agent card grid, status summary, sort
  picker, add-agent tile, git diff stats display.

### Modified Capabilities

None yet - the inert launcher button in this session touches
`crates/knot/src/main.rs` only and adds no capability surface (it does
nothing when clicked).

## Impact

- New spec under `openspec/specs/dashboard/`.
- `crates/knot/src/main.rs`:
  - `WorkspaceWindow` gains a view-mode field (terminal vs. dashboard) and
    a shared dashboard-grid render function used in-place, reusing
    `AgentEditor` for the add-agent flow and `state_color`/`state_label`
    already added for the workspace agent list row.
  - A new `CommandCenterWindow` (global, all attached workspaces) reuses
    the same dashboard-grid render function.
- `knot-git`: no schema changes: `DiffStats`/`parse_numstat` already exist
  and are reused as-is; this change is the first UI consumer.
- `knot-agents`: `Workspace.color_hex` already exists and is reused as-is
  for the workspace color bar.
