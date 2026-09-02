## Why

Skwad lets a user browse and resume a coding agent's past sessions for the
current folder. Each supported agent stores its history in a different place
and format on disk. The Rust port needs the per-agent source locations, the
common session-summary shape, and the caching/refresh behavior stated.

## What Changes

- Add `conversation-history` spec: the provider abstraction, the common
  `SessionSummary` shape, per-agent-type source and format (Claude JSONL under
  `~/.claude/projects/<dashed-path>`, Codex SQLite `~/.codex/state_5.sqlite`,
  Gemini `~/.gemini/tmp/<project>/logs.json`, Copilot
  `~/.copilot/session-state/<id>/workspace.yaml`), the 20-session cap sorted by
  recency, folder-scoped caching with explicit refresh and invalidate, and
  delete-then-backfill.
- No implementation code.

## Capabilities

### New Capabilities

- `conversation-history`: read and list a coding agent's prior sessions for a
  folder, from each agent's on-disk store, with a uniform summary and a
  per-folder cache.

### Modified Capabilities

None.

## Impact

- New spec under `openspec/specs/conversation-history/`.
- Session ids feed `agent-lifecycle` resume/fork (an agent can be launched with
  `resume_session_id`).
- Non-goals: rendering the transcript, agent types without a history provider
  (opencode, shell), and writing/altering the agents' own history stores beyond
  deleting a session on request.
