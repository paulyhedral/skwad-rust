## Context

See proposal.md - Why. The contract is `openspec/specs/mcp-messaging/spec.md`.

The Swift reference is `Skwad/MCP/MCPMessageStore.swift` (the `actor` holding
the `[MCPMessage]` array: `add`, `getUnread`, `markAsRead`, `hasUnread`,
`getLatestUnreadId`, `cleanup`) and `Skwad/MCP/AgentCoordinator.swift`
(`sendMessage`, `checkMessages`, `broadcastMessage`, plus
`findAgent(byNameOrId:)` / `findAgentInSameWorkspace` for resolution and
`notifyAgentOfMessage` -> `agentDataProvider.injectText` for the delivery
nudge).

`skwad_agents::AgentStore` (from `agent-lifecycle-port`) already exposes what
resolution needs: `agents() -> &[Agent]` and `workspaces() -> &[Workspace]`,
where `Workspace.agent_ids: Vec<Uuid>` gives membership directly - no new
public API needed on `skwad-agents` for this change.

Constraints from repo conventions: `Result` + `thiserror`, no panics in
library code, constants in one module, functions <= 5-6 args, `cargo
+nightly fmt`.

## Goals / Non-Goals

**Goals:**

- One crate, `skwad-messaging`, implementing every requirement in the spec
  against `skwad_agents::AgentStore` as the source of truth for agent
  identity, registration, workspace membership, and idle state.
- Keep the crate synchronous and dependency-light: the spec's operations are
  all sub-millisecond map/vec work, matching the Swift actor's role (a
  guarded in-memory list), not an async service.
- A delivery-nudge seam (`DeliveryNotifier`) that lets `send`/`broadcast`
  report "this message should surface now" without the crate knowing what a
  terminal is.

**Non-Goals:**

- Resolving agents by name/id string ambiguity beyond what
  `AgentStore`/callers already provide - `send`/`broadcast`/`check` take
  `Uuid` sender and recipient identifiers already resolved by the caller
  (mirrors `mcp-tools`, which is the change that will resolve MCP tool
  string arguments to `Uuid`s before calling into this crate). Duplicating
  `findAgent(byNameOrId:)`-style name lookup here would re-implement
  resolution `mcp-tools` already owns per its own spec's tool argument
  contracts.
- A concrete `DeliveryNotifier` that touches a terminal - see proposal.md
  Non-goals.
- Concurrency primitives beyond what a single caller-owned `MessageStore`
  needs - no actor, no channel. If a future caller needs shared mutable
  access across threads, it wraps `MessageStore` in its own `Arc<Mutex<...>>`
  exactly like `skwad-mcp` does for `AgentStore` today.

## Decisions

### Crate layout

`skwad-messaging` as a sibling of `skwad-mcp`, depending on `skwad-agents`
(for `Agent`, `AgentState`, `AgentStore`) plus `thiserror`, `serde`, `uuid`
(all already pinned in `[workspace.dependencies]`). Modules: `consts`
(retention cap `100`), `error` (`MessageError`, though most failures are
returned as `Result<(), String>`/`String` per the spec's "rejection SHALL
return an explanatory string" rather than typed errors - matches the Swift
reference's `String?` return), `message` (`Message`), `store`
(`MessageStore`), `routing` (`send`, `broadcast`, `check`, the shared
eligibility predicate), `notify` (`DeliveryNotifier` trait + `NoopNotifier`
test double). `lib.rs` re-exports the public surface.

### Rejection strings: `Result<(), String>`, not a `thiserror` enum

The spec pins exact rejection text ("Sender not registered", "Recipient not
found", "Cannot send messages to shell agents", "Only the owner can send
messages to a companion agent", "Companion agents can only send messages to
their owner") as the wire-visible contract `mcp-tools` will surface verbatim
in `ToolCallResult` text. A `thiserror` enum would need a `Display` impl
producing the exact same strings, adding a type with no behavior the string
doesn't already carry. `send(...) -> Result<Uuid, String>` (the `Uuid` being
the new message's id on success) and `broadcast(...) -> usize` (0 already
means "nothing sent", matching the spec's "unregistered sender" and "no
eligible recipients" cases identically) keep the crate's public surface
exactly as small as the spec's scenarios require.

### Eligibility as one shared predicate

`send` and `broadcast` apply the identical four checks (registered, same
workspace, not shell, companion ownership) per the spec's "Broadcast SHALL
apply the same filter per recipient." A private `fn eligible(sender: &Agent,
recipient: &Agent) -> Result<(), String>` in `routing.rs` is the single
implementation both call - `broadcast` additionally filters `recipient.id !=
sender.id` and unregistered recipients before invoking it (those two aren't
rejection-worthy for a direct `send`, which already requires a resolved,
named recipient).

### `DeliveryNotifier`: trait + no-op test double, not a channel

```rust
pub trait DeliveryNotifier {
    fn notify(&self, agent_id: Uuid, message_id: Uuid);
}
```

`send`/`broadcast` take `&dyn DeliveryNotifier` and call `notify` once per
stored message where the recipient's `AgentState == Idle` at store time -
mirrors the spec's "subject to the input-protection guard" only insofar as
the guard itself lives with whatever owns the terminal; this crate's
contract stops at "tell the notifier a message landed for an idle agent."
Alternative considered: an `mpsc` channel the caller drains - rejected as
more machinery than a synchronous callback needs, and it would force
`skwad-messaging` to pick a channel type (`tokio::sync::mpsc` vs
`std::sync::mpsc`) that's really the terminal-owning crate's runtime
decision, not this crate's.

### Retention cleanup: caller-invoked, not automatic

Matches the Swift reference: `cleanup()` is a separate method the coordinator
calls periodically, not triggered inside `add`. `MessageStore::cleanup(&mut
self)` keeps that shape - automatic cleanup-on-every-add would mean every
`send` call pays an O(n) scan even when nowhere near the 100-message cap.

## Risks / Trade-offs

- [Risk] `send`/`broadcast` need both `Agent` lookup and `Workspace`
  membership from `AgentStore`, so a caller must pass a consistent
  `&AgentStore` snapshot; if a caller mixes a stale agent list with a fresh
  workspace list (or vice versa) eligibility checks silently use inconsistent
  data. -> Functions take a single `&AgentStore` parameter (never separate
  slices), so there is exactly one snapshot to keep consistent, and the
  eventual caller (`mcp-tools`, wired against the same `AgentStore` instance
  `skwad-mcp`'s status endpoint reads) already holds one shared reference.
- [Risk] `DeliveryNotifier` being a no-op until a later change wires a real
  terminal implementation means the idle-nudge requirement is only testable
  against a test double, not end-to-end, in this change. -> Acceptable: the
  spec's scenarios are phrased at the "notifier is/isn't called" level
  ("the inbox prompt is injected" reduces to "notify is called"), which the
  no-op double's call-recording variant verifies; the actual injection is
  `agent-hooks`/terminal-crate territory per the stack mapping.

## Migration Plan

New crate, additive workspace member - no migration. `skwad-agents`,
`skwad-mcp`, and earlier crates are unaffected.
