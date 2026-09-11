## 1. Crate scaffold

- [ ] 1.1 Create `crates/skwad-messaging/` with `Cargo.toml` (workspace
      edition; `skwad-agents` path dep; `thiserror`, `serde`, `uuid` via
      workspace) and empty `src/lib.rs`; add `skwad-messaging` to the root
      `Cargo.toml` workspace members; verify `cargo build -p
      skwad-messaging` succeeds and `cargo metadata` lists the crate.
- [ ] 1.2 Add `src/consts.rs` (`READ_RETENTION_LIMIT: usize = 100`) and
      `src/error.rs` (re-export `pub type Result<T, E = MessageError>` if
      any operation needs a typed error beyond the spec's plain rejection
      strings - otherwise a placeholder module doc explaining why routing
      failures return `String` per design.md's "Rejection strings"
      decision); verify `cargo build -p skwad-messaging`.

## 2. Message model and store

- [ ] 2.1 Implement `Message { id: Uuid, from: Uuid, to: Uuid, content:
      String, timestamp: SystemTime, is_read: bool }` in `src/message.rs`
      with a constructor that defaults `is_read` to false and mints a fresh
      `id`/`timestamp`; verify a unit test asserting both defaults (spec:
      "New message is unread").
- [ ] 2.2 Implement `MessageStore` in `src/store.rs` wrapping `messages:
      Vec<Message>` with `add(&mut self, Message)`, `unread_for(&self,
      agent_id: Uuid) -> Vec<&Message>`, `mark_read(&mut self, agent_id:
      Uuid)`, `has_unread(&self, agent_id: Uuid) -> bool`,
      `latest_unread_id(&self, agent_id: Uuid) -> Option<Uuid>`; verify unit
      tests for unread filtering, mark-read clearing subsequent
      `has_unread`, and `latest_unread_id` returning the most recent match
      (spec: "Check clears unread").
- [ ] 2.3 Implement `MessageStore::cleanup(&mut self)` capping read messages
      at `READ_RETENTION_LIMIT`, discarding the oldest read messages first,
      never touching unread ones; verify a unit test with >100 read messages
      plus some unread, asserting the count settles at 100 read + all unread
      survive (spec: "Old read messages pruned").

## 3. Delivery notifier seam

- [ ] 3.1 Implement `DeliveryNotifier` trait (`fn notify(&self, agent_id:
      Uuid, message_id: Uuid)`) in `src/notify.rs` plus a `RecordingNotifier`
      test double (collects `(Uuid, Uuid)` calls behind `RefCell`/`Mutex`,
      whichever is simplest for a single-threaded test) and a `NoopNotifier`
      for callers with no delivery-side-effect need yet; verify a unit test
      that `RecordingNotifier` records exactly the calls made to it.

## 4. Routing: send, broadcast, check

- [ ] 4.1 Implement the shared eligibility predicate in `src/routing.rs`:
      `fn eligible(sender: &Agent, recipient: &Agent) -> Result<(), String>`
      returning the exact spec rejection strings for shell recipients and
      companion-ownership violations (both directions); verify unit tests
      for each of the four rejection paths in isolation.
- [ ] 4.2 Implement `send(store: &mut MessageStore, agents: &AgentStore,
      notifier: &dyn DeliveryNotifier, sender: Uuid, recipient: Uuid,
      content: String) -> Result<Uuid, String>`: resolve sender
      (unregistered -> "Sender not registered"), resolve recipient
      within the sender's workspace (not found/cross-workspace -> "Recipient
      not found"), apply `eligible`, store the message, call
      `notifier.notify` when the recipient's `AgentState == Idle`, return
      the new message id; verify unit tests for "Unregistered sender",
      "Cross-workspace send fails", "Direct send to shell agent", "Owner
      messages its companion", "Third party messages a companion", and both
      idle/busy notifier scenarios.
- [ ] 4.3 Implement `broadcast(store: &mut MessageStore, agents:
      &AgentStore, notifier: &dyn DeliveryNotifier, sender: Uuid, content:
      String) -> usize`: on unregistered sender return 0; otherwise iterate
      the sender's workspace, apply `eligible` (skipping the sender itself
      and unregistered agents) per recipient, store one message per
      eligible recipient, notify per the same idle-gating as `send`, return
      the count; verify unit tests for "Broadcast to a mixed workspace" and
      "Broadcast to a busy recipient" (notifier not called for a Working
      recipient).
- [ ] 4.4 Implement `check(store: &mut MessageStore, agents: &AgentStore,
      agent_id: Uuid, mark_as_read: bool) -> Vec<Message>` for
      `check-messages`: returns unread messages for the resolved agent,
      marks them read only when `mark_as_read` is true; verify a unit test
      for both the destructive default and the non-destructive read leaving
      flags unchanged.

## 5. Integration and checks

- [ ] 5.1 Re-export the public surface (`Message`, `MessageStore`,
      `DeliveryNotifier`, `RecordingNotifier`, `NoopNotifier`, `send`,
      `broadcast`, `check`) from `lib.rs` with module docs linking
      `openspec/specs/mcp-messaging/spec.md`; verify `cargo doc -p
      skwad-messaging` builds with no warnings.
- [ ] 5.2 Run `make rust` (nightly fmt check + clippy `-D warnings` + test +
      build) for the whole workspace and confirm it passes.
- [ ] 5.3 Run `openspec validate mcp-messaging-port` and confirm the change
      validates.
