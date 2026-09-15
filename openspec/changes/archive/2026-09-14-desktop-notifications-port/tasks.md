## 1. Pure logic

- [x] 1.1 Implement `should_notify(desktop_notifications_enabled: bool,
      show_awaiting_notice: bool) -> bool` in `main.rs` alongside
      `should_show_awaiting_notice`, gating the existing in-window-toast
      suppression signal (which already covers dedup + currently-selected)
      behind the setting. Verify with unit tests for all true/false
      combinations. IMPLEMENTATION NOTE: revised from the original two
      separate `already_awaiting`/`is_selected` params — comparing against
      the *live* agent state for "already awaiting" would always read
      `Input` (the same hook event that fires this path already set it),
      permanently suppressing every notification. Reusing
      `should_show_awaiting_notice`'s message-diff dedup is the correct
      signal and avoids duplicating that logic.
- [x] 1.2 Implement `notification_body(message: &str) -> &str` returning the
      message when non-empty, else "Needs your attention". Verify with unit
      tests for empty string and a real message.

## 2. Wiring in skwad

- [x] 2.1 Where the `awaiting_input` queue is drained (`main.rs`), alongside
      building the existing `AwaitingNotice`, compute
      `should_show_awaiting_notice`'s result once and pass it to
      `should_notify` with `settings.desktop_notifications_enabled`; when
      true, call `app.show_system_notification(SystemNotification { tag:
      id.to_string().into(), title: format!("Knot - {name}").into(), body:
      notification_body(&message).into(), actions: Vec::new() })`. Verified
      with unit tests on `should_notify`/`notification_body` directly.
- [x] 2.2 At startup, register `cx.on_system_notification_response(...)`
      parsing the response's `tag` back into a `Uuid`
      (`notification_response_agent_id`) and, when valid, activating the app
      (`cx.activate(true)`). SCOPE REDUCTION: does not select the specific
      agent in a specific window. Each workspace is an independent `Shell`
      window/entity with its own `agent_selection`, and nothing currently
      tracks which open window owns which agent across windows — that
      registry doesn't exist yet, and building it is real new
      infrastructure beyond what this change's scope justifies. A click
      raises Knot to the front (parity with the existing `ShowAllWindows`
      action) but does not switch any window's selected agent. Follow-up:
      add an agent-id -> window registry, then extend the response handler
      to select the correct agent in its window. Verified with unit tests on
      `notification_response_agent_id` (valid/invalid tag).

## 3. Final verification

- [x] 3.1 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
      warnings`, `cargo test --workspace`, and `cargo build --workspace` all
      pass clean. Notification delivery was not manually verified by running
      the app in this session (no interactive macOS session available to
      exercise the permission prompt / actual notification banner) — flagged
      in the PR description as follow-up manual verification before merge,
      since `gpui`'s system-notification path is not exercised by the
      automated test suite.
