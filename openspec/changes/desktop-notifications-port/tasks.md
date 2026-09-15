## 1. Crate scaffold

- [ ] 1.1 Create `crates/skwad-notifications` (`Cargo.toml` + `src/lib.rs`),
      add `objc2` and `objc2-user-notifications` (0.3.2, matching the
      version already resolved transitively) as dependencies, add it to the
      workspace members and to `skwad`'s dependencies. Verify
      `cargo build --workspace` succeeds with no version bump in
      `Cargo.lock` for either crate.

## 2. Pure logic (unit-testable, no objc2 calls)

- [ ] 2.1 Implement `should_notify(desktop_notifications_enabled: bool,
      already_awaiting: bool, is_selected: bool) -> bool` capturing the
      three suppression conditions (setting off, repeat/already-awaiting,
      currently selected). Verify with unit tests for all
      true/false combinations.
- [ ] 2.2 Implement `notification_body(message: Option<&str>) -> &str`
      returning the message when non-empty, else "Needs your attention".
      Verify with unit tests for `None`, empty string, and a real message.

## 3. UNUserNotificationCenter binding

- [ ] 3.1 Implement a `NotificationCenter` type wrapping
      `UNUserNotificationCenter.current()`: a `request_authorization()`
      method (alert + sound) and a `notify(agent_id: Uuid, title: &str, body:
      &str)` method building and adding a `UNNotificationRequest` with
      identifier `input-<uuid>` and `userInfo["agentId"]`, delivered
      immediately (`nil` trigger). Verify by manual run (noted in the PR
      description — objc2 framework calls are not unit-testable without a
      running notification center).
- [ ] 3.2 Implement the delegate (`UNUserNotificationCenterDelegate`
      equivalent): `willPresent` returns `.banner, .sound` so notifications
      show while the app is foregrounded; `didReceive` extracts `agentId`
      from `userInfo` and pushes it onto a `Mutex<Vec<Uuid>>` click-queue
      (mirroring the existing `awaiting_input`/`notifier` drain pattern),
      rather than calling back into GPUI directly. Verify with a unit test
      that a `didReceive` call with a valid `agentId` pushes it onto the
      queue, and one with a missing/invalid id pushes nothing.

## 4. Wiring in skwad

- [ ] 4.1 At startup (alongside existing one-time setup in `main.rs`), call
      `NotificationCenter::request_authorization()` once. Verify by manual
      run (noted in the PR description).
- [ ] 4.2 Where the `awaiting_input` queue is drained (`main.rs`, ~line
      2033), alongside building `AwaitingNotice`, also call `should_notify`
      with the agent's current state and `agent_selection`, and if true call
      `NotificationCenter::notify` with the agent's name and
      `notification_body(message)`. Verify with a unit test on the
      extracted decision function (reusing `should_show_awaiting_notice`'s
      existing suppression signal per design.md, composed with
      `should_notify`'s setting/repeat checks) rather than the full GPUI
      render loop.
- [ ] 4.3 Drain the click-queue each frame (same cadence as
      `awaiting_input`/`notifier`) and, for each clicked agent id still
      present in the store, set it as the selected agent and raise the
      window. Verify with a unit test on the pure "id present in store ->
      selection changes; id absent -> no-op" logic.

## 5. Final verification

- [ ] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo test --workspace`, and `cargo build
      --workspace`; confirm all pass clean. Note in the PR description that
      notification delivery and permission-prompt behavior were also
      manually verified by running the app, since `objc2` framework calls
      are not exercised by the automated test suite.
