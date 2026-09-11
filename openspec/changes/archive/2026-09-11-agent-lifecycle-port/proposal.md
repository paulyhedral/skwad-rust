## Why

`skwad-core`'s settings store already persists `SavedAgent`, `Workspace`, and
`BenchAgent` (from `settings-persistence-port`), and `Settings` has a full
persona API (from `personas-port`). Nothing yet turns those durable records
into the runtime `Agent` the app actually operates on: no create/remove,
restart/resume, edit-with-conditional-restart, companion cascade, ordering, or
bench deployment. `agent-lifecycle` is next in the "standalone crate first"
order because, like `skwad-history`, it has a complete spec and needs no
terminal engine or MCP server - those are separate specs (`agent-hooks`,
`mcp-server`) that will depend on this crate's `Agent` type instead of the
other way around.

## What Changes

- Add a new crate `crates/skwad-agents` implementing
  `openspec/specs/agent-lifecycle/spec.md`:
  - `Agent` - the runtime struct: the eight durable fields mirrored from
    `SavedAgent` (id, name, avatar, folder, agent_type, created_by,
    is_companion, shell_command, persona_id) plus the runtime-only fields the
    spec names (state, status_text, is_registered, is_pending_start,
    terminal_title, restart_token, session_id, resume_session_id,
    fork_session, metadata). Runtime fields always start at their documented
    defaults; `skwad-agents` never reads them from a `SavedAgent`.
  - `AgentState` enum (`Idle`, `Running`, `Input`, `Error`), `String`-keyed
    like the Swift reference (`"Idle"`, `"Working"`, `"Awaiting input"`,
    `"Error"`) so any persisted/wire representation matches.
  - `AgentStore` - owns `Vec<Agent>` plus the workspace list (reuses
    `skwad_core::Workspace` for placement/ordering fields only; layout-mode
    interpretation stays out of scope) and implements the spec's operations:
    - `create` (folder + optional overrides, insert-after, workspace
      inheritance per the "Ordering and workspace placement" requirement)
    - `remove` (companion cascade, then unregister-if-registered as a caller
      hook, matching "Agent removal")
    - `restart` / `resume_session`
    - `edit` (name/avatar never restart; folder/agent_type/persona restart;
      optional companion relocation)
    - `deploy_bench` (existence check, prune-on-failure)
    - `reorder` within a workspace, `move_to_workspace`
  - `to_saved` / `from_saved` conversions between `Agent` and
    `skwad_core::SavedAgent`, used by the crate's own load/save helpers over
    `skwad_core::Settings` (`saved_agents`, `saved_workspaces`).
  - Constants (default agent type, default state) reuse `skwad_core::consts`
    where already defined; anything new lives in `crates/skwad-agents/src/consts.rs`.
  - `AgentError` (`thiserror`) with a crate `Result` alias.
  - Unit tests covering every scenario in the spec: create-with-defaults,
    insert-after-sibling, reload-drops-runtime-state, legacy-record-without-companion-fields,
    header-prefers-status-text, companion-bound-to-owner, remove-cascades-to-companions,
    remove-unregisters-first, restart-keeps-identity-drops-session,
    resume-targets-prior-session, rename-does-not-restart,
    folder-change-restarts-and-relocates-companions, new-agent-inherits-source-workspace,
    stale-bench-entry-is-pruned.
- Add `crates/skwad-agents` to the workspace `Cargo.toml` members. No new
  external dependencies: `uuid`, `serde`, `thiserror` are already workspace
  deps; `skwad-agents` depends on `skwad-core` for `Settings`/`SavedAgent`/
  `Workspace`/`BenchAgent`/`Persona`.

Non-goals:

- Terminal session creation/teardown, the terminal command builder, or
  activity tracking (`agent-hooks`, `agent-launch-command`) - `restart`/
  `resume_session` change the durable+runtime fields the spec names and bump
  `restart_token`; actually tearing down a terminal is a caller responsibility
  wired in a later change.
- MCP registration/unregistration itself - `remove` exposes that an
  unregister is owed (via the agent's `is_registered` flag) but does not call
  MCP; `mcp-server`/`mcp-tools` are separate specs.
- Split-pane / layout-mode selection logic (`enterSplit`, `applyCompanionLayout`,
  `selectAgent`, dashboard/detach state) - `Workspace`'s layout fields are
  carried opaquely, exactly as `skwad_core::Workspace` already treats them.
- Git stats, markdown panel, mermaid panel, shell-start staggering, message
  notification dedup - none are in `agent-lifecycle`'s spec.
- Any GUI - a later change wires `skwad-agents` into the app shell.

## Capabilities

### New Capabilities

None. This change implements the existing `agent-lifecycle` spec without
changing its requirements.

### Modified Capabilities

None. `openspec/specs/agent-lifecycle/spec.md` is the unchanged contract;
this change adds the implementation. `skip_specs: true`.

## Impact

- New crate: `crates/skwad-agents/` (`Cargo.toml`, `src/lib.rs`, `consts.rs`,
  `error.rs`, `agent.rs`, `store.rs`, `convert.rs`, `tests/`).
- Modified: root `Cargo.toml` (workspace members gains `skwad-agents`),
  `Cargo.lock`.
- No new external dependencies.
- `skwad-core`, `skwad-git`, `skwad-discovery`, `skwad-history`, `skwad`, and
  the Swift build are unaffected. `skwad-agents` depends only on `skwad-core`
  and `thiserror` (both via workspace where applicable).
