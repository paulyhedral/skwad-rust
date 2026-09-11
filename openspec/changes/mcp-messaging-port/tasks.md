## 1. Crate scaffold

- [x] 1.1 Create `crates/skwad-messaging/` with `Cargo.toml` (workspace
      edition; `skwad-agents` path dep; `thiserror`, `serde`, `uuid` via
      workspace) and empty `src/lib.rs`; add `skwad-messaging` to the root
      `Cargo.toml` workspace members; verify `cargo build -p
      skwad-messaging` succeeds and `cargo metadata` lists the crate.
- [x] 1.2 Add `src/consts.rs` (`READ_RETENTION_LIMIT: usize = 100`) and
      `src/error.rs` (`SendError`, a `thiserror` enum whose `Display`
      produces the spec's exact rejection strings, plus a crate `Result`
      alias); verify `cargo build -p skwad-messaging`.

## 2. Message model and store

- [x] 2.1 Implement `Message { id: Uuid, from: Uuid, to: Uuid, content:
      String, timestamp: SystemTime, is_read: bool }` in `src/message.rs`
      with a constructor that defaults `is_read` to false and mints a fresh
      `id`/`timestamp`; verify a unit test asserting both defaults (spec:
      "New message is unread").
- [x] 2.2 Implement `MessageStore` in `src/store.rs` wrapping `messages:
      Vec<Message>` with `add(&mut self, Message)`, `unread_for(&self,
      agent_id: Uuid) -> Vec<&Message>`, `mark_read(&mut self, agent_id:
      Uuid)`, `has_unread(&self, agent_id: Uuid) -> bool`,
      `latest_unread_id(&self, agent_id: Uuid) -> Option<Uuid>`; verify unit
      tests for unread filtering, mark-read clearing subsequent
      `has_unread`, and `latest_unread_id` returning the most recent match
      (spec: "Check clears unread").
- [x] 2.3 Implement `MessageStore::cleanup(&mut self)` capping read messages
      at `READ_RETENTION_LIMIT`, discarding the oldest read messages first,
      never touching unread ones; verify a unit test with >100 read messages
      plus some unread, asserting the count settles at 100 read + all unread
      survive (spec: "Old read messages pruned").

## 3. Delivery notifier seam

- [x] 3.1 Implement `DeliveryNotifier` trait (`fn notify(&self, agent_id:
      Uuid, message_id: Uuid)`) in `src/notify.rs` plus a `RecordingNotifier`
      test double (collects `(Uuid, Uuid)` calls behind a `Mutex`) and a
      `NoopNotifier` for callers with no delivery-side-effect need yet;
      verify a unit test that `RecordingNotifier` records exactly the calls
      made to it.

## 4. Routing: send, broadcast, check

- [x] 4.1 Implement the shared eligibility predicate in `src/routing.rs`:
      `fn eligible(sender: &Agent, recipient: &Agent) -> Result<()>`
      returning `SendError::ShellRecipient`/`NotCompanionOwner`/
      `CompanionNotOwner` for shell recipients and companion-ownership
      violations (both directions); verify unit tests for each rejection
      path in isolation (exercised via `send`/`broadcast`, below).
- [x] 4.2 Implement `send(store: &mut MessageStore, notifier: &dyn
      DeliveryNotifier, sender: &Agent, workspace_members: &[Agent],
      recipient_id: Uuid, content: impl Into<String>) -> Result<Uuid>`:
      reject an unregistered sender (`SenderNotRegistered`), resolve the
      recipient within `workspace_members` (`RecipientNotFound` if absent -
      covers both "not found" and "cross-workspace", since callers only ever
      pass same-workspace members), apply `eligible`, store the message,
      call `notifier.notify` when the recipient's `AgentState == Idle`,
      return the new message id. Deviates from the original `agents:
      &AgentStore` signature in tasks/design - `AgentStore` has no public
      mutator for `is_registered`/`state`, so tests (and any caller) could
      not construct a registered or non-`Idle` agent through it; taking
      pre-resolved `&Agent`/`&[Agent]` values sidesteps that gap without
      touching `skwad-agents`. Verified by unit tests: "Unregistered
      sender", "Cross-workspace send fails" (recipient absent from
      `workspace_members`), "Direct send to shell agent", "Owner messages
      its companion", "Third party messages a companion", "Companion can
      only message its owner", and both idle/busy notifier scenarios.
- [x] 4.3 Implement `broadcast(store: &mut MessageStore, notifier: &dyn
      DeliveryNotifier, sender: &Agent, workspace_members: &[Agent],
      content: impl Into<String>) -> usize`: on unregistered sender return
      0; otherwise iterate `workspace_members`, skip the sender and
      unregistered agents, apply `eligible` per recipient, store one message
      per eligible recipient, notify per the same idle-gating as `send`,
      return the count; verify unit tests for "Broadcast to a mixed
      workspace" and "Broadcast to a busy recipient" (notifier not called
      for a Working recipient).
- [x] 4.4 Implement `check(store: &mut MessageStore, agent_id: Uuid,
      mark_as_read: bool) -> Vec<Message>` for `check-messages`: returns
      unread messages for the given agent id, marks them read only when
      `mark_as_read` is true. Dropped the `agents: &AgentStore` parameter
      from the original signature - agent resolution already happened by
      the time a caller has an `agent_id` to check, so the store is unused
      dead weight here; verify a unit test for both the destructive default
      and the non-destructive read leaving flags unchanged.

## 5. Integration and checks

- [x] 5.1 Re-export the public surface (`Message`, `MessageStore`,
      `DeliveryNotifier`, `RecordingNotifier`, `NoopNotifier`, `send`,
      `broadcast`, `check`) from `lib.rs` with module docs linking
      `openspec/specs/mcp-messaging/spec.md`; verify `cargo doc -p
      skwad-messaging` builds with no warnings.
- [x] 5.2 Run `make rust` (nightly fmt check + clippy `-D warnings` + test +
      build) for the whole workspace and confirm it passes.
- [x] 5.3 Run `openspec validate mcp-messaging-port` and confirm the change
      validates.
