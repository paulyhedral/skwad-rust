## 1. Pure logic

- [ ] 1.1 Implement `should_notify(desktop_notifications_enabled: bool,
      already_awaiting: bool, is_selected: bool) -> bool` in `main.rs`
      alongside `should_show_awaiting_notice`, capturing the three
      suppression conditions (setting off, repeat/already-awaiting,
      currently selected). Verify with unit tests for all
      true/false combinations.
- [ ] 1.2 Implement `notification_body(message: Option<&str>) -> &str`
      returning the message when non-empty, else "Needs your attention".
      Verify with unit tests for `None`, empty string, and a real message.

## 2. Wiring in skwad

- [ ] 2.1 Where the `awaiting_input` queue is drained (`main.rs`, ~line
      2033), alongside building the existing `AwaitingNotice`, also compute
      `already_awaiting` (agent's current state is already Awaiting input)
      and `is_selected` (`agent_selection == Some(id)`), and when
      `should_notify(settings.desktop_notifications_enabled, already_awaiting,
      is_selected)` is true, call
      `cx.show_system_notification(gpui::SystemNotification { tag:
      agent_id.to_string().into(), title: format!("Skwad - {name}").into(),
      body: notification_body(message).into(), actions: Vec::new() })`.
      Verify with a unit test on the extracted decision (composing
      `should_notify` with the existing `should_show_awaiting_notice`
      signal) rather than the full GPUI render loop.
- [ ] 2.2 At startup (alongside existing one-time setup in `main.rs`),
      register `cx.on_system_notification_response(...)`: parse the
      response's `tag` back into a `Uuid`, and if it matches an agent still
      present in the store, set it as the selected agent and activate the
      window. A tag that fails to parse, or an agent id no longer present,
      SHALL be a no-op. Verify with a unit test on the extracted "tag ->
      selection change" logic (parse + store lookup), independent of the
      `gpui` callback registration itself.

## 3. Final verification

- [ ] 3.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo test --workspace`, and `cargo build
      --workspace`; confirm all pass clean. Note in the PR description that
      notification delivery and click-to-navigate were also manually
      verified by running the app, since `gpui`'s system-notification path
      is not exercised by the automated test suite.
