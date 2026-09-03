## Why

Claude and Codex agents report their own lifecycle (session start, turn start,
turn complete, permission prompts) through plugin hooks that POST to the Skwad
server. Those events are the authoritative status source for hook-capable
agents and also carry session ids and working-directory metadata. The Rust port
needs the event contract and the per-agent-type handling stated.

## What Changes

- Add `agent-hooks` spec: the two hook HTTP routes, request shape, agent-type
  dispatch, Claude handling (SessionStart register with startup/resume
  distinction, activity status from running/idle/input, transcript parsing for
  the last assistant message), Codex handling (single agent-turn-complete
  notify with the message inline, thread-id as session id), metadata
  extraction, and the optional autopilot classification trigger.
- No implementation code.

## Capabilities

### New Capabilities

- `agent-hooks`: ingest of plugin hook events from Claude and Codex agents —
  registration, activity status, session-id capture, metadata, transcript
  last-message extraction, autopilot trigger.

### Modified Capabilities

None.

## Impact

- New spec under `openspec/specs/agent-hooks/`.
- Feeds `activity-detection` (hook is an authoritative status source) and
  `agent-lifecycle` (session id, registered flag). The autopilot classifier
  itself is a separate capability documented later; this spec only fixes the
  trigger conditions.
- Non-goals: agent types without hooks (opencode, gemini, copilot, shell) —
  they use terminal-only detection and inline registration.
