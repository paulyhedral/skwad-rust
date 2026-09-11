## Context

`skwad_mcp::ToolCatalog` is the seam (`crates/skwad-mcp/src/tools.rs`):

```rust
#[async_trait]
pub trait ToolCatalog: Send + Sync {
    fn list(&self) -> Vec<ToolDefinition>;
    async fn call(&self, name: &str, arguments: serde_json::Value) -> ToolCallResult;
}
```

`McpServer` already dispatches `tools/list`/`tools/call` through whatever
catalog it's given (`EmptyCatalog` today). Everything the thirteen tools need
already exists as a library call:

- `skwad_agents::AgentStore` - `create`, `remove`, `edit`, lookup, workspace
  membership.
- `skwad_messaging::{send, check, broadcast}` plus `SendError`.
- `skwad_discovery::scan` - `RepoInfo`/`WorktreeInfo`.
- `skwad_git::Repository::create_worktree`, `worktree::is_working_tree`.

None of these run behind a shared lock today outside `crates/skwad`'s own
wiring - this change assumes the catalog is constructed with `Arc<Mutex<...>>`
handles the binary crate already owns (same shape `McpServer`'s
`AgentsSnapshotFn` uses), not new locking primitives.

## Goals / Non-Goals

**Goals:**
- One `ToolCatalog` impl (`McpToolCatalog`) covering all thirteen tools per
  `openspec/specs/mcp-tools/spec.md`, string-for-string compatible tool
  names/schemas/error text with the Swift reference where the spec pins it.
- Every handler validates its own required arguments and returns
  `ToolCallResult::error` (never a raw panic or transport error) on a bad
  call.

**Non-Goals:**
- Building the gpui-kit panel UI that would actually render a markdown file
  or mermaid diagram - out of scope, tracked as a later UI change.
- Changing `AgentStore`'s create/remove/edit semantics - only adding
  panel-state fields and thin setters.
- A generic "response envelope" abstraction - each tool's success payload is
  a plain `#[derive(Serialize)]` struct matching its Swift response type
  (`ListAgentsResponse`, `CreateAgentResponse`, etc.), pretty-printed to
  `ToolContent::text` the same way `successResult` does in Swift.

## Decisions

**One crate, one file per tool group, not one file per tool.** Thirteen
tools share four backing systems (agents, messaging, repos/worktrees,
panels). Grouping by backing system (`agents.rs`, `messaging.rs`,
`repos.rs`, `panels.rs`) keeps each file's imports and error mapping
coherent; a `mod.rs`-style one-file-per-tool split would just fragment
near-identical arg-parsing boilerplate. `create-worktree` lands in
`repos.rs` alongside `list-repos`/`list-worktrees` since all three touch
`skwad-discovery`/`skwad-git`, not `agents.rs`, even though the spec groups
it near `close-agent` narratively.

**Argument parsing: a small helper, not `serde` `Deserialize` per tool.**
Swift pulls args with `arguments["x"] as? String` and returns a targeted
"Missing required parameter: x" on `nil`. A `#[derive(Deserialize)]` struct
per tool would report serde's generic "missing field" error, losing the
spec's per-field required-list precision (`register-agent`'s scenario names
the exact missing arg). Instead: a couple of free functions,
`require_str(args: &Value, key: &str) -> Result<&str, ToolCallResult>` and
`optional_str`/`optional_bool`, used the same way in every handler. Small
enough that a trait or macro would be overhead for four helper functions.

**Panel state lives on `Agent`, not a separate `PanelStore`.** The spec ties
markdown/mermaid state to "the target agent's panel state" 1:1 - no
cross-agent sharing, no independent lifecycle. A new struct plus a second
map keyed by agent id would duplicate what `AgentStore`'s existing
`Vec<Agent>` + lookup already provides. Fields:

```rust
pub struct Agent {
    // ...existing...
    pub markdown_file: Option<PathBuf>,
    pub markdown_maximized: bool,
    pub markdown_history: Vec<PathBuf>,   // most recent first, matches spec
    pub mermaid_source: Option<String>,
    pub mermaid_title: Option<String>,
}
```

`mermaid_source`/`mermaid_title` as two `Option`s rather than one
`Option<(String, Option<String>)>` - simpler field access from the future UI
reader, and Rust has no tuple-field-access sugar worth trading for it.

**`display-markdown` file existence check reads the filesystem directly.**
Swift uses `FileManager.default.fileExists`. Rust equivalent is
`std::path::Path::exists()` - no new dependency, matches the spec's
"File not found" failure mode. This is the one handler that touches the
filesystem instead of only in-memory state; it stays a synchronous
`std::fs` call (tokio's blocking-pool guidance is for slow I/O, and a stat
call on a local path is not that).

**Agent-not-found recovery message: one shared helper.** Every handler that
takes a caller/target `agentId` string needs the same "list every other
agent so you can find yourself" fallback. One function,
`agent_not_found(store: &AgentStore, provided_id: &str) -> ToolCallResult`,
called from each handler - matches the Swift reference's single
`agentNotFoundError` used from six call sites.

## Risks / Trade-offs

- [New mutable state on `Agent` with no reader yet] -> Acceptable: the spec
  requires the tools to persist this state now regardless of when a UI
  consumes it, same shape as `skwad-messaging`'s `DeliveryNotifier` trait
  landing before any terminal-injection implementation existed.
- [`create-agent`'s bench-template defaulting duplicates Swift's
  field-by-field fallback logic (`arguments["x"] ?? benchAgent?.x`)] ->
  Mitigation: a single `resolve_create_agent_fields` function takes the raw
  args and an `Option<&BenchAgent>` and returns the resolved
  name/type/repo/icon/command/persona tuple once, so the fallback chain
  exists in one place, tested directly rather than only through the full
  tool-call path.
- [`skwad-git`'s `create_worktree` shells out to `git`; a bad `repoPath` or
  concurrent worktree add could leave `git` mid-operation] -> Out of scope
  for this change: `Repository::create_worktree` already surfaces `git`'s
  own error via `GitError::Command`, and the tool handler just relays that
  text with `isError: true` - no new retry/rollback logic invented here.
