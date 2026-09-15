## Why

`activity-detection`'s existing "Awaiting input" requirement already says the
system "SHALL raise a desktop notification" when an agent needs attention,
but nothing in the Rust port does that yet: `main.rs` only tracks an
in-window `AwaitingNotice` (a toast rendered inside the app's own view,
suppressed only by comparing it to the currently selected agent). There is no
macOS `UNUserNotificationCenter` notification, so a user with Knot
backgrounded or hidden to the menu bar — exactly the case the Swift
reference's `NotificationService` exists for — gets no signal at all. This
gap was found while auditing which Swift `Services/` files have no Rust
counterpart yet; there is no tracking GitHub issue prior to this proposal.

## What Changes

- Port `Skwad/Services/NotificationService.swift` behavior: request
  notification authorization once at startup; on an agent entering Awaiting
  input, raise a macOS desktop notification titled with the agent's name and
  bodied with the hook-supplied message (or a default), unless the setting
  is off, the agent is already Awaiting input (dedup against repeat hook
  events for the same prompt), or the agent is the one currently visible/
  selected in the app.
- Clicking the notification brings the app window forward. Full parity with
  Swift's `switchToAgent` (selecting that specific agent, in whichever
  window owns it) is deferred — see design.md and tasks.md: the port has no
  agent-id -> window registry yet to route the selection to.
- Gated by the existing `desktop_notifications_enabled` scalar setting
  (already ported, currently unread by any Rust code path).

## Capabilities

### New Capabilities

- `desktop-notifications`: OS-level (macOS `UNUserNotificationCenter`)
  notification delivery for the Awaiting-input event: authorization,
  dedup, visibility suppression, and click-to-navigate.

### Modified Capabilities

(none — `activity-detection`'s existing "raise a desktop notification"
requirement already covers the trigger condition; this proposal supplies the
delivery mechanism it currently lacks, without changing that requirement's
text)

## Impact

- `knot` binary only — no new crate. `gpui` (already linked via `gpui-kit`)
  turns out to already wrap `UNUserNotificationCenter` behind a
  platform-neutral `App::show_system_notification` /
  `on_system_notification_response` / `dismiss_system_notification` API, so
  this change is a few functions plus wiring inside `main.rs`, not a new
  binding layer (see design.md - Decisions).
- `knot` binary: wire the existing `awaiting_input` queue (already produced
  by `knot-activity`'s `Effect::AwaitingInput`) to also raise an OS
  notification via `cx.show_system_notification(...)`, alongside the
  existing in-window `AwaitingNotice` toast (kept as-is), and register a
  response callback once at startup to select the clicked notification's
  agent.
- `knot_core::Settings.desktop_notifications_enabled`: gains its first
  reader.
- No changes to `knot-activity`, `knot-agents`, or hook ingestion —
  the state machine and its `AwaitingInput` effect are unchanged.
