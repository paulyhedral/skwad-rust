## 1. Crate scaffold

- [x] 1.1 Create `crates/skwad-agents/` with `Cargo.toml` (workspace edition, `thiserror`, `serde`, `uuid` via workspace, `skwad-core` path dep) and empty `src/lib.rs`; add `crates/skwad-agents` to root `[workspace] members`; verify `cargo build -p skwad-agents` succeeds and `cargo metadata` lists the crate. Workspace members is a `crates/*` glob, so no root `Cargo.toml` edit was needed.
- [x] 1.2 ~~Add `src/consts.rs`~~ - no agent-lifecycle-specific constant was needed: `AgentState::default()` covers the default state, and `DEFAULT_AGENT_TYPE`/`DEFAULT_AVATAR` are reused from `skwad_core::consts` inline (`"claude"` literal in `store.rs::create`, avatar defaults to `String::new()` since `Agent.avatar` has no non-empty-string constraint the way `SavedAgent.avatar` does). No empty file added (YAGNI).
- [x] 1.3 Add `src/error.rs` with `AgentError` (`thiserror`: `NotFound(Uuid)`, `CompanionCannotOwn(Uuid)`) and `pub type Result<T, E = AgentError> = std::result::Result<T, E>`; re-export from `lib.rs`; verify `cargo build -p skwad-agents`.

## 2. Agent type

- [x] 2.1 Implement `AgentState` enum (`Idle`, `Running`, `Input`, `Error`) in `src/agent.rs` with `Serialize`/`Deserialize` matching Swift raw strings (`"Idle"`, `"Working"`, `"Awaiting input"`, `"Error"`) and `Default` = `Idle`; verify a round-trip test for each variant's exact string.
- [x] 2.2 Implement `struct Agent` with the eight durable fields (id, name, avatar, folder, agent_type, created_by, is_companion, shell_command, persona_id) plus the runtime fields the spec names (state, status_text, is_registered, is_pending_start, terminal_title, restart_token, session_id, resume_session_id, fork_session, metadata: `BTreeMap<String, String>`); verify `cargo build -p skwad-agents`.
- [x] 2.3 Implement `Agent::header_title(&self) -> &str` (status_text when non-empty, else terminal_title), matching "Three distinct status fields"; verify a unit test for both branches.

## 3. Durable/runtime conversion

- [x] 3.1 Implement `convert::from_saved(&SavedAgent) -> Agent`: copies the eight durable fields, sets every runtime field to its documented default (state Idle, status_text/terminal_title empty, is_registered/is_pending_start false, session_id/resume_session_id None, fork_session false, a fresh restart_token, empty metadata); verify a test that a `SavedAgent`-derived `Agent` is Idle/unregistered/no session id/no terminal title (spec: "Reload drops runtime state" - `SavedAgent` itself carries no runtime fields to begin with, so the test pins that `from_saved` never invents non-default state rather than clearing pre-set ones).
- [x] 3.2 Implement `convert::to_saved(&Agent) -> SavedAgent`, reading only the eight durable fields; verify a round-trip test `to_saved(from_saved(&x)) == x` for a `SavedAgent`.
- [x] 3.3 Verify legacy decode already covered: add a test building a `SavedAgent` from JSON missing `createdBy`/`isCompanion` (via `serde_json::from_str`, exercising `skwad_core`'s existing `#[serde(default)]`) and confirm `from_saved` yields `created_by: None`, `is_companion: false`, `agent_type: "claude"` (spec: "Legacy record without companion fields").

## 4. AgentStore: creation and ordering

- [x] 4.1 Implement `struct AgentStore { agents: Vec<Agent>, workspaces: Vec<Workspace>, current_workspace_id: Option<Uuid> }` with `new`, `agents(&self) -> &[Agent]`, `workspaces(&self) -> &[Workspace]`, `agent(&self, id) -> Option<&Agent>`, `add_workspace`, `current_workspace_id`/`set_current_workspace`; `current_workspace_id` was added beyond the original plan - the spec's "otherwise it joins the current workspace" fallback needs a notion of "current" the store must track; verify `cargo build -p skwad-agents`.
- [x] 4.2 Implement `struct CreateOptions` (named `CreateOptions`, not `CreateRequest`, for symmetry with `EditRequest` staying a "request" only where it mutates an existing agent) with folder passed as `AgentStore::create`'s first positional arg (folder is the one field every creation supplies) and the rest as overrides; `AgentStore::create(&mut self, folder, opts: CreateOptions) -> Uuid`: name defaults to folder's last component, agent_type defaults to `"claude"`, fresh id, appended or inserted after the named sibling in both `agents` and the target workspace; verify tests for "Create from folder with defaults" and "Insert after a sibling".
- [x] 4.3 Implement workspace inheritance and first-agent-becomes-active in `create`: target workspace is the `created_by` or `insert_after` source's workspace when one exists, else `ensure_current_workspace` (creates a default "Skwad" workspace on first use); if the target workspace has no active agent, the new agent becomes it; verify a test for "New agent inherits source workspace".
- [x] 4.4 Implement `AgentStore::create_shell_companion(&mut self, owner: Uuid) -> Result<Uuid>`: builds a `CreateOptions` with `agent_type = "shell"`, `created_by = owner`, `is_companion = true`, `insert_after = owner`; errors if `owner` is itself a companion; verify a test for "Companion is bound to its owner".

## 5. AgentStore: removal

- [x] 5.1 Implement `AgentStore::remove(&mut self, id: Uuid) -> Vec<RemovedAgent>` where `RemovedAgent { id: Uuid, was_registered: bool }`: recursively removes every companion (via `created_by == id`) before the agent itself, removing each from every workspace and the master list; verify a test for "Removing an owner cascades to companions" - asserts owner-last and both companions present, since the spec leaves sibling order unspecified and this store's "always insert right after the owner" rule reverses same-owner companion creation order (matches the Swift reference's identical insert-after-owner behavior).
- [x] 5.2 Verify `was_registered` is `true` in the returned list exactly for agents whose `is_registered` was `true` at removal time, and `false` otherwise; verify a test for "Removing a registered agent unregisters it" (asserted via the returned flag, since this crate does not call MCP itself).

## 6. AgentStore: restart and resume

- [x] 6.1 Implement private `AgentStore::recreate_terminal(&mut self, id: Uuid)` (named for what it does, not `start`): regenerates `restart_token`, resets `state = Idle`, sets `is_registered = false`, clears `terminal_title`. Deliberately does **not** touch `session_id`/`resume_session_id`/`fork_session` - the Swift reference's private `startAgent` doesn't either; only the public `restartAgent` clears them first. Folding that clearing into the shared helper would make `resume_session` (which needs those fields to survive) impossible without a workaround, so `restart` and `resume_session` each own their own session-field handling before delegating here.
- [x] 6.2 Implement `AgentStore::restart(&mut self, id: Uuid) -> Result<()>`: clears `session_id`/`resume_session_id`/`fork_session` then calls `recreate_terminal`; verify a test for "Restart keeps identity, drops session" (id unchanged, restart_token differs, session_id cleared, unregistered, state Idle).
- [x] 6.3 Implement `AgentStore::resume_session(&mut self, id: Uuid, session_id: impl Into<String>) -> Result<()>`: sets `resume_session_id` and `session_id` to the target, clears `fork_session`, then calls `recreate_terminal` (which leaves the just-set session fields alone); verify a test for "Resume targets a prior session" (resume_session_id and session_id both equal the target - this only holds because `recreate_terminal` doesn't clear them, unlike the originally planned single `start` that cleared everything).

## 7. AgentStore: edit

- [x] 7.1 Implement `struct EditRequest { name: String, avatar: String, folder: Option<String>, agent_type: Option<String>, persona_id: Option<Uuid>, persona_changed: bool, relocate_companions: bool }`; verify `cargo build -p skwad-agents`.
- [x] 7.2 Implement `AgentStore::edit(&mut self, id: Uuid, req: EditRequest) -> Result<()>`: name/avatar always update without a restart; folder change, agent_type change, or `persona_changed` each trigger `restart` after applying; verify a test for "Rename does not restart" (terminal_title/restart_token untouched).
- [x] 7.3 Implement companion relocation: when folder changes and `relocate_companions` is set, every companion of `id` whose `folder` equals the *old* folder is moved to the new folder and restarted; companions at a different folder are left alone; verify a test for "Folder change restarts and can relocate companions".

## 8. AgentStore: bench deployment

- [x] 8.1 Implement `AgentStore::deploy_bench(&mut self, bench: &BenchAgent, folder_exists: impl FnOnce(&Path) -> bool) -> Option<Uuid>` (folder-existence check injected so the crate stays filesystem-free and testable): resolved the sentinel-vs-`Option` choice from design.md to the simpler `Option<Uuid>` - `None` means "prune", `Some(id)` means "created"; a caller with no bench-removal step to run just ignores the `None` case, same shape either way; if `folder_exists` is false, returns `None`, no agent created; if true, creates an agent from the bench entry's folder/name/avatar/agent_type/shell_command/persona_id; verify a test for "Stale bench entry is pruned" (no agent created) and a success-path test (agent created with the bench entry's fields).

## 9. AgentStore: reorder and workspace movement

- [x] 9.1 Implement `AgentStore::reorder(&mut self, workspace_id: Uuid, from: usize, to: usize)` moving an id within `Workspace::agent_ids`; verify a unit test.
- [x] 9.2 Implement `AgentStore::move_to_workspace(&mut self, agent_id: Uuid, target_workspace_id: Uuid)`: removes from the source workspace's `agent_ids`/`active_agent_ids`, appends to the target, and if the target had no active agent, makes this one active; verify a unit test.

## 10. Integration and checks

- [x] 10.1 Re-export the public surface (`Agent`, `AgentState`, `AgentStore`, `CreateOptions`, `EditRequest`, `RemovedAgent`, `AgentError`, `Result`, plus `from_saved`/`to_saved`) from `lib.rs` with module docs linking `openspec/specs/agent-lifecycle/spec.md`; verify `cargo doc -p skwad-agents` builds with no warnings.
- [x] 10.2 Run `make rust` (nightly fmt check + clippy `-D warnings` + test + build) for the whole workspace and confirm it passes.
- [x] 10.3 Run `openspec validate agent-lifecycle-port` and confirm the change validates.
