## ADDED Requirements

### Requirement: Dashboard launcher

The workspace window SHALL show an icon-only "Dashboard" button (image +
tooltip) next to its "New agent" button.

#### Scenario: Launcher present but inert (this session)

- **WHEN** a workspace window is open
- **THEN** a "Dashboard" icon button is visible next to "New agent"
- **AND** clicking it does nothing yet (wiring lands in a later task)

#### Scenario: Launcher opens the dashboard (future task)

- **WHEN** a workspace window is open and the "Dashboard" button is clicked
- **THEN** a dashboard window scoped to that workspace opens (or focuses,
  if already open)

### Requirement: Agent card grid

The dashboard SHALL show agents grouped by workspace, each in a card
showing its avatar, name, status text and color, folder name, and git
diff stats, with an "Add Agent" tile per workspace group.

#### Scenario: Card reflects agent state

- **WHEN** an agent's automatic state is `Running`
- **THEN** its card shows the orange status indicator and the current
  status/title text

#### Scenario: Card shows git diff stats

- **WHEN** an agent's folder has uncommitted changes
- **THEN** its card shows insertions/deletions/file counts parsed via
  `knot_git::parse_numstat`

#### Scenario: Empty workspace

- **WHEN** a workspace has no non-companion agents
- **THEN** its section shows "No agents" instead of a card grid

### Requirement: Sort modes

The dashboard SHALL support sorting each workspace's agent cards by
manual (store order), name, or status.

#### Scenario: Sort by name

- **WHEN** the user selects "Name" in the sort picker
- **THEN** each workspace's cards are ordered alphabetically by agent name

### Requirement: Add Agent tile

Each workspace section SHALL show an "Add Agent" tile that opens the
existing agent-creation dialog, prefilled for that workspace.

#### Scenario: Add Agent opens prefilled dialog

- **WHEN** the user clicks "Add Agent" in a workspace's section
- **THEN** the agent editor opens with the folder prefilled from an
  existing agent in that workspace, if any
