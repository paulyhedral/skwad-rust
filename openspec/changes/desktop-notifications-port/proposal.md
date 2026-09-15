## Why

`activity-detection`'s existing "Awaiting input" requirement already says the
system "SHALL raise a desktop notification" when an agent needs attention,
but nothing in the Rust port does that yet: `main.rs` only tracks an
in-window `AwaitingNotice` (a toast rendered inside the app's own view,
suppressed only by comparing it to the currently selected agent). There is no
macOS `UNUserNotificationCenter` notification, so a user with Skwad
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
- Clicking the notification SHALL select that agent (equivalent to Swift's
  `switchToAgent`) and bring the app window forward.
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

- New crate or module owning the macOS notification center binding
  (candidate: a small `skwad-notifications` crate, or a module inside
  `skwad` if the surface stays this small — decided in design.md).
- `skwad` binary: wire the existing `awaiting_input` queue (already produced
  by `skwad-activity`'s `Effect::AwaitingInput`) to also raise an OS
  notification, alongside the existing in-window `AwaitingNotice` toast (kept
  as-is).
- `skwad_core::Settings.desktop_notifications_enabled`: gains its first
  reader.
- No changes to `skwad-activity`, `skwad-agents`, or hook ingestion —
  the state machine and its `AwaitingInput` effect are unchanged.
