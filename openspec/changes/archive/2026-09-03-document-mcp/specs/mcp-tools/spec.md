## Purpose

Defines the concrete MCP tool catalog Skwad exposes to agents: each tool's
name, required and optional inputs, success payload, and failure behavior.
Routing rules for messaging tools are owned by `mcp-messaging`; this spec fixes
the tool surface itself.

## ADDED Requirements

### Requirement: Tool catalog

`tools/list` SHALL return exactly these tools: `register-agent`,
`list-agents`, `send-message`, `check-messages`, `broadcast-message`,
`list-repos`, `list-worktrees`, `create-agent`, `close-agent`,
`create-worktree`, `set-status`, `display-markdown`, `view-mermaid`. Every tool
SHALL declare a JSON object input schema with typed properties and a required
list.

#### Scenario: Catalog is complete and stable

- **WHEN** `tools/list` is called
- **THEN** all thirteen tools are present, each with a name, description, and
  object input schema

### Requirement: Missing required argument is a tool error

When a tool call omits a required argument, the handler SHALL return a tool
result with `isError` true and text naming the missing argument. It SHALL NOT
raise a transport-level error.

#### Scenario: send-message without content

- **WHEN** `send-message` is called with `from` and `to` but no `content`
- **THEN** the result has `isError` true and names the missing `content`

### Requirement: register-agent

`register-agent` SHALL require `agentId` and accept optional `sessionId`. On
success it SHALL mark the agent registered, associate the session id when
given, and return the unread-message count and the list of skwad members
visible to the caller.

#### Scenario: Register returns roster

- **WHEN** a known agent calls `register-agent` with its id
- **THEN** it is marked registered and the result includes the current skwad
  members

### Requirement: list-agents

`list-agents` SHALL require the caller's `agentId` and return, for each agent
the caller may see (its own workspace; companions only if owned by the caller),
the agent id, name, folder, status string, and registered flag.

#### Scenario: Caller sees own workspace

- **WHEN** an agent calls `list-agents`
- **THEN** the response lists agents in the caller's workspace and excludes
  companions the caller does not own

### Requirement: send-message, check-messages, broadcast-message

`send-message` SHALL require `from`, `to`, `content`. `check-messages` SHALL
require `agentId` and accept optional `markAsRead` (default true).
`broadcast-message` SHALL require `from`, `content`. Their delivery, routing,
and rejection semantics are defined by `mcp-messaging`; on rejection the tool
result SHALL carry the rejection string with `isError` true.

#### Scenario: Rejected send surfaces the reason

- **WHEN** `send-message` targets a shell agent
- **THEN** the tool result text is "Cannot send messages to shell agents" with
  `isError` true

### Requirement: list-repos and list-worktrees

`list-repos` SHALL take no arguments and return repositories discovered under
the configured source folder, each with its name and worktrees.
`list-worktrees` SHALL require `repoPath` and return that repository's
worktrees as name and absolute path.

#### Scenario: list-worktrees for a repo

- **WHEN** `list-worktrees` is called with a valid `repoPath`
- **THEN** the result lists each worktree's name and path

### Requirement: create-agent

`create-agent` SHALL require the caller's `agentId`. It SHALL accept
`benchAgentId` (deploy a bench template; then name/agentType/repoPath are
optional), or otherwise require `name`, `agentType`, and `repoPath`. Optional
inputs: `icon`, `createWorktree` with `branchName`, `companion`, `command`
(shell only), `personaId`. On success it SHALL create the agent attributed to
the caller and return the new agent id.

#### Scenario: Create from explicit fields

- **WHEN** `create-agent` is called with `name`, `agentType`, `repoPath`
- **THEN** an agent is created with `created_by` set to the caller and the new
  id is returned

#### Scenario: Create with a new worktree

- **WHEN** `create-agent` sets `createWorktree` true without `branchName`
- **THEN** the result is an error naming the missing `branchName`

### Requirement: close-agent

`close-agent` SHALL require the caller's `agentId` and a `target`. It SHALL
close the target only if the caller created it; otherwise it SHALL fail with a
message stating the caller may only close agents it created.

#### Scenario: Cannot close a user-created agent

- **WHEN** an agent calls `close-agent` targeting an agent it did not create
- **THEN** the call fails and nothing is closed

### Requirement: create-worktree

`create-worktree` SHALL require `repoPath` and a non-empty `branchName`, create
a worktree on a new branch, and return the new worktree path on success.

#### Scenario: Empty branch name

- **WHEN** `create-worktree` is called with an empty `branchName`
- **THEN** the result is an error and no worktree is created

### Requirement: set-status

`set-status` SHALL require the caller's `agentId` and a `status` string. It
SHALL set the agent's human-readable status text; an empty string SHALL clear
it. This status is distinct from the automatic state machine value.

#### Scenario: Clear status

- **WHEN** `set-status` is called with an empty `status`
- **THEN** the agent's status text becomes empty and its state is unchanged

### Requirement: display-markdown and view-mermaid

`display-markdown` SHALL require `agentId` and `filePath` and accept optional
`maximized`. `view-mermaid` SHALL require `agentId` and `source` and accept
optional `title`. Each SHALL update the target agent's panel state and return a
success indicator. `display-markdown` SHALL record a history of shown files,
most recent first.

#### Scenario: Show a markdown file

- **WHEN** `display-markdown` is called with a valid `filePath`
- **THEN** the agent's markdown panel targets that file and the file is
  prepended to its file history
