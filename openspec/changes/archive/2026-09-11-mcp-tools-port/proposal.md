## Why

`skwad-mcp` dispatches `tools/list`/`tools/call` through a `ToolCatalog`
trait (`EmptyCatalog` is the only implementation so far). Every crate the
tool catalog needs - `skwad-agents` (lifecycle), `skwad-messaging`
(send/check/broadcast), `skwad-discovery` (repos), `skwad-git` (worktrees) -
is now ported, so the concrete catalog from `openspec/specs/mcp-tools/spec.md`
can be wired in.

## What Changes

- Add a new crate `crates/skwad-mcp-tools` implementing the thirteen tools
  the spec names, as a `ToolCatalog` (`skwad_mcp::ToolCatalog`) built over
  handles into `skwad-agents`, `skwad-messaging`, `skwad-discovery`, and
  `skwad-git`:
  - `register-agent`, `list-agents`: mark registered, return roster scoped
    to the caller's workspace (companions excluded unless owned).
  - `send-message`, `check-messages`, `broadcast-message`: thin adapters
    over `skwad_messaging::{send, check, broadcast}`; `SendError`'s
    `Display` becomes the `isError` text verbatim.
  - `list-repos`, `list-worktrees`: adapters over `skwad_discovery::scan`.
  - `create-agent`, `close-agent`: adapters over `AgentStore::create` /
    `AgentStore::remove`, including bench-template defaulting and the
    creator-only close restriction.
  - `create-worktree`: adapter over `skwad_git::Repository::create_worktree`
    plus `is_working_tree` for the not-a-repo check.
  - `set-status`: adapter over the agent's `status_text` field (already on
    `Agent`; no new state).
  - `display-markdown`, `view-mermaid`: new panel-state fields on `Agent`
    (`markdown_file: Option<PathBuf>`, `markdown_maximized: bool`,
    `markdown_history: Vec<PathBuf>`, `mermaid: Option<(String, Option
    <String>)>`) plus setters on `AgentStore`, since no UI consumes this yet.
    Tool handlers validate inputs and update this state; actually rendering
    a panel is later UI work, same deferral shape `skwad-messaging` used for
    `DeliveryNotifier`.
  - A single `agentNotFoundError`-equivalent helper producing the Swift
    reference's recovery-list message (every other agent's name/folder/id)
    so an agent that lost its id can re-identify itself.
  - JSON input parsing/validation per tool: missing required arguments
    return `ToolCallResult::error` naming the field, never a transport
    error (`isError: true`, not a JSON-RPC error response).
  - Tests covering every scenario in the spec: full catalog listing (all
    thirteen, each with name/description/object schema), missing-argument
    errors per tool, register roster, list-agents visibility scoping,
    rejected send surfaces the messaging error text, list-worktrees shape,
    create-agent from explicit fields / from a bench template / missing
    `branchName` when `createWorktree` is set, close-agent ownership
    check, create-worktree with an empty `branchName`, set-status clearing
    to empty, display-markdown history ordering, view-mermaid title
    handling.
- Wire the new catalog into `crates/skwad/src` wherever `McpServer` is
  constructed, replacing `EmptyCatalog`.
- Add `crates/skwad-mcp-tools` to the workspace `Cargo.toml` members. No new
  `[workspace.dependencies]`.

Non-goals:

- Rendering the markdown/mermaid panels in the UI - this change only adds
  the state the tools write to and returns a success indicator; a later
  UI change reads it.
- Any change to `mcp-server`, `mcp-messaging`, `agent-lifecycle`,
  `repo-discovery`, or `worktree-management` behavior - this change only
  consumes their existing public APIs.
- `agent-hooks` / `agent-launch-command` / `activity-detection` - separate,
  still-unported specs this change does not touch.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. `openspec/specs/mcp-tools/spec.md` is the unchanged contract this
change implements. `skip_specs: true`.

## Impact

- New crate: `crates/skwad-mcp-tools/` (`Cargo.toml`, `src/lib.rs`,
  `consts.rs`, `error.rs`, plus one module per tool group: `agents.rs`,
  `messaging.rs`, `repos.rs`, `panels.rs`).
- Modified: `crates/skwad-agents/src/agent.rs` and `store.rs` (panel-state
  fields and setters), `crates/skwad/src` (catalog wiring), root
  `Cargo.toml` (workspace members gains `skwad-mcp-tools`), `Cargo.lock`.
- `skwad-core`, `skwad-git`, `skwad-discovery`, `skwad-history`,
  `skwad-messaging`, `skwad-mcp` (`ToolCatalog` trait itself) are
  unaffected - this change is a consumer, not a change to their contracts.
