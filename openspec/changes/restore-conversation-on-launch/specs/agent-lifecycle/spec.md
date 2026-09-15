## MODIFIED Requirements

### Requirement: Durable versus runtime fields

The system SHALL persist only these agent fields: id, name, avatar, folder,
agent type, created-by, is-companion, shell command, persona id. All other
fields are runtime-only and MUST reset to defaults when agents are loaded:
state (Idle), status text (empty), registered (false), pending-start (false),
terminal title (empty), session id (none), resume-session id (none), hook
metadata (empty), git stats (none).

When the `restore-conversation-on-launch` scalar setting is enabled, loading
agents as part of a layout restore SHALL, for each restored agent, look up the
most recent session for that agent's `(folder, agent type)` via the
conversation-history provider registry after runtime fields are reset to
defaults, and, if a session is found, set that agent's resume-session id to
the found session's id before its terminal session is launched. Session id
itself remains unset by this lookup; it is set by the normal resume flow when
the terminal actually resumes. When `restore-conversation-on-launch` is
disabled, when no history provider exists for the agent's type, or when no
session is found, resume-session id SHALL remain at its default (none) and
the agent launches fresh, with no error surfaced.

This lookup applies only to loading agents for layout restore (e.g. cold app
launch). It SHALL NOT apply to a manual "Restart" of an already-running
agent, which continues to always clear session id and resume-session id per
the Restart requirement.

#### Scenario: Reload drops runtime state

- **WHEN** an agent that was Working with a session id is persisted and reloaded
- **THEN** the reloaded agent is Idle, unregistered, with no session id and no
  terminal title

#### Scenario: Legacy record without companion fields

- **WHEN** a persisted agent record predates the created-by / is-companion
  fields
- **THEN** it loads with created-by unset and is-companion false, and agent type
  defaults to `claude` if absent

#### Scenario: Restore-conversation setting off leaves resume-session id unset

- **WHEN** `restore-conversation-on-launch` is disabled and an agent for
  folder `/Users/x/proj` with a matching prior `claude` session is loaded at
  layout restore
- **THEN** the reloaded agent's resume-session id is unset and it launches a
  fresh conversation

#### Scenario: Restore-conversation setting on finds a prior session

- **WHEN** `restore-conversation-on-launch` is enabled, an agent of type
  `claude` for folder `/Users/x/proj` is loaded at layout restore, and the
  conversation-history provider for `claude` reports a most-recent session
  `s9` for that folder
- **THEN** the reloaded agent's resume-session id is set to `s9` before its
  terminal launches, and session id remains unset until the resume completes

#### Scenario: No history available falls back to fresh launch

- **WHEN** `restore-conversation-on-launch` is enabled and an agent of type
  `shell` (no history provider) is loaded at layout restore
- **THEN** the reloaded agent's resume-session id remains unset and it
  launches fresh, with no error

#### Scenario: Manual restart is unaffected

- **WHEN** `restore-conversation-on-launch` is enabled and a running agent is
  manually restarted from the UI
- **THEN** its session id and resume-session id are cleared as usual; no
  history lookup is performed
