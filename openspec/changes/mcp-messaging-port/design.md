## Context

See proposal.md - Why. The contract is `openspec/specs/mcp-messaging/spec.md`.

The Swift reference is `Skwad/MCP/MCPMessageStore.swift` (the `actor` holding
the `[MCPMessage]` array: `add`, `getUnread`, `markAsRead`, `hasUnread`,
`getLatestUnreadId`, `cleanup`) and `Skwad/MCP/AgentCoordinator.swift`
(`sendMessage`, `checkMessages`, `broadcastMessage`, plus
`findAgent(byNameOrId:)` / `findAgentInSameWorkspace` for resolution and
`notifyAgentOfMessage` -> `agentDataProvider.injectText` for the delivery
nudge).

`skwad_agents::AgentStore` (from `agent-lifecycle-port`) exposes `agent(id)`
and `workspaces()` for read access, but has no public mutator for
`is_registered` or `state` - `create()` always yields an unregistered,
`Idle` agent, and nothing else can flip either field. Adding one would be
scope creep onto a crate this change doesn't own; instead `skwad-messaging`
takes pre-resolved `&Agent` values (all of `Agent`'s fields are already
`pub`), pushing workspace-membership resolution to the caller. See
"Agent resolution: `&[Agent]` slices, not `&AgentStore`" below.

Constraints from repo conventions: `Result` + `thiserror`, no panics in
library code, constants in one module, functions <= 5-6 args, `cargo
+nightly fmt`.

## Goals / Non-Goals

**Goals:**

- One crate, `skwad-messaging`, implementing every requirement in the spec
  against `skwad_agents::Agent` values the caller resolves and supplies -
  identity, registration, and idle state all read straight off `Agent`'s
  public fields.
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
(for `Agent`, `AgentState`) plus `thiserror`, `serde`, `uuid` (all already
pinned in `[workspace.dependencies]`). Modules: `consts` (retention cap
`100`), `error` (`SendError`), `message` (`Message`), `store`
(`MessageStore`), `routing` (`send`, `broadcast`, `check`, the shared
eligibility predicate), `notify` (`DeliveryNotifier` trait + `NoopNotifier`
test double). `lib.rs` re-exports the public surface.

### Rejection reason: a `thiserror` enum, not a bare `String`

The spec pins exact rejection text ("Sender not registered", "Recipient not
found", "Cannot send messages to shell agents", "Only the owner can send
messages to a companion agent", "Companion agents can only send messages to
their owner") as the wire-visible contract `mcp-tools` will surface verbatim
in `ToolCallResult` text - but "the wire text is fixed" is an argument for a
typed enum, not against one: `SendError`'s `#[error("...")]` attributes
produce the exact spec strings via `Display`/`.to_string()`, while every
caller that branches on *which* rejection happened (tests here, and any
future caller) matches an enum variant instead of comparing strings -
no typo risk, exhaustiveness-checked by the compiler. `send(...) ->
Result<Uuid, SendError>` (the `Uuid` being the new message's id on success)
and `broadcast(...) -> usize` (0 already means "nothing sent", matching the
spec's "unregistered sender" and "no eligible recipients" cases identically)
keep the crate's public surface exactly as small as the spec's scenarios
require.

### Eligibility as one shared predicate

`send` and `broadcast` apply the identical three checks (not shell,
companion ownership both directions) per the spec's "Broadcast SHALL apply
the same filter per recipient." A private `fn eligible(sender: &Agent,
recipient: &Agent) -> Result<()>` in `routing.rs` is the single
implementation both call - registration and workspace-membership are
resolved separately by each caller (`send` via `workspace_members`
containment, `broadcast` via an `is_registered` filter over the same slice)
since those two checks differ in shape between a single named recipient and
an iterated fan-out.

### Agent resolution: `&[Agent]` slices, not `&AgentStore`

`send`/`broadcast` take `sender: &Agent` plus `workspace_members: &[Agent]`
(every agent, including `sender`, in the sender's workspace) rather than
`&skwad_agents::AgentStore`. `AgentStore` has no public mutator for
`is_registered`/`state` - only `create()` sets them, always to
unregistered/`Idle` - so a test (or any caller) cannot get an `AgentStore`
into a registered or non-`Idle` state at all. `Agent`'s fields are all
already `pub`, so both production callers (who resolve `sender` and collect
`workspace_members` from their own `AgentStore` via `.agent(id)` and
`.workspaces()`) and tests (which build `Agent { .. }` literals directly)
work from the same, already-public surface. This also keeps
`skwad-messaging` from reaching into `AgentStore`'s workspace-lookup
internals (`workspace_of` is private there) - the caller already has to
walk `.workspaces()` to find the sender's workspace for its own purposes.

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

- [Risk] `workspace_members` is caller-assembled, so a caller could pass a
  stale or mismatched slice (missing the sender, spanning two workspaces).
  -> `send`/`broadcast` don't trust the slice's provenance: they still check
  `sender.is_registered` and search `workspace_members` by id rather than
  assuming position or completeness, so a malformed slice degrades to
  "recipient not found" / a lower broadcast count, never a wrong delivery.
  The eventual caller (`mcp-tools`) builds the slice directly from its own
  `AgentStore.workspaces()` lookup, the same data `skwad-mcp`'s status
  endpoint already reads.
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
