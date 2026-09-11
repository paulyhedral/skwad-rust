## Why

`skwad-git`, `skwad-discovery`, and the `skwad-core` settings store cover their
specs; the next slice is `conversation-history`. It is the last large
subsystem with a complete spec and no coupling to the agent runtime, the
terminal engine, or the MCP layer: it reads other coding agents' past sessions
for a project folder straight off disk. Porting it now keeps the "standalone
crate first" order (like `skwad-git` and `skwad-discovery`) before the MCP and
agent-lifecycle work that does depend on the runtime.

## What Changes

- Add a new crate `crates/skwad-history` implementing
  `openspec/specs/conversation-history/spec.md`:
  - `SessionSummary` — `id`, `title` (may be empty), `timestamp`, `message_count`
    (0 when not derivable).
  - `HistoryProvider` trait: `load_sessions(folder) -> Vec<SessionSummary>`,
    `delete_session(id, folder)`. Runtime-agnostic and blocking; the GUI wraps
    calls in `spawn_blocking`, same rule as `skwad-git`.
  - Provider registry by agent type: `claude`, `codex`, `gemini`, `copilot`.
    Any other type reports unsupported and every history operation is a no-op.
  - `claude` — read `*.jsonl` under `~/.claude/projects/<dashed-folder>`
    (absolute path, `/` -> `-`); session id is the file stem, timestamp is the
    file mtime; parse each line as JSON, count `user`/`assistant` entries, take
    the title from the first non-meta `user` message, expanding
    `<command-name>` / `<command-args>` wrappers. Delete removes `<id>.jsonl`
    and any `<id>` sibling.
  - `codex` — open `~/.codex/state_5.sqlite` read-only, select non-archived
    `threads` where `cwd = folder` ordered by `updated_at desc` limit 20,
    taking id, rollout path, title, updated-at; when the DB title is unusable,
    fall back to the first real `user_message` in the rollout JSONL. Missing DB
    -> empty list. Delete removes the rollout file and sets `archived = 1`.
  - `gemini` — find the `~/.gemini/tmp/<hash>/` dir whose `.project_root`
    equals the folder, read `logs.json`, group by `sessionId`, title from the
    first `user` entry per session (falling back to the session's chat file),
    20 most recent. Delete removes the chat file and drops the session's
    entries from `logs.json`.
  - `copilot` — enumerate `~/.copilot/session-state/<id>/`, read each
    `workspace.yaml` (flat `key: value`), keep entries whose `cwd` equals the
    folder, title from the yaml `summary` or a fallback parse of
    `events.jsonl`, 20 most recent. Delete removes the session directory.
  - `TitleUtils` port: registration-prompt / `<local-command-` / `/clear`
    rejection, first-line extraction, 80-char truncation (77 + `...`).
  - `HistoryCache` — per `(agent_type, folder)` map. Read returns the cached
    list (empty when absent, no disk read). Refresh reloads via the provider
    and replaces the entry. Invalidate drops the entry. Delete calls the
    provider then refreshes the entry.
- Add workspace dependencies: `rusqlite` (bundled SQLite, read-only use) and
  `time` (RFC 3339 parsing for the Gemini/Copilot timestamps). `serde` /
  `serde_json` are already in `[workspace.dependencies]`. No YAML crate; the
  `workspace.yaml` files are flat and parsed line-by-line as in the Swift
  reference.
- Constants (base paths, `state_5.sqlite` name, recency cap `20`, title max
  length, registration-prompt needles, command-wrapper tags) in
  `crates/skwad-history/src/consts.rs`.
- `HistoryError` (`thiserror`) with a crate `Result` alias; a provider I/O or
  parse failure degrades to an empty list rather than surfacing, matching the
  spec's "unparseable session still listed" behavior.
- Unit tests with fixture trees under `tests/`: dashed-path resolution,
  unparseable-session fallback, unsupported-agent no-op, per-provider source
  parsing, 20-session cap and ordering, read-without-refresh returns empty,
  delete-then-backfill shrinks the list, missing Codex DB yields empty.

Non-goals:

- Any GUI, sidebar view, or wiring into the app — later changes consume the
  cache.
- Reading full transcript bodies or rendering messages; only the summary shape.
- A watcher or automatic refresh; refresh is explicit.
- Reactive change notification; a mutation updates the map and returns.
- Writing Claude/Codex history, or migrating data between agents.

## Capabilities

### New Capabilities

None. This change implements the existing `conversation-history` spec without
changing its requirements.

### Modified Capabilities

None. `openspec/specs/conversation-history/spec.md` is the unchanged contract;
this change adds the implementation. `skip_specs: true`.

## Impact

- New crate: `crates/skwad-history/` (`Cargo.toml`, `src/lib.rs`, `consts.rs`,
  `error.rs`, `cache.rs`, `title.rs`, `providers/{claude,codex,gemini,copilot}.rs`,
  `tests/`).
- Modified: root `Cargo.toml` (`[workspace.dependencies]` gains `rusqlite`,
  `time`), `Cargo.lock`.
- Dependencies added: `rusqlite` (feature `bundled`), `time` (features
  `parsing`, `formatting` / `macros` as needed).
- `skwad-core`, `skwad-git`, `skwad-discovery`, `skwad`, and the Swift build
  are unaffected. `skwad-history` depends only on `thiserror`, `serde`,
  `serde_json`, `rusqlite`, `time` (all via workspace).
