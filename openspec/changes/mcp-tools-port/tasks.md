## 1. Crate scaffold

- [x] 1.1 Create `crates/skwad-mcp-tools/` with `Cargo.toml` (workspace
      edition; `skwad-mcp`, `skwad-agents`, `skwad-messaging`,
      `skwad-discovery`, `skwad-git` path deps; `thiserror`, `serde`,
      `serde_json`, `uuid`, `async-trait` via workspace) and empty
      `src/lib.rs`; add the crate to root `Cargo.toml` workspace members;
      verify `cargo build -p skwad-mcp-tools` succeeds.
- [x] 1.2 Add `src/consts.rs` (the thirteen tool name string constants) and
      `src/error.rs` if a local error type is needed beyond
      `ToolCallResult`; verify `cargo build -p skwad-mcp-tools`.
- [x] 1.3 Add `src/args.rs` with `require_str`, `optional_str`,
      `optional_bool` helpers over `&serde_json::Value`, each returning
      `Result<T, ToolCallResult>` with the "Missing required parameter: x"
      text on failure; verify unit tests for present/missing/wrong-type
      cases on each helper.

## 2. Agent panel state

- [x] 2.1 Add `markdown_file: Option<PathBuf>`, `markdown_maximized: bool`,
      `markdown_history: Vec<PathBuf>`, `mermaid_source: Option<String>`,
      `mermaid_title: Option<String>` to `Agent` in
      `crates/skwad-agents/src/agent.rs`, defaulted empty/None in
      `from_saved`; verify `cargo build -p skwad-agents` and existing
      `skwad-agents` tests still pass unchanged.
- [x] 2.2 Add `AgentStore::set_markdown_panel(id, file, maximized)` (updates
      the three markdown fields, pushes `file` to the front of
      `markdown_history`, dedup so an already-shown file moves to the front
      rather than duplicating) and `AgentStore::set_mermaid_panel(id,
      source, title)`; verify a unit test asserting history ordering
      (show A, then B, then A again -> history is `[A, B]`) and a mermaid
      round-trip test.

## 3. Agent tools (register-agent, list-agents, create-agent, close-agent, set-status)

- [x] 3.1 Implement `src/agents.rs` `register_agent` and `list_agents`
      handlers plus a shared `agent_not_found(store, provided_id) ->
      ToolCallResult` helper producing the recovery-list message; verify
      unit tests for the roster scenario, unregistered-caller lookup
      failure, and companion visibility (caller sees own workspace,
      excludes companions it doesn't own).
- [x] 3.2 Implement `create_agent`, including `resolve_create_agent_fields`
      (bench-template defaulting per design.md) and the
      `createWorktree`-without-`branchName` validation; verify unit tests
      for explicit-fields creation, bench-template creation, missing
      `branchName` when `createWorktree` is set, and missing
      name/agentType/repoPath with no bench id.
- [x] 3.3 Implement `close_agent` with the creator-only ownership check;
      verify unit tests for successful close by the creator and rejection
      when the caller didn't create the target.
- [x] 3.4 Implement `set_status`, including clearing to empty string;
      verify a unit test that an empty `status` clears `status_text`
      without changing `state`.

## 4. Messaging tools

- [x] 4.1 Implement `src/messaging.rs` `send_message`, `check_messages`,
      `broadcast_message` as thin adapters over `skwad_messaging::{send,
      check, broadcast}`, mapping `SendError`'s `Display` to
      `ToolCallResult::error`; verify unit tests for a successful send, a
      rejected send (shell recipient) surfacing the exact rejection text
      with `isError: true`, check-messages with and without `markAsRead`,
      and broadcast recipient count.

## 5. Repo and worktree tools

- [x] 5.1 Implement `src/repos.rs` `list_repos` and `list_worktrees` over
      `skwad_discovery::scan`; verify unit tests for the response shape
      (name + worktrees list; repoPath + worktree name/path list) and an
      unknown `repoPath` returning an empty worktree list.
- [x] 5.2 Implement `create_worktree` over
      `skwad_git::{Repository, worktree::is_working_tree}`, validating a
      non-empty `branchName` and a valid git repo before calling
      `create_worktree`; verify unit tests for the empty-`branchName`
      error, the not-a-git-repo error, and a successful create returning
      the new path (using a `tempfile` git fixture, matching
      `skwad-git`'s existing worktree tests).

## 6. Panel tools

- [x] 6.1 Implement `src/panels.rs` `display_markdown` (agent lookup, file
      existence check via `Path::exists`, calls
      `AgentStore::set_markdown_panel`) and `view_mermaid` (agent lookup,
      calls `AgentStore::set_mermaid_panel`); verify unit tests for a
      missing file, a missing agent, and successful calls asserting the
      resulting `Agent` state.

## 7. Catalog assembly and wiring

- [x] 7.1 Implement `McpToolCatalog` in `src/lib.rs`: holds the
      `Arc<Mutex<AgentStore>>` (or equivalent handle already used by
      `crates/skwad`) plus whatever `skwad-discovery`/`skwad-git` inputs
      the repo/worktree handlers need, implements `ToolCatalog::list`
      (all thirteen `ToolDefinition`s, descriptions/schemas matching the
      spec) and `ToolCatalog::call` (dispatch by name, `unknown tool: x`
      for anything else); verify a unit test asserting `list()` returns
      exactly the thirteen names with `type: "object"` schemas.
- [ ] 7.2 Wire `McpToolCatalog` into wherever `crates/skwad` constructs
      `McpServer`, replacing `EmptyCatalog`; verify `cargo build -p skwad`
      and a manual `tools/list` HTTP round-trip (or an integration test if
      one already drives `McpServer` end-to-end) returns the full catalog.

## 8. Full-suite verification

- [x] 8.1 Run `make rust` (fmt + clippy + test + build, whole workspace)
      and fix anything it surfaces; verify it exits clean. (Passes outside
      the sandbox; `skwad-mcp`'s HTTP tests need real socket binds and
      `skwad-discovery`'s watch test needs real filesystem-event delivery,
      both blocked inside the sandbox network/fs policy - pre-existing,
      unrelated to this change.)
