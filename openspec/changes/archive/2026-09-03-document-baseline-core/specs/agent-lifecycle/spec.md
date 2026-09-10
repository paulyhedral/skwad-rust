## Purpose

Defines how agents and their shell companions are created, edited, restarted,
resumed, and removed; which agent fields are durable versus runtime-only; and
how an agent is placed within workspaces. This is the contract the Rust port
satisfies; behavior is the intended target, derived from the Swift reference
app with the dual terminal engine removed (Ghostty only).

## ADDED Requirements

### Requirement: Agent creation

The system SHALL create an agent from a working-directory path. Name defaults to
the last path component of the folder. Agent type defaults to `claude`. Each
agent receives a new unique id at creation. A caller MAY override name, avatar,
agent type, shell command, and persona, and MAY request insertion immediately
after a named sibling agent; absent a sibling, the new agent is appended.

#### Scenario: Create from folder with defaults

- **WHEN** an agent is created for `/Users/x/proj` with no other fields
- **THEN** its name is `proj`, its type is `claude`, it has a fresh id
- **AND** it is appended to the agent list and added to a workspace

#### Scenario: Insert after a sibling

- **WHEN** an agent is created with `insert_after = <sibling id>`
- **THEN** it is placed directly after that sibling in both the master agent
  list and the sibling's workspace ordering

### Requirement: Durable versus runtime fields

The system SHALL persist only these agent fields: id, name, avatar, folder,
agent type, created-by, is-companion, shell command, persona id. All other
fields are runtime-only and MUST reset to defaults when agents are loaded:
state (Idle), status text (empty), registered (false), pending-start (false),
terminal title (empty), session id (none), hook metadata (empty), git stats
(none).

#### Scenario: Reload drops runtime state

- **WHEN** an agent that was Working with a session id is persisted and reloaded
- **THEN** the reloaded agent is Idle, unregistered, with no session id and no
  terminal title

#### Scenario: Legacy record without companion fields

- **WHEN** a persisted agent record predates the created-by / is-companion
  fields
- **THEN** it loads with created-by unset and is-companion false, and agent type
  defaults to `claude` if absent

### Requirement: Three distinct status fields

The system SHALL keep three independent per-agent strings: `state` (the
automatic state machine value), `status_text` (set only by the agent via the
`set-status` MCP tool), and `terminal_title` (from terminal escape sequences,
with leading spinner/indicator glyphs stripped). The terminal header SHALL show
`status_text` when non-empty, otherwise `terminal_title`.

#### Scenario: Header prefers agent-set status

- **WHEN** `status_text` is `"Refactoring auth"` and `terminal_title` is `"zsh"`
- **THEN** the header shows `"Refactoring auth"`

### Requirement: Shell companions

The system SHALL support shell companion agents bound to an owner agent via
created-by set to the owner id and is-companion true. A non-companion agent MAY
own companions; a companion MUST NOT own companions. Creating a shell companion
places it after its owner and enters a split layout pairing owner and companion.

#### Scenario: Companion is bound to its owner

- **WHEN** a shell companion is created for owner `A`
- **THEN** the companion's created-by is `A`, is-companion is true, agent type
  is `shell`, and it is inserted after `A`

### Requirement: Agent removal

When an agent is removed, the system SHALL first remove every companion it owns,
then, if the agent is registered with MCP, unregister it, then tear down its
terminal session and any per-agent notification tracking, then remove it from
all workspaces and from the master agent list. Removal SHALL re-select or
collapse split panes so no pane references the removed agent.

#### Scenario: Removing an owner cascades to companions

- **WHEN** owner `A` with companions `B` and `C` is removed
- **THEN** `B` and `C` are removed first, then `A`
- **AND** none remain in any workspace or the master list

#### Scenario: Removing a registered agent unregisters it

- **WHEN** a registered agent is removed
- **THEN** an MCP unregister is issued for its id before teardown

### Requirement: Restart

Restarting an agent SHALL preserve its id and regenerate its restart token,
which forces its terminal session to be destroyed and recreated. Restart SHALL
clear session id, resume-session id, and fork flag; reset state to Idle; set
registered to false; and clear the terminal title.

#### Scenario: Restart keeps identity, drops session

- **WHEN** a registered agent with session id `s1` is restarted
- **THEN** its id is unchanged, its restart token differs, its session id is
  cleared, it is unregistered, and its state is Idle

### Requirement: Resume session

Resuming an agent into a session SHALL set both resume-session id and session id
to the target session, clear the fork flag, and then perform a restart.

#### Scenario: Resume targets a prior session

- **WHEN** an agent is resumed into session `s2`
- **THEN** resume-session id and session id are `s2`, fork is false, and the
  terminal session is recreated

### Requirement: Edit triggers restart only for launch-affecting changes

Editing an agent's name or avatar SHALL NOT restart it. Changing its folder,
agent type, or persona SHALL restart it. When the folder changes and companion
relocation is requested, each companion that shared the old folder SHALL be
moved to the new folder and restarted.

#### Scenario: Rename does not restart

- **WHEN** only the agent's name changes
- **THEN** the terminal session is not recreated

#### Scenario: Folder change restarts and can relocate companions

- **WHEN** the agent's folder changes with relocate-companions requested
- **THEN** the agent restarts, and each companion at the old folder moves to the
  new folder and restarts

### Requirement: Ordering and workspace placement

The system SHALL let an agent be reordered within its workspace and moved to
another workspace. A new agent inherits the workspace of its created-by or
insert-after source when one exists; otherwise it joins the current workspace.
If a workspace has no active agent, a newly added agent becomes its active
agent.

#### Scenario: New agent inherits source workspace

- **WHEN** an agent is created with `insert_after = X` and `X` lives in
  workspace `W`
- **THEN** the new agent is added to `W`, not the current workspace

### Requirement: Bench deployment

Deploying a bench agent SHALL verify the target folder exists and is a
directory. If the check fails, the bench entry SHALL be removed and no agent
created. If it succeeds, an agent is created from the bench entry's folder,
name, avatar, agent type, shell command, and persona.

#### Scenario: Stale bench entry is pruned

- **WHEN** a bench agent whose folder no longer exists is deployed
- **THEN** no agent is created and the bench entry is removed
