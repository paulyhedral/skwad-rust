# conversation-history Specification

## Purpose
Defines how Skwad reads a coding agent's past sessions for a given project
folder: the uniform session-summary shape, the per-agent on-disk source and
format, the recency cap and ordering, folder-scoped caching with explicit
refresh and invalidate, and deleting a session then backfilling the list.

## Requirements

### Requirement: Session summary shape

Every provider SHALL return sessions as a summary carrying: id (the stable
session identifier), title (the first meaningful user message, possibly empty),
timestamp (best available recency signal), and message count (user plus
assistant messages, zero when not derivable).

#### Scenario: Unparseable session still listed

- **WHEN** a session file exists but cannot be parsed for a title
- **THEN** it is listed with an empty title, a zero message count, and its
  file timestamp

### Requirement: Provider registry by agent type

The system SHALL resolve a history provider by agent type. Providers exist for
`claude`, `codex`, `gemini`, and `copilot`. For any other agent type, history
SHALL be reported as unsupported and all history operations SHALL be no-ops.

#### Scenario: Unsupported agent type

- **WHEN** history is requested for a `shell` agent
- **THEN** support is reported false and no read is attempted

### Requirement: Claude source and format

The Claude provider SHALL read `*.jsonl` files under
`~/.claude/projects/<dashed-folder>`, where `<dashed-folder>` is the absolute
project path with path separators replaced by `-`. The session id is the file
name without extension; the timestamp is the file modification time.

#### Scenario: Dashed path resolution

- **WHEN** the folder is `/Users/x/src/app`
- **THEN** sessions are read from `~/.claude/projects/-Users-x-src-app`

### Requirement: Codex source and format

The Codex provider SHALL open `~/.codex/state_5.sqlite` read-only and select
threads whose `cwd` equals the folder and that are not archived, ordered by
`updated_at` descending, taking id, rollout path, title, and updated-at. A
missing database SHALL yield an empty list.

#### Scenario: No Codex database

- **WHEN** `~/.codex/state_5.sqlite` does not exist
- **THEN** the Codex provider returns an empty list

### Requirement: Gemini source and format

The Gemini provider SHALL locate the project directory under `~/.gemini/tmp`,
read `logs.json`, group entries by `sessionId`, and use the first user message
per session as the title with its timestamp.

#### Scenario: Title from first user message

- **WHEN** a Gemini session has multiple user entries
- **THEN** its title is the earliest user message

### Requirement: Copilot source and format

The Copilot provider SHALL enumerate `~/.copilot/session-state/<id>/` entries,
read each `workspace.yaml`, keep only those whose `cwd` equals the folder, and
use the yaml summary (or a resolved fallback) as the title. The session id is
the directory name.

#### Scenario: Folder mismatch excluded

- **WHEN** a Copilot session's `workspace.yaml` `cwd` differs from the folder
- **THEN** that session is not listed

### Requirement: Recency cap and ordering

Each provider SHALL return at most 20 sessions, most recent first.

#### Scenario: More than 20 sessions

- **WHEN** a folder has 50 sessions on disk
- **THEN** the 20 most recent are returned in descending recency order

### Requirement: Folder-scoped cache

Results SHALL be cached per `(agent type, folder)` key. Reads SHALL return the
cached list (empty when absent). Refresh SHALL reload from disk off the main
thread and replace the cache entry. Invalidate SHALL drop the entry without
reloading.

#### Scenario: Read without refresh

- **WHEN** sessions are read for a key that has never been refreshed
- **THEN** the result is an empty list and no disk read occurs

### Requirement: Delete then backfill

Deleting a session SHALL remove its files via the provider and then refresh the
`(agent type, folder)` cache so the list reflects the removal.

#### Scenario: List shrinks after delete

- **WHEN** a session is deleted
- **THEN** a subsequent read of that key no longer includes it
