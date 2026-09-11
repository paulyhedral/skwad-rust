## 1. Crate scaffold

- [x] 1.1 Create `crates/skwad-history/` with `Cargo.toml` (workspace edition, `thiserror`, `serde`, `serde_json` via workspace) and empty `src/lib.rs`; verify `cargo build -p skwad-history` succeeds and `cargo metadata` lists the crate.
- [x] 1.2 Add `rusqlite` (`features = ["bundled"]`) and `time` (`features = ["parsing", "formatting", "macros"]`) to root `[workspace.dependencies]`, reference both from `skwad-history/Cargo.toml`; verify `cargo build -p skwad-history` links and `Cargo.lock` updates.
- [x] 1.3 Add `src/consts.rs` with the base paths (`~/.claude/projects`, `~/.codex/state_5.sqlite`, `~/.gemini/tmp`, `~/.copilot/session-state`), recency cap `20`, title max/trunc `80`/`77`, registration-prompt needles, and command-wrapper tag strings; verify `cargo build -p skwad-history`.
- [x] 1.4 Add `src/error.rs` with `HistoryError` (`thiserror`) covering I/O, JSON, and SQLite failures, plus `pub type Result<T> = core::result::Result<T, HistoryError>`; re-export from `lib.rs`; verify `cargo build -p skwad-history`.

## 2. Title utilities

- [x] 2.1 Implement `src/title.rs`: `is_registration_prompt`, `is_valid_title` (empty / registration / `<local-command-` prefix / `/clear` rejection), `extract_title` (first line, trim, truncate), `truncate` (char count > 80 -> first 77 chars + `...`); verify unit tests cover each rejection branch and the truncation boundary at 80/81 chars.
- [x] 2.2 Implement Claude command-message expansion `format_command_message(&str) -> Option<String>` (`<command-name>` + optional `<command-args>` -> `"name args"` / `"name"`, `None` when unparsable); verify unit tests for name-only, name+args, and malformed input.

## 3. Core types and registry

- [x] 3.1 Define `SessionSummary { id: String, title: String, timestamp: OffsetDateTime, message_count: usize }` (derive `Clone`, `Debug`, `PartialEq`) in `lib.rs` or `provider.rs`; verify `cargo build -p skwad-history`.
- [x] 3.2 Define `trait HistoryProvider { fn load_sessions(&self, folder: &str) -> Vec<SessionSummary>; fn delete_session(&self, id: &str, folder: &str); }` and `fn provider(agent_type: &str) -> Option<Box<dyn HistoryProvider>>` (match over `claude`/`codex`/`gemini`/`copilot`) plus `pub fn supports_history(agent_type: &str) -> bool`; verify a test asserts `supports_history("shell")` is false and the four known types are true.

## 4. Claude provider

- [x] 4.1 Implement `dashed_dir(folder)` -> `~/.claude/projects/<folder with '/' -> '-'>` and a missing-dir guard returning empty; verify a unit test on `/Users/x/src/app` -> `-Users-x-src-app`.
- [x] 4.2 Implement `load_sessions`: list `*.jsonl`, sort by mtime desc, parse each line as JSON, count `user`/`assistant` types, take title from first non-`isMeta` `user` message (expanding command wrappers, applying `is_valid_title`/`extract_title`); include a session with empty title + 0 count only when it is the most recent and unparsable; cap at 20; verify fixture-tree tests for a normal session, a command-only first message, and the unparseable-most-recent fallback.
- [x] 4.3 Implement `delete_session`: remove `<id>.jsonl` and any `<id>` sibling entry, ignoring missing files; verify a fixture test that the files are gone and a second delete is a no-op.

## 5. Codex provider

- [x] 5.1 Build a tiny fixture `threads(id, rollout_path, title, updated_at, cwd, archived)` DB in a tempdir at test time (no vendored binary file - avoids a binary blob in git, same coverage) with archived and non-archived rows for two `cwd` values; verify a test opens it read-only.
- [x] 5.2 Implement `load_sessions`: missing DB -> empty; else prepared `SELECT id, rollout_path, title, updated_at FROM threads WHERE cwd = ?1 AND archived = 0 ORDER BY updated_at DESC LIMIT 20`, `updated_at` (unix secs) -> `OffsetDateTime`, `message_count` 0; verify the fixture test returns only the matching non-archived rows in recency order.
- [x] 5.3 Implement title resolution: use the DB title when `is_valid_title`, else parse the rollout JSONL for the first `payload.type == "user_message"` with a valid `payload.message`, else empty string; verify a fixture rollout file drives the fallback.
- [x] 5.4 Implement `delete_session`: remove the rollout file, then open read-write and `UPDATE threads SET archived = 1 WHERE id = ?1`; missing DB -> no-op; verify the fixture row flips to archived and drops out of a subsequent `load_sessions`.

## 6. Gemini provider

- [x] 6.1 Implement `find_project_dir(folder)`: scan `~/.gemini/tmp/*`, match the dir whose `.project_root` file trims to `folder`; missing -> `None`; verify a fixture-tree test.
- [x] 6.2 Implement `load_sessions`: read `logs.json` array, group by `sessionId`, keep the earliest `type == "user"` entry per session (message + RFC 3339 timestamp, epoch on parse failure), 20 most recent desc; title from that message or a fallback parse of `chats/session-*<shortId>*.json` first `user` text; `message_count` 0; verify fixture tests for grouping, ordering/cap, and the chat-file title fallback.
- [x] 6.3 Implement `delete_session`: remove the matching `chats/` file and rewrite `logs.json` without that `sessionId`'s entries; verify the fixture `logs.json` shrinks and a re-read omits the session.

## 7. Copilot provider

- [x] 7.1 Implement the flat `workspace.yaml` line parser (`key: value`, split once) returning `cwd`, `summary`, `updated_at`; verify a unit test on a sample file.
- [x] 7.2 Implement `load_sessions`: enumerate `~/.copilot/session-state/*`, keep dirs whose `workspace.yaml` `cwd` equals `folder`, title from `summary` (valid) else fallback parse of `events.jsonl` first `type == "user.message"` `data.content`, timestamp from `updated_at` (epoch on failure), sort desc, cap 20; verify fixture tests for the folder filter, the events fallback, and the cap.
- [x] 7.3 Implement `delete_session`: remove the `<id>/` directory, ignoring missing; verify a fixture test.

## 8. Cache

- [x] 8.1 Implement `HistoryCache { entries: Mutex<HashMap<(String, String), Vec<SessionSummary>>> }` with `new`; `get(agent_type, folder)` returns a clone of the entry or empty without disk I/O; verify a test that `get` on a fresh cache returns empty and no provider is invoked.
- [x] 8.2 Implement `refresh(agent_type, folder)`: no-op for an unsupported type, else call the provider and replace the entry; `invalidate(agent_type, folder)` drops the entry; verify tests that refresh populates from a fixture tree and invalidate clears it.
- [x] 8.3 Implement `delete_session(agent_type, id, folder)`: call the provider's delete then `refresh` the same key; verify a fixture test that the list no longer contains the deleted id.

## 9. Integration and checks

- [x] 9.1 Re-export the public surface (`SessionSummary`, `HistoryProvider`, `HistoryCache`, `HistoryError`, `Result`, `supports_history`) from `lib.rs` with module docs linking `openspec/specs/conversation-history/spec.md`; verify `cargo doc -p skwad-history` builds with no warnings.
- [x] 9.2 Run `make rust` (nightly fmt check + clippy `-D warnings` + test + build) for the whole workspace and confirm it passes.
- [x] 9.3 Run `openspec validate conversation-history-port` and confirm the change validates.
