## Why

`mcp-tools` (next change) needs `send-message`, `check-messages`, and
`broadcast-message` to exist as callable Rust logic. Those three tools'
routing rules, retention, and delivery semantics are owned by the
`mcp-messaging` spec, not `mcp-tools` itself, so the queue needs its own
crate before the tool catalog can wire against it - same dependency shape
that made `mcp-server` land before `mcp-tools`.

## What Changes

- Add a new crate `crates/skwad-messaging` implementing
  `openspec/specs/mcp-messaging/spec.md`:
  - `Message` (id, sender/recipient `Uuid`, content, timestamp, read flag
    defaulting to false), held in an in-process store only - no
    persistence, matching the spec's "does not survive an app restart".
  - `MessageStore`: `add`, `unread_for(agent_id)`, `mark_read(agent_id)`,
    `has_unread(agent_id)`, `latest_unread_id(agent_id)`, and `cleanup()`
    (caps read messages at 100, oldest-first, never touches unread).
  - `send(sender: &Agent, workspace_members: &[Agent], recipient_id, content)
    -> Result<Uuid, SendError>` applying, in order: sender must be registered
    (`SendError::SenderNotRegistered`), recipient must resolve within
    `workspace_members` (`RecipientNotFound`), recipient must not be a shell
    agent (`ShellRecipient`), companion-ownership routing
    (`NotCompanionOwner` / `CompanionNotOwner`) - `SendError`'s `Display`
    produces the spec's exact rejection text.
  - `broadcast(sender: &Agent, workspace_members: &[Agent], content) ->
    usize` fanning out to every eligible recipient in `workspace_members`
    (excludes sender, unregistered, shell, and companion-rule violators),
    returning the count created.
  - `check(agent_id, mark_as_read: bool) -> Vec<Message>` for
    `check-messages`, non-destructive when `mark_as_read` is false.
  - A `DeliveryNotifier` trait (`notify(agent_id: Uuid, message_id: Uuid)`)
    that `send`/`broadcast` call once per stored message when the recipient
    is currently `AgentState::Idle`, replacing the Swift reference's direct
    `injectText` call - `skwad-messaging` has no terminal/PTY dependency, so
    the actual "type into the terminal" side effect stays with whichever
    crate owns the terminal, wired in later. This also fixes the spec's
    documented divergence: both send and broadcast idle-gate identically
    (the Swift reference nudges every broadcast recipient unconditionally).
  - Tests covering every scenario in the spec: new message is unread,
    unregistered sender, cross-workspace send, shell-agent send, companion
    routing (both directions), idle vs busy delivery nudge (direct and
    broadcast), check clears unread, broadcast fan-out count, retention
    cleanup.
- Add `crates/skwad-messaging` to the workspace `Cargo.toml` members. No new
  `[workspace.dependencies]` - reuses `skwad-agents`, `thiserror`, `serde`,
  `uuid` already pinned there.

Non-goals:

- Wiring `send-message`/`check-messages`/`broadcast-message` as MCP tools -
  `mcp-tools` is a separate change; this change only implements the queue
  and routing logic those tools will call.
- The actual terminal text injection performed by `DeliveryNotifier` - a
  later change (whichever crate owns the terminal/PTY) supplies a real
  implementation; this change ships the trait plus a no-op test double.
- Persistence, cross-restart durability, or any storage beyond an in-memory
  `Vec`/`HashMap` - the spec explicitly rules this out.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. `openspec/specs/mcp-messaging/spec.md` is the unchanged contract; this
change adds the implementation. `skip_specs: true`.

## Impact

- New crate: `crates/skwad-messaging/` (`Cargo.toml`, `src/lib.rs`,
  `consts.rs`, `error.rs`, `message.rs`, `store.rs`, `routing.rs`,
  `notify.rs`).
- Modified: root `Cargo.toml` (workspace members gains `skwad-messaging`),
  `Cargo.lock`.
- `skwad-core`, `skwad-git`, `skwad-discovery`, `skwad-history`,
  `skwad-mcp`, `skwad`, and the Swift build are unaffected. `skwad-messaging`
  depends on `skwad-agents` (for `Agent`/`AgentState`, to resolve senders and
  recipients and check idle state) plus `thiserror`, `serde`, `uuid` (all via
  workspace).
