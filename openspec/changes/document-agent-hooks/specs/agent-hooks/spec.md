## Purpose

Defines how Skwad ingests plugin hook events from Claude and Codex agents:
the HTTP routes, request shape, agent-type dispatch, registration handling,
activity-status mapping, session-id capture, working-directory and model
metadata extraction, transcript last-message extraction, and the conditions
under which a completed turn triggers autopilot classification.

## ADDED Requirements

### Requirement: Hook routes and request shape

The system SHALL accept `POST /api/v1/agent/register` and
`POST /api/v1/agent/status`. Every hook request body SHALL be JSON containing at
least `agent_id` (a uuid string); a missing or unparseable `agent_id` SHALL
yield a 400. The body MAY contain `agent` (the agent type, default `claude`),
`session_id`, `source`, `hook`, `status`, and a nested `payload` object.

#### Scenario: Invalid agent id

- **WHEN** a hook request omits `agent_id`
- **THEN** the response is 400 and no state changes

#### Scenario: Unknown agent type

- **WHEN** a hook request carries `agent` set to a type with no handler
- **THEN** the response is 400 naming the unknown type

### Requirement: Agent-type dispatch

The register and status routes SHALL dispatch on the `agent` field. `claude`
SHALL use the Claude handler; `codex` SHALL use the Codex handler on the status
route. Any other value SHALL be rejected.

#### Scenario: Codex status routed to Codex handler

- **WHEN** a status hook arrives with `agent` = `codex`
- **THEN** the Codex handler processes it

### Requirement: Claude registration (SessionStart)

On the register route for a Claude agent, the system SHALL treat `source` of
`startup` as full registration and `source` of `resume` as a session-id
update. For `startup`, the session id SHALL be recorded unless the agent is a
pure resume (has a resume-session id and is not forking), in which case the
session id comes from the later `resume` event. For `resume`, the payload
session id SHALL be recorded unless the agent is forking. Successful
registration SHALL mark the agent registered and return the skwad member list.

#### Scenario: Fresh startup

- **WHEN** a Claude agent posts register with `source` = `startup` and a
  session id, and it is not resuming
- **THEN** the agent is registered with that session id and the response lists
  skwad members

#### Scenario: Resume event on a forking agent is ignored for session id

- **WHEN** a forking agent posts register with `source` = `resume`
- **THEN** its session id is not overwritten by the resume payload

### Requirement: Claude activity status

On the status route for a Claude agent, the system SHALL read `status` and map
`running` to Working, `idle` to Idle, `input` to Awaiting input; any other
value SHALL be a 400. The mapped status SHALL be applied with hook as the
source. An `input` status SHALL additionally raise a desktop notification,
using `payload.message` when present.

#### Scenario: Running hook

- **WHEN** a Claude status hook posts `status` = `running`
- **THEN** the agent becomes Working via the hook source and the
  input-protection guard is cancelled

#### Scenario: Input hook notifies

- **WHEN** a Claude status hook posts `status` = `input` with a payload message
- **THEN** the agent becomes Awaiting input and a desktop notification carrying
  that message is shown

### Requirement: Codex turn-complete

The Codex handler SHALL act only on a `payload.type` of `agent-turn-complete`;
any other event SHALL be a 400. On that event the system SHALL set the agent to
Idle via the hook source and, when `payload.thread-id` is a non-empty string,
record it as the agent's session id.

#### Scenario: Turn complete sets idle and session

- **WHEN** Codex posts a notify with `payload.type` = `agent-turn-complete` and
  a `thread-id`
- **THEN** the agent becomes Idle and its session id is the thread id

### Requirement: Metadata extraction

The system SHALL extract known string fields from the hook payload and merge
them into the agent's metadata, keeping only present non-empty values. For
Claude: `transcript_path`, `cwd`, `model`, `session_id`. For Codex: `cwd`,
`thread-id`, `turn-id`.

#### Scenario: cwd captured from hook

- **WHEN** a hook payload includes a non-empty `cwd`
- **THEN** the agent's metadata `cwd` is updated and other metadata keys are
  preserved

### Requirement: Transcript last-assistant-message extraction (Claude)

The system SHALL be able to read a Claude transcript JSONL file and return the
text of the last assistant message, scanning from the end. Content SHALL be
read from either a plain string or an array of text parts. If the user message
immediately preceding that assistant message is the Skwad registration prompt,
the extractor SHALL return an empty string so callers skip it. An unreadable or
message-free file SHALL return nothing.

#### Scenario: Registration reply is suppressed

- **WHEN** the last assistant message answers the registration prompt
- **THEN** the extractor returns an empty string

#### Scenario: Normal assistant reply

- **WHEN** the last assistant message follows an ordinary user message
- **THEN** the extractor returns that assistant text

### Requirement: Autopilot classification trigger

When autopilot is enabled and an AI API key is configured, a completed turn
SHALL trigger classification of the last assistant message: for Claude on a
`Stop` hook using the transcript extractor, for Codex using
`payload.last-assistant-message` directly. An empty or absent last message
SHALL NOT trigger classification, and classification SHALL run without blocking
the status response. Classification behavior itself is out of scope here.

#### Scenario: Disabled autopilot does not classify

- **WHEN** a Stop hook arrives with autopilot disabled
- **THEN** the status update still applies and no classification is started
