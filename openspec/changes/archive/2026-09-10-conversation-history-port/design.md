## Context

See proposal.md - Why. The contract is `openspec/specs/conversation-history/spec.md`.

The Swift reference is five files under `Skwad/Services/`:
`ConversationHistoryService.swift` (registry + cache) and one
`*HistoryProvider.swift` per agent, plus `TitleUtils.swift`. Each provider
reads a different on-disk format:

- claude: `~/.claude/projects/<dashed>/*.jsonl`, one JSON object per line.
- codex: `~/.codex/state_5.sqlite`, `threads` table; titles sometimes need a
  fallback read of the rollout JSONL named in `rollout_path`.
- gemini: `~/.gemini/tmp/<hash>/` located by a `.project_root` marker;
  `logs.json` is one flat array, `chats/session-*.json` holds full messages.
- copilot: `~/.copilot/session-state/<id>/workspace.yaml` (flat `key: value`),
  `events.jsonl` for the title fallback.

Constraints from repo conventions: no async runtime inside the crate (`skwad-git`
rule), `Result` + `thiserror`, no panics in library code, constants in one
module, functions <= 5-6 args, `cargo +nightly fmt`.

## Goals / Non-Goals

**Goals:**

- One crate, `skwad-history`, that returns `SessionSummary` lists per agent
  type with the spec's caching semantics.
- Blocking, runtime-agnostic API; caller owns threading.
- Fixture-driven tests that pin each on-disk format so a format drift fails
  loudly.

**Non-Goals:**

- Concurrency inside the crate beyond a plain `Mutex` around the cache map.
- A trait object zoo: four providers, one small trait, static dispatch where
  it is free.
- Exact byte-compat with the Swift cache-key string; the key is internal.

## Decisions

### Crate layout

`skwad-history` as a sibling of `skwad-git`. Modules: `consts`, `error`,
`title`, `cache`, `provider` (the trait + registry), and
`providers/{claude,codex,gemini,copilot}`. `lib.rs` re-exports
`SessionSummary`, `HistoryCache`, `HistoryError`, `Result`, and
`supports_history`.

Alternative: fold into `skwad-core`. Rejected - `skwad-core` has no
`rusqlite`/`time` deps and is depended on by the binary; keeping the SQLite
link out of the core crate matches how `skwad-git` stays separate.

### Registry as a match, not a map

`fn provider(agent_type: &str) -> Option<Box<dyn HistoryProvider>>` over a
`match`. Four arms. A `HashMap<&str, Box<dyn ...>>` built once buys nothing
here and needs `OnceLock`. `supports_history(agent_type)` is
`provider(agent_type).is_some()`.

### Trait shape

```rust
pub trait HistoryProvider {
    fn load_sessions(&self, folder: &str) -> Vec<SessionSummary>;
    fn delete_session(&self, id: &str, folder: &str);
}
```

Both methods swallow their own I/O errors and degrade (empty list / no-op),
because the spec requires a broken session file to still be listed and never
says an operation surfaces an error. Internal helpers return
`Result<_, HistoryError>` and are `.unwrap_or_default()`-ed at the trait
boundary, so the failure paths stay testable.

### Timestamp representation

`SessionSummary.timestamp: OffsetDateTime` (`time` crate).

- claude: file mtime via `std::fs::metadata`, `SystemTime` -> `OffsetDateTime`.
- codex: `updated_at` is unix seconds -> `OffsetDateTime::from_unix_timestamp`.
- gemini / copilot: RFC 3339 strings -> `OffsetDateTime::parse` with
  `time::format_description::well_known::Rfc3339`; on parse failure use
  `OffsetDateTime::UNIX_EPOCH` (mirrors Swift's `Date.distantPast` sort-to-bottom
  without pulling a "distant past" sentinel).

Alternative: `std::time::SystemTime` everywhere. Rejected - no RFC 3339 parse
in std, and callers will want a real datetime for display anyway.

### YAML: line parser, no crate

`workspace.yaml` in the reference is flat `key: value`. Port the Swift
`split(":", maxSplits: 1)` loop directly. `serde_yaml` is unmaintained and
`serde_yml` is a heavy transitive tree for three keys (`cwd`, `summary`,
`updated_at`). If a real nested document shows up later that is a new spec
concern, not this change.

### SQLite access

`rusqlite` with `features = ["bundled"]` so CI needs no system SQLite.
Open with `OpenFlags::SQLITE_OPEN_READ_ONLY` for reads. `delete_session`
needs a write (`archived = 1`) so it opens read-write; a missing or locked DB
makes delete a no-op, consistent with the trait rule. Prepared statements with
bound params, never string interpolation of `folder`.

### Cache

```rust
pub struct HistoryCache {
    entries: Mutex<HashMap<(String, String), Vec<SessionSummary>>>,
}
```

`get`, `refresh`, `invalidate`, `delete_session` take `&self`. `refresh` and
`delete_session` do disk I/O on the calling thread - the spec's "off the main
thread" is the caller's `spawn_blocking`, same contract as `skwad-git`. No
`is_loading` flag; that was SwiftUI view state, out of scope here.

`SessionSummary` derives `Clone`; `get` returns a clone of the vec so the lock
is released before the caller touches the data.

### TitleUtils port

`title.rs`, free functions: `is_registration_prompt`, `is_valid_title`,
`extract_title` (first line, trimmed, then `truncate`), `truncate` (>80 chars
-> first 77 + `...`). Registration-prompt needles and the 80/77 constants live
in `consts.rs`. Char-count truncation uses `chars().count()` /
`char_indices()`, not byte length, to match Swift's `String.count`.

### Claude command-message expansion

Port `formatCommandMessage`: find `<command-name>...</command-name>` and the
optional `<command-args>...</command-args>`, return `"name args"` or `"name"`.
Substring search on the line, no XML parser. If `<command-name>` is present
but unparsable, skip the line as a title candidate (Swift returns `""` which
then fails `isValidTitle`).

## Risks / Trade-offs

- [Codex `threads` schema or `state_5.sqlite` filename changes] -> pinned in
  `consts.rs`; a schema drift makes the query fail and the provider returns
  empty, which a fixture-DB test will catch on the vendored copy.
- [Gemini/Copilot store real YAML or nested JSON later] -> line parser returns
  `None` for the malformed file, session is dropped; revisit as a spec change,
  not silently patched here.
- [`rusqlite` bundled build adds C compile time to CI] -> one-time cost, both
  CI runners already build C (libghostty is downloaded, but `cc` is present).
  Accept it for hermetic tests.
- [Char vs grapheme truncation] -> Swift `String.count` is grapheme clusters,
  Rust `chars().count()` is scalars; differ only for combining marks in a
  title's first 80 chars. Acceptable; no `unicode-segmentation` dep for this.
- [`time` vs `chrono`] -> `time` is lighter and enough for parse + display.
  If a later crate needs `chrono`, both can coexist; not worth pre-deciding.

## Migration Plan

New crate, no existing behavior touched. Land behind the OpenSpec change,
merge with a merge commit (`feat(skwad-history): ...`). Rollback is deleting
the crate directory and its two workspace-dep lines. Nothing consumes it until
a later change wires the app.
